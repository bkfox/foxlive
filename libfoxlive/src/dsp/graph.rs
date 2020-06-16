use std::ops::Deref;
use std::convert::Into;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool,Ordering};

use petgraph as pg;
use petgraph::stable_graph as sg;

use crate as libfoxlive;
use libfoxlive_derive::service;
use crate::data::*;
use crate::data::sample::fill_samples;
use crate::rpc::channel::*;
use crate::rpc::*;

use super::dsp::{DSP,BoxedDSP};


/// Scope passed to graph objects when processing audio
pub trait ProcessScope : 'static {
    fn n_samples(&self) -> NSamples;
    fn last_frame_time(&self) -> NFrames;
}


/// Graph node
pub struct Unit<S,PS>
    where S: 'static+Sync+Sample, PS: 'static+Sync+ProcessScope
{
    /// Rendered buffer
    pub order: usize,
    /// Wether node have been mapped
    mapped: bool,
    /// Unit is being processing some audio
    pub processing: AtomicBool,
    /// Contained dsp
    pub dsp: BoxedDSP<S, PS>,
}

pub type Ix = ObjectIndex;
pub type NodeIndex = sg::NodeIndex<Ix>;
pub type EdgeIndex = sg::EdgeIndex<Ix>;
pub type Dag<S,PS> = sg::StableGraph<Unit<S,PS>, (), pg::Directed, Ix>;


/// Audio graph processing directed acyclic DSP nodes.
pub struct Graph<S,PS>
    where S: 'static+Sync+Sample, PS: 'static+Sync+ProcessScope+Clone
{
    /// The graph.
    dag: Dag<S,PS>,
    /// Nodes topologically sorted
    ordered_nodes: Vec<NodeIndex>,
    /// Max number of channels supported by nodes
    n_channels: NChannels,
    /// Buffer arena used to store nodes outputs.
    buffers: Vec<S>,
    /// A temporary buffer used in processing
    dry_buffer: Buffer<S,Vec<S>>,
    /// Node objects values map
    objects_map: BTreeMap<ObjectIndex, (NodeIndex,FieldInfo)>,
    /// Events transport broadcasting responses to all receivers (this allows to have a pubsub
    /// without the cost of multiple event queues).
    transport: Option<BroadcastChannel<service::Response<S,PS>,service::Request<S,PS>>>,
}


impl<S,PS> Unit<S,PS>
    where S: 'static+Sync+Sample, PS: 'static+Sync+ProcessScope
{
    /// Create a new unit
    fn new(dsp: BoxedDSP<S, PS>) -> Self
    {
        Unit {
            order: 0,
            mapped: false,
            processing: AtomicBool::new(false),
            dsp: dsp,
        }
    }

    /// Get buffer slice in the provided buffers arena
    fn buffer<'a>(&self, buffers: &'a mut Vec<S>, buffer_len: usize) -> SliceBuffer<'a,S> {
        let pos = self.order * buffer_len;
        (true,self.dsp.n_channels(),&mut buffers[pos..pos+buffer_len]).into()
    }

    /*fn process_audio(&mut self, scope: &PS, input: Option<&dyn BufferView<Sample=S>>) {
        self.buffer.resize(self.dsp.n_channels(), scope.n_samples());
        self.dsp.process_audio(scope, input, Some(&mut self.buffer));
    }*/
}

impl<D,S,PS> From<D> for Unit<S,PS>
    where S: 'static+Sync+Sample, PS: 'static+Sync+ProcessScope,
          D: DSP<Sample=S,Scope=PS>+Sync
{
    fn from(dsp: D) -> Self {
        Self::new(Box::new(dsp))
    }
}

impl<S,PS> Deref for Unit<S,PS>
    where S: 'static+Sync+Sample, PS: 'static+Sync+ProcessScope
{
    type Target = dyn DSP<Sample=S,Scope=PS>;

    fn deref(&self) -> &Self::Target {
        self.dsp.deref()
    }
}


unsafe impl<S,PS> Sync for Graph<S,PS>
    where S: 'static+Sync+Sample, PS: 'static+Sync+ProcessScope+Clone
{}

