use std::ops::{Add,Deref};
use std::sync::atomic::{AtomicBool,Ordering};

use petgraph as pg;
use petgraph::stable_graph as sg;

use crate::data::channels_buffer::ChannelsBuffer;
use crate::data::channels::*;
use crate::data::samples::{NSamples, NFrames};
use super::dsp::{DSP,BoxedDSP};


/// Scope passed to graph objects when processing audio
pub trait ProcessScope {
    fn n_samples(&self) -> NSamples;
    fn last_frame_time(&self) -> NFrames;
}


/// Graph node
pub struct Unit<S,PS>
    where S: Default+Copy+Add<Output=S>,
          PS: ProcessScope
{
    buffer: ChannelsBuffer<S>,
    last_frame_time: NFrames,
    processing: AtomicBool,
    dsp: BoxedDSP<S, PS>,
}

impl<S,PS> Unit<S,PS>
    where S: Default+Copy+Add<Output=S>,
          PS: ProcessScope
{
    fn new<D>(dsp: D) -> Unit<S,PS>
        where D: 'static+DSP<Sample=S,Scope=PS>
    {
        let n_outputs = dsp.n_outputs();
        Unit {
            buffer: ChannelsBuffer::with_capacity(n_outputs, 1024),
            last_frame_time: 0,
            processing: AtomicBool::new(false),
            dsp: Box::new(dsp),
        }
    }

    fn process_audio(&mut self, scope: &PS, input: Option<&dyn Channels<Sample=S>>) {
        self.buffer.resize_frame(scope.n_samples());
        self.dsp.process_audio(scope, input, Some(&mut self.buffer));
    }

    /// Return inner DSP consuming self.
    fn into_inner(self) -> BoxedDSP<S, PS> {
        self.dsp
    }
}


impl<S,PS> Deref for Unit<S,PS>
    where S: Default+Copy+Add<Output=S>,
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
    where S: Default+Copy+Add<Output=S>,
          PS: ProcessScope
{
    dag: Dag<S,PS>,
    ordered_nodes: Vec<NodeIndex>,
    dry_buffer: ChannelsBuffer<S>,
}


unsafe impl<S,PS> Sync for Graph<S,PS>
    where S: Default+Copy+Add<Output=S>,
          PS: ProcessScope
{}
unsafe impl<S,PS> Send for Graph<S,PS>
    where S: Default+Copy+Add<Output=S>,
          PS: ProcessScope
{}


impl<S,PS> Graph<S,PS>
    where S: Default+Copy+Add<Output=S>,
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
                    buffer.resize(node.n_outputs(), scope.n_samples());
                    buffer.fill(S::default());

                    // gather input buffers
                    let inputs = self.dag.neighbors_directed(node_index, pg::Direction::Incoming);
                    for input in inputs {
                        // take input if not removed
                        if let Some(input) = self.dag.node_weight(input) {
                            buffer.merge_inplace(&input.buffer);
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
    fn updated(&mut self) {
        self.ordered_nodes = pg::algo::toposort(&self.dag, None)
                                 .expect("cycles are not allowed");
    }
}

