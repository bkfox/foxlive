use std::ops::Deref;
use std::marker::PhantomData;

use crate as libfoxlive;
use libfoxlive_derive::foxlive_controller;
use crate::data::channels::*;
use crate::data::samples::*;
use crate::format::media::*;

use super::controller::*;
use super::dsp::DSP;
use super::graph::ProcessScope;


/// View over a media
#[foxlive_controller("media")]
pub struct MediaView<S,PS,M>
    where S: Sample+IntoControlValue,
          PS: ProcessScope,
          M: 'static+Deref<Target=Media<S>>
{
    pub media: M,
    #[control(Index, "position")]
    pub pos: usize,
    #[control(F32(1.0,1.0,0.1), "ampl")]
    pub amp: S,
    phantom: PhantomData<PS>,
}

impl<S,PS,M> MediaView<S,PS,M>
    where S: Sample+IntoControlValue,
          PS: ProcessScope,
          M: 'static+Deref<Target=Media<S>>
{
    pub fn new(media: M, amp: S) -> Self {
        Self {
            media: media,
            pos: 0,
            amp: amp,
            phantom: PhantomData
        }
    }
}


impl<S,PS,M> DSP for MediaView<S,PS,M>
    where S: Sample+IntoControlValue,
          PS: ProcessScope,
          M: 'static+Deref<Target=Media<S>>
{
    type Sample = S;
    type Scope = PS;

    fn process_audio(&mut self, scope: &Self::Scope, _input: Option<&dyn Channels<Sample=Self::Sample>>,
                     output: Option<&mut dyn ChannelsMut<Sample=Self::Sample>>)
    {
        let output = output.unwrap();
        let buffers = self.media.buffers.read().unwrap();
        if buffers.n_samples() > (self.pos + scope.n_samples() as usize) {
            output.zip_map_inplace(&*buffers, self.pos, &|_, b| b*self.amp);
            self.pos = self.pos + scope.n_samples() as usize;
        }
    }

    fn n_channels(&self) -> NChannels { self.media.n_channels() }
    fn is_source(&self) -> bool { true }
}