unsafe impl<S,PS> Send for Graph<S,PS>
    where S: 'static+Sync+Sample, PS: 'static+Sync+ProcessScope+Clone
{}

impl<S,PS> Graph<S,PS>
    where S: 'static+Sync+Sample, PS: 'static+Sync+ProcessScope+Clone
{
    /// Create a new empty `Graph`.
    pub fn new() -> Graph<S, PS> {
        Graph::with_capacity(0,0)
    }

    /// Create a new `Graph` with capacity for the provided nodes and edges.
    pub fn with_capacity(nodes: usize, edges: usize) -> Graph<S, PS> {
        Graph {
            dag: Dag::with_capacity(nodes, edges),
            ordered_nodes: Vec::with_capacity(nodes),
            n_channels: 0,
            buffers: Vec::new(),
            dry_buffer: Buffer::with_capacity(true, 2, 1024),
            objects_map: BTreeMap::new(),
            transport: None,
        }
    }

    /// Init event channel, returning other channel of the channel
    pub fn init_transport(&mut self, cap: usize)
        -> Option<BroadcastChannelRev<service::Response<S,PS>,service::Request<S,PS>>>
    {
        if self.transport.is_some() {
            return None;
        }

        let (a, b) = BroadcastChannel::channel(cap);
        self.transport = Some(a);
        Some(b)
    }

    /// Return node for the provided index.
    pub fn node(&self, index: NodeIndex) -> Option<&Unit<S,PS>> {
        self.dag.node_weight(index)
    }

    /// Return mutable node for the provided index.
    pub fn node_mut(&mut self, index: NodeIndex) -> Option<&mut Unit<S,PS>> {
        self.dag.node_weight_mut(index)
    }

    /// Return internal graph
    pub fn graph(&self) -> &Dag<S,PS> {
        &self.dag
    }

    /// Process graph nodes
    pub fn process_nodes(&mut self, scope: &PS) {
        let buffer_len = scope.n_samples() * self.n_channels as usize;
        self.buffers.resize(buffer_len * self.ordered_nodes.len(), S::equilibrium());
        let mut order = 0;

        for node_index in self.ordered_nodes.iter() {
            let node_index = *node_index;
            let node = self.dag.node_weight(node_index);

            // node has been removed
            if node.is_none() {
                continue;
            }

            let node = node.unwrap();
            node.processing.store(true, Ordering::Relaxed);

            // ensure buffer size
            let input =
                // Source: no need to process inputs nodes
                if node.is_source() {
                    None
                }
                // Filters and sink
                else {
                    let buffer = &mut self.dry_buffer;
                    buffer.resize(node.n_channels(), scope.n_samples());
                    buffer.fill(S::equilibrium());

                    // gather input buffers
                    let inputs = self.dag.neighbors_directed(node_index, pg::Direction::Incoming);
                    for input in inputs {
                        // take input if not removed
                        if let Some(input) = self.dag.node_weight(input) {
                            let node_buffer = input.buffer(&mut self.buffers, buffer_len);
                            buffer.merge_inplace(&node_buffer);
                        }
                    }

                    Some(&self.dry_buffer as &dyn BufferView<Sample=S>)
                };

            // process node
            let mut node = self.dag.node_weight_mut(node_index).expect("");
            node.order = order;
            if node.is_sink() {
                node.dsp.process_audio(scope, input, None);
            }
            else {
                let mut node_buffer = node.buffer(&mut self.buffers, buffer_len);

                let n = node.dsp.process_audio(scope, input, Some(&mut node_buffer));
                fill_samples(&mut node_buffer.as_slice_mut()[n..], S::equilibrium());

                if input.is_some() && node.wet() != S::identity() {
                    let input = input.unwrap();
                    let (dry, wet) = (-node.wet(), node.wet());
                    node_buffer.zip_map_inplace(input, &|a,b| a.mul_amp(wet).add_amp(b.mul_amp(dry).to_signed_sample()));
                }
            }
            node.processing.store(false, Ordering::Relaxed);
            order += 1;
        }
    }

    /// Notify graph that it has been updated after changes have been made.
    pub fn updated(&mut self) {
        self.ordered_nodes = pg::algo::toposort(&self.dag, None)
                                 .expect("cycles are not allowed");
    }

    /// Process all available events at once.
    pub fn process_requests(&mut self) {
        // FIXME: here nodes_updated detection
        let nodes_updated = true;

        while let Ok(Some(request)) = self.transport.as_mut().unwrap().receiver.try_recv() {
            let r = self.process_request(request);
            if let Some(r) = r {
                self.transport.as_mut().unwrap().sender.try_send(r);
            }
        }

        if nodes_updated {
            self.updated();
        }
    }

    /// Map object for a provided node
    fn map_node_object(&mut self, index: NodeIndex) {
        /*match self.dag.node_weight_mut(index) {
            Some(node) if !node.mapped => {
                let mut mapper = GraphObjectMapper {
                    objects_map: &mut self.objects_map,
                    node: index,
                };
                node.dsp.map_object(&mut mapper);
                node.mapped = true;
            },
            _ => {}
        };*/
    }
}

