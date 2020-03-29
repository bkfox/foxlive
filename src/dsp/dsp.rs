use std::any::Any;

use crate::data::samples::Sample;
use crate::data::channels::{NChannels,Channels};
use super::graph::ProcessScope;


/// Generic DSP trait in order to process audio from graph.
pub trait DSP: Any {
    type Sample: Default+Copy;
    type Scope: ProcessScope;

    fn process_audio(&mut self, scope: &Self::Scope, input: Option<&dyn Channels<Sample=Self::Sample>>,
                     output: Option<&mut dyn Channels<Sample=Self::Sample>>);

    fn n_inputs(&self) -> NChannels {
        0
    }

    fn n_outputs(&self) -> NChannels {
        0
    }

    /// Return True if the DSP is a sink (aka does not output)
    fn is_sink(&self) -> bool {
        self.n_outputs() == 0
    }

    /// Return True if the DSP is a source (aka does not take inputs)
    fn is_source(&self) -> bool {
        self.n_inputs() == 0
    }
}


pub type BoxedDSP<S: Sample, PS: ProcessScope> = Box<dyn DSP<Sample=S,Scope=PS>>;


// TODO/FIXME: impl DSP for Fn()


