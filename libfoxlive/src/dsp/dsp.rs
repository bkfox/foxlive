use std::any::Any;

use crate::data::{BufferView,Sample,NChannels};
use super::graph::ProcessScope;
use super::controller::Controller;


/// Generic DSP trait in order to process audio from graph.
pub trait DSP: Any+Controller {
    type Sample: Sample;
    type Scope: ProcessScope;

    fn process_audio(&mut self, scope: &Self::Scope, input: Option<&dyn BufferView<Sample=Self::Sample>>,
                     output: Option<&mut dyn BufferView<Sample=Self::Sample>>);

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


