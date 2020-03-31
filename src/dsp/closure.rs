use std::marker::PhantomData;

use crate::data::channels::*;
use crate::data::samples::Sample;

use super::graph::ProcessScope;
use super::dsp::DSP;


/// Implement DSP trait for a closure
struct ClosureDSP<S,PS,F>
    where S: 'static+Sample,
          PS: 'static+ProcessScope,
          F: 'static+FnMut(&PS, Option<&dyn Channels<Sample=S>>, Option<&mut dyn ChannelsMut<Sample=S>>)
{
    n_channels: NChannels,
    is_source: bool,
    is_sink: bool,
    closure: F,
    phantom: PhantomData<(S,PS)>,
}

impl<S,PS,F> ClosureDSP<S,PS,F>
    where S: 'static+Sample,
          PS: 'static+ProcessScope,
          F: 'static+FnMut(&PS, Option<&dyn Channels<Sample=S>>, Option<&mut dyn ChannelsMut<Sample=S>>)
{
    fn new(n_channels: NChannels, is_source: bool, is_sink: bool, closure: F) -> Self {
        Self {
            n_channels: n_channels,
            is_source: is_source,
            is_sink: is_sink,
            closure: closure,
            phantom: PhantomData
        }
    }
}


impl<S,PS,F> DSP for ClosureDSP<S,PS,F>
    where S: 'static+Sample,
          PS: 'static+ProcessScope,
          F: 'static+FnMut(&PS, Option<&dyn Channels<Sample=S>>, Option<&mut dyn ChannelsMut<Sample=S>>)
{
    type Sample = S;
    type Scope = PS;

    fn process_audio(&mut self, scope: &Self::Scope, input: Option<&dyn Channels<Sample=Self::Sample>>,
                     output: Option<&mut dyn ChannelsMut<Sample=Self::Sample>>)
    {
        (self.closure)(scope, input, output)
    }

    fn n_channels(&self) -> NChannels { self.n_channels }
    fn is_source(&self) -> bool { self.is_source }
    fn is_sink(&self) -> bool { self.is_sink }
}

