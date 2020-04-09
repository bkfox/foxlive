use std::ops::Deref;
use std::convert::Into;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool,Ordering};

use petgraph as pg;
use petgraph::stable_graph as sg;

use crate::data::{Buffer,BufferView,SliceBuffer,Sample,NChannels,NSamples,NFrames};
use crate::data::samples::fill_samples;

use super::controller::*;
use super::dsp::{DSP,BoxedDSP};


/// Scope passed to graph objects when processing audio
pub trait ProcessScope : 'static {
    fn n_samples(&self) -> NSamples;
    fn last_frame_time(&self) -> NFrames;
}


/// Graph node
pub struct Unit<S,PS>
    where S: Sample+Default,
          PS: ProcessScope
{
    /// Rendered buffer
    pub order: usize,
    /// Wether controls have been mapped
    mapped: bool,
    /// Unit is being processing some audio
    pub processing: AtomicBool,
    /// Contained dsp
    pub dsp: BoxedDSP<S, PS>,
}

impl<S,PS> Unit<S,PS>
    where S: Sample+Default,
          PS: ProcessScope
{
    fn new<D>(dsp: D) -> Self
        where D: 'static+DSP<Sample=S,Scope=PS>
    {
        Unit {
            order: 0,
            mapped: false,
            processing: AtomicBool::new(false),
            dsp: Box::new(dsp),
        }
    }

    /// Get buffer slice in the provided buffers arena
    fn buffer<'a>(&self, buffers: &'a mut Vec<S>, buffer_len: usize) -> SliceBuffer<'a,S> {
        let pos = self.order * buffer_len;
        (true,self.n_channels(),&mut buffers[pos..pos+buffer_len]).into()
    }

    /*fn process_audio(&mut self, scope: &PS, input: Option<&dyn BufferView<Sample=S>>) {
        self.buffer.resize(self.dsp.n_channels(), scope.n_samples());
        self.dsp.process_audio(scope, input, Some(&mut self.buffer));
    }*/

    /// Return inner DSP consuming self.
    fn into_inner(self) -> BoxedDSP<S, PS> {
        self.dsp
    }
}


impl<S,PS> Deref for Unit<S,PS>
    where S: Sample+Default,
          PS: ProcessScope
{
    type Target = dyn DSP<Sample=S,Scope=PS>;

    fn deref(&self) -> &Self::Target {
        self.dsp.deref()
    }
}


pub type Ix = u32;
pub type NodeIndex = sg::NodeIndex<Ix>;
pub type EdgeIndex = sg::EdgeIndex<Ix>;
pub type Dag<S,PS> = sg::StableGraph<Unit<S,PS>, (), pg::Directed, Ix>;


pub struct Graph<S,PS>
    where S: Sample+Default,
          PS: ProcessScope
{
    dag: Dag<S,PS>,
    ordered_nodes: Vec<NodeIndex>,
    n_channels: NChannels,
    buffers: Vec<S>,
    dry_buffer: Buffer<S,Vec<S>>,
    controls: BTreeMap<ControlIndex, (NodeIndex,ControlMap)>,
}


unsafe impl<S,PS> Sync for Graph<S,PS>
    where S: Sample+Default,
          PS: ProcessScope
{}

unsafe impl<S,PS> Send for Graph<S,PS>
    where S: Sample+Default,
          PS: ProcessScope
{}


impl<S,PS> Graph<S,PS>
    where S: Sample+Default,
          PS: ProcessScope
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
            controls: BTreeMap::new(),
        }
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

    /// Add a new node for the provided `DSP`.
    pub fn add_node<D>(&mut self, dsp: D) -> NodeIndex
        where D: 'static+DSP<Sample=S,Scope=PS>
    {
        self.n_channels = self.n_channels.max(dsp.n_channels());

        let node = self.dag.add_node(Unit::new(dsp));
        self.map_node_controls(node);
        node
    }

    /// Add a new node as child of the provided parent.
    pub fn add_child<D>(&mut self, parent: NodeIndex, dsp: D) -> NodeIndex
        where D: 'static+DSP<Sample=S,Scope=PS>
    {
        let child = self.add_node(dsp);
        self.dag.add_edge(parent, child, ());
        child
    }

    /// Add edge between two nodes
    pub fn add_edge(&mut self, parent: NodeIndex, child: NodeIndex) -> EdgeIndex
    {
        self.dag.add_edge(parent, child, ())
    }

    /// Remove a node
    pub fn remove_node(&mut self, node: NodeIndex) -> Option<Box<dyn DSP<Sample=S,Scope=PS>>>
    {
        self.dag.remove_node(node).and_then(|n| Some(n.dsp))
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

    pub fn buffer_slice<'a>(buffers: &'a mut Vec<S>, n_channels: NChannels, pos: usize, len: usize) -> SliceBuffer<'a,S> {
        (true,n_channels,&mut buffers[pos..pos+len]).into()
    }

    /// Process graph nodes
    pub fn process_nodes(&mut self, scope: &PS) {
        let buffer_len = scope.n_samples() * self.n_channels as usize;
        self.buffers.resize(buffer_len * self.ordered_nodes.len(), S::default());
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
                    buffer.fill(S::default());

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
                fill_samples(&mut node_buffer.as_slice_mut()[n..], S::default());

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

    /// Map controls for a provided node
    fn map_node_controls(&mut self, index: NodeIndex) {
        match self.dag.node_weight_mut(index) {
            Some(node) if !node.mapped => {
                let mut mapper = GraphControlsMapper {
                    controls: &mut self.controls,
                    node: index,
                };
                node.dsp.map_controls(&mut mapper);
                node.mapped = true;
            },
            _ => {}
        };
    }
}


impl<S,PS> Controller for Graph<S,PS>
    where S: Sample+Default,
          PS: ProcessScope
{
    fn get_metadata(&mut self) -> Metadatas {
        vec!((String::from("name"), String::from("graph")))
    }

    fn get_control(&self, control: ControlIndex) -> Option<ControlValue> {
        self.controls.get(&control).and_then(|(node, map)| self.node(*node))
                                   .and_then(|node| node.get_control(control))
    }

    fn set_control(&mut self, control: ControlIndex, value: ControlValue) -> Result<ControlValue, ()> {
        if let Some((index, _)) = self.controls.get(&control) {
            if let Some(node) = self.dag.node_weight_mut(*index) {
                while node.processing.load(Ordering::Relaxed) {}
                return node.dsp.set_control(control, value);
            }
        }
        Err(())
    }

    fn map_controls(&self, mapper: &mut dyn ControlsMapper) {
        for (control, (_, map)) in self.controls.iter() {
            mapper.declare(*control, map.control_type, map.metadata.clone());
        }
    }
}


pub struct GraphControlsMapper<'a>
{
    controls: &'a mut BTreeMap<ControlIndex,(NodeIndex,ControlMap)>,
    node: NodeIndex,
}


impl<'a> ControlsMapper for GraphControlsMapper<'a> {
    fn declare(&mut self, control: ControlIndex, control_type: ControlType,
               metadata: Metadatas)
    {
        self.controls.insert(self.controls.len() as ControlIndex, (self.node, ControlMap {
            control: control,
            control_type: control_type,
            metadata: Vec::from(metadata),
        }));
    }
}


