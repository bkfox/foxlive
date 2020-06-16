use std::any::Any;

use crate::rpc::Object;
use crate::data::{BufferView,Sample,NChannels};
use super::graph::ProcessScope;


/// Generic DSP trait in order to process audio from graph.
pub trait DSP: Any+Object {
    type Sample: Sample;
    type Scope: ProcessScope;

    /// Process audio using provided input and output. Return total number of written samples
    /// nevermind the channel.
    /// Sink always return 0 since they don't write to provided output.
    fn process_audio(&mut self, scope: &Self::Scope, input: Option<&dyn BufferView<Sample=Self::Sample>>,
                     output: Option<&mut dyn BufferView<Sample=Self::Sample>>) -> usize;

    // FIXME: return number of optional NChannels
    //
    // Having a DSP Graph such as:
    //
    //      media = n => effects => n => mixer => 2
    //
    // A DSP with None n_channels will have same output channels as input. Graph should take it
    // in account in order to have consistent buffer.
    //
    // Question is: should we have n_outputs and n_inputs separately?
    // - mapping in zip_map(..., mix):
    //      - allows to easilly handles channel mapping
    //      - down/up mixing will be done only when n_output || n_input will be Some
    //      - graph input mixing is handled automatically
    //      - graph building should handle None values for shared buffer
    //      - 
    /// Return number of handled channels
    fn n_channels(&self) -> NChannels {
        0
    }

    /// Return True if the DSP has inputs
    fn is_sink(&self) -> bool { false }

    /// Return True if the DSP has outputs
    fn is_source(&self) -> bool { false }

    /// Dry/Wet mix percentage, as 1.0 is full wet, 0.0 is full dry
    fn wet(&self) -> <<Self as DSP>::Sample as Sample>::Float { Self::Sample::identity() }
}


pub type BoxedDSP<S, PS> = Box<dyn DSP<Sample=S,Scope=PS>+Sync>;