#[service]
impl<S,PS> Graph<S,PS>
    where S: 'static+Sync+Sample, PS: 'static+Sync+ProcessScope+Clone
{
    /// Add a new node for the provided `DSP`.
    pub fn add_node(&mut self, dsp: BoxedDSP<S,PS>) -> NodeIndex
    {
        self.n_channels = self.n_channels.max(dsp.n_channels());

        let index = self.dag.add_node(Unit::new(dsp));
        self.map_node_object(index);
        index
    }

    /// Add a new node as child of the provided parent.
    pub fn add_child(&mut self, parent: NodeIndex, dsp: BoxedDSP<S,PS>) -> NodeIndex {
        let child = self.add_node(dsp);
        self.dag.add_edge(parent, child, ());
        child
    }

    /// Add edge between two nodes
    pub fn add_edge(&mut self, parent: NodeIndex, child: NodeIndex) -> EdgeIndex {
        self.dag.add_edge(parent, child, ())
    }

    /// Remove a node
    pub fn remove_node(&mut self, node: NodeIndex) {
        self.dag.remove_node(node);
    }

    /// Remove an edge
    pub fn remove_edge(&mut self, edge: EdgeIndex) {
        self.dag.remove_edge(edge);
    }

    /// Remove edge between two nodes
    pub fn disconnect_nodes(&mut self, parent: NodeIndex, child: NodeIndex) {
        self.dag.find_edge(parent, child)
                .and_then(|edge| Some(self.dag.remove_edge(edge)));
    }
}


/*
impl<S,PS> Object for Graph<S,PS>
    where S: 'static+Sync+Sample, PS: 'static+Sync+ProcessScope+Clone
{
    fn object_meta(&self) -> ObjectMeta {
        ObjectMeta::new("graph", None)
    }

    fn get_value(&self, index: ObjectIndex) -> Option<Value> {
        self.objects_map.get(&index).and_then(|(node, _)| self.node(*node))
                                    .and_then(|node| node.get_value(index))
    }

    fn set_value(&mut self, index: ObjectIndex, value: Value) -> Result<Value, ()> {
        if let Some((node, _)) = self.objects_map.get(&index) {
            if let Some(node) = self.dag.node_weight_mut(*node) {
                while node.processing.load(Ordering::Relaxed) {}
                return node.dsp.set_value(index, value);
            }
        }
        Err(())
    }

    fn map_object(&self, mapper: &mut dyn ObjectMapper) {
        for (index, (_, map)) in self.objects_map.iter() {
            mapper.declare(*index, map.value_type, map.metadata.clone());
        }
    }
}


pub struct GraphObjectMapper<'a>
{
    objects_map: &'a mut BTreeMap<ObjectIndex,(NodeIndex,FieldInfo)>,
    node: NodeIndex,
}


impl<'a> ObjectMapper for GraphObjectMapper<'a> {
    fn declare(&mut self, index: ObjectIndex, value_type: ValueType, metadata: Metadatas)
    {
        self.objects_map.insert(self.objects_map.len() as ObjectIndex, (self.node, FieldInfo {
            index, value_type
            metadata: Vec::from(metadata),
        }));
    }
}*/



