use std::sync::{Arc,RwLock};

use smallvec::SmallVec;

use super::channels::*;
use super::samples::*;


/// Continguous samples buffer
pub type Buffer<S: Sample> = Vec<S>;

/// Container of multiple audio buffers
pub type Buffers<S: Sample> = SmallVec<[Buffer<S>; 5]>;

/// Container of multiple buffer slices
pub type BuffersSlices<'a,S: Sample> = SmallVec<[&'a mut [S]; 5]>;


impl<S: Sample> Channels for Buffers<S> {
    type Sample = S;

    fn n_samples(&self) -> NSamples {
        if self.len() > 0 {
            self[0].len()
        }
        else { 0 }
    }

    fn n_channels(&self) -> NChannels {
        self.len() as NChannels
    }

    fn channel(&self, channel: NChannels) -> SampleSlice<Self::Sample> {
        &self[channel as usize][..]
    }
}

impl<S: Sample> ChannelsMut for Buffers<S> {
    fn channel_mut(&mut self, channel: NChannels) -> SampleSliceMut<Self::Sample> {
        &mut self[channel as usize][..]
    }

    fn clear(&mut self) {
        for channel in self.iter_mut() {
            channel.clear();
        }
    }

    fn resize_channels(&mut self, channels: NChannels) {
        self.resize(channels as usize, Buffer::with_capacity(self.len()));
    }
}


impl<'a,S: Sample> Channels for BuffersSlices<'a,S> {
    type Sample = S;

    fn n_samples(&self) -> NSamples {
        if self.len() > 0 {
            self[0].len()
        }
        else { 0 }
    }

    fn n_channels(&self) -> NChannels {
        self.len() as NChannels
    }

    fn channel(&self, channel: NChannels) -> SampleSlice<Self::Sample> {
        &self[channel as usize][..]
    }
}


