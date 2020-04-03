use std::ops::Deref;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool,Ordering};

use petgraph as pg;
use petgraph::stable_graph as sg;

use crate::data::channels_buffer::ChannelsBuffer;
use crate::data::channels::*;
use crate::data::samples::{Sample,NSamples, NFrames};

use super::controller::*;
use super::dsp::{DSP,BoxedDSP};


/// Scope passed to graph objects when processing audio
pub trait ProcessScope : 'static {
    fn n_samples(&self) -> NSamples;
    fn last_frame_time(&self) -> NFrames;
}


/// Graph node
pub struct Unit<S,PS>
    where S: Sample,
          PS: ProcessScope
{
    /// Rendered buffer
    pub buffer: ChannelsBuffer<S>,
    /// Last buffer update time
    pub last_frame_time: NFrames,
    /// Unit is being processing some audio
    pub processing: AtomicBool,
    /// Contained dsp
    pub dsp: BoxedDSP<S, PS>,
    /// Wether controls have been mapped
    mapped: bool,
}

impl<S,PS> Unit<S,PS>
    where S: Sample,
          PS: ProcessScope
{
    fn new<D>(dsp: D) -> Unit<S,PS>
        where D: 'static+DSP<Sample=S,Scope=PS>
    {
        let n_channels = dsp.n_channels();
        Unit {
            buffer: ChannelsBuffer::with_capacity(n_channels, 1024),
            last_frame_time: 0,
            processing: AtomicBool::new(false),
            dsp: Box::new(dsp),
            mapped: false,
        }
    }

    fn process_audio(&mut self, scope: &PS, input: Option<&dyn Channels<Sample=S>>) {
        self.buffer.resize(self.dsp.n_channels(), scope.n_samples());
        self.dsp.process_audio(scope, input, Some(&mut self.buffer));
    }

    /// Return inner DSP consuming self.
    fn into_inner(self) -> BoxedDSP<S, PS> {
        self.dsp
    }
}


impl<S,PS> Deref for Unit<S,PS>
    where S: Sample,
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
    where S: Sample,
          PS: ProcessScope
{
    dag: Dag<S,PS>,
    ordered_nodes: Vec<NodeIndex>,
    dry_buffer: ChannelsBuffer<S>,
    controls: BTreeMap<ControlIndex, (NodeIndex,ControlMap)>,
}


unsafe impl<S,PS> Sync for Graph<S,PS>
    where S: Sample,
          PS: ProcessScope
{}

unsafe impl<S,PS> Send for Graph<S,PS>
    where S: Sample,
          PS: ProcessScope
{}


impl<S,PS> Graph<S,PS>
    where S: Sample,
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
            dry_buffer: ChannelsBuffer::with_capacity(2, 1024),
            controls: BTreeMap::new(),
        }
    }

    /// Return node for the provided index.
    pub fn node(&self, node: NodeIndex) -> Option<&Unit<S,PS>> {
        self.dag.node_weight(node)
    }

    /// Return mutable node for the provided index.
    pub fn node_mut(&mut self, node: NodeIndex) -> Option<&mut Unit<S,PS>> {
        self.dag.node_weight_mut(node)
    }

    /// Return internal graph
    pub fn graph(&self) -> &Dag<S,PS> {
        &self.dag
    }

    /// Return internal graph as mutable reference.
    pub fn graph_mut(&mut self) -> &mut Dag<S,PS> {
        &mut self.dag
    }

    /// Add a new node for the provided `DSP`.
    pub fn add_node<D>(&mut self, dsp: D) -> NodeIndex
        where D: 'static+DSP<Sample=S,Scope=PS>
    {
        self.dag.add_node(Unit::new(dsp))
    }

    /// Add a new node as child of the provided parent.
    pub fn add_child<D>(&mut self, parent: NodeIndex, dsp: D) -> NodeIndex
        where D: 'static+DSP<Sample=S,Scope=PS>
    {
        let child = self.dag.add_node(Unit::new(dsp));
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

    /// Process graph nodes
    pub fn process_nodes(&mut self, scope: &PS) {
        for node_index in self.ordered_nodes.iter() {
            let node_index = *node_index;
            let node = self.dag.node_weight(node_index);

            // node has been removed
            if node.is_none() {
                continue;
            }

            let node = node.unwrap();
            node.processing.store(true, Ordering::Relaxed);

            // node already processed: do next
            if node.last_frame_time == scope.last_frame_time() {
                continue;
            }

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
                            buffer.merge_inplace(&input.buffer, 0);
                        }
                    }

                    Some(&self.dry_buffer as &dyn Channels<Sample=S>)
                };

            // process node
            let mut node = self.dag.node_weight_mut(node_index).expect("");
            node.process_audio(scope, input);
            node.last_frame_time = scope.last_frame_time();

            node.processing.store(false, Ordering::Relaxed);
        }
    }

    /// Notify graph that it has been updated after changes have been made.
    pub fn updated(&mut self) {
        self.ordered_nodes = pg::algo::toposort(&self.dag, None)
                                 .expect("cycles are not allowed");
    }

    /// Map controls for a provided node
    fn map_node_controls(&mut self, node_index: NodeIndex, node: &mut Unit<S,PS>) {
        let mut mapper = GraphControlsMapper {
            controls: &mut self.controls,
            node: node_index,
        };
        node.dsp.map_controls(&mut mapper);
        node.mapped = true;
    }
}


impl<S,PS> Controller for Graph<S,PS>
    where S: Sample,
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
        if let Some((node, _)) = self.controls.get(&control) {
            if let Some(node) = self.dag.node_weight_mut(*node) {
                return node.dsp.set_control(control, value);
            }
        }
        Err(())
    }

    fn map_controls(&self, mapper: &mut dyn ControlsMapper) {
        for (control, (node, map)) in self.controls.iter() {
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




