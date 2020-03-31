use std::any::Any;

use crate::data::channels::*;
use crate::data::samples::Sample;
use super::graph::ProcessScope;


/// Generic DSP trait in order to process audio from graph.
pub trait DSP: Any {
    type Sample: Sample;
    type Scope: ProcessScope;

    fn process_audio(&mut self, scope: &Self::Scope, input: Option<&dyn Channels<Sample=Self::Sample>>,
                     output: Option<&mut dyn ChannelsMut<Sample=Self::Sample>>);

    /// Return number of handled channels
    fn n_channels(&self) -> NChannels {
        0
    }

    /// Return True if the DSP has inputs
    fn is_sink(&self) -> bool { false }

    /// Return True if the DSP has outputs
    fn is_source(&self) -> bool { false }
}


pub type BoxedDSP<S, PS> = Box<dyn DSP<Sample=S,Scope=PS>>;


