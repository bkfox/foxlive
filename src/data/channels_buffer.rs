use std::ops::Add;

use super::samples::*;
use super::channels::{NChannels,Channels};


/// Provide buffer for multiple channels where frames are stored in
/// continguous memory.
pub struct ChannelsBuffer<S: Default+Copy+Add<Output=S>> {
    // size of a frame
    n_samples: NSamples,
    // channels count
    n_channels: NChannels,
    // samples buffer
    samples: Vec<S>,
}

impl<S: Default+Copy+Add<Output=S>> ChannelsBuffer<S>
{
    pub fn new() -> ChannelsBuffer<S> {
        ChannelsBuffer::with_capacity(0, 0)
    }

    pub fn with_capacity(n_channels: NChannels, n_samples: NSamples) -> ChannelsBuffer<S> {
        ChannelsBuffer {
            n_samples: n_samples,
            n_channels: n_channels,
            samples: Vec::with_capacity((n_samples as usize) * (n_channels as usize))
        }
    }

    /// Resize buffer for the provided number of channels and frames.
    /// Calling this method invalidates its content.
    pub fn resize(&mut self, n_channels: NChannels, n_samples: NSamples) {
        self.n_channels = n_channels;
        self.n_samples = n_samples;
        self.samples.resize((n_channels as usize) * (n_samples as usize), Default::default());
    }

    /// Resize frame. It invalidates buffers' content.
    pub fn resize_frame(&mut self, n_samples: NSamples) -> NSamples {
        if self.n_samples != n_samples {
            self.resize(self.n_channels, n_samples);
        }
        self.n_samples
    }

    /// Resize buffer for the given channels count. It invalidates buffer's
    /// content.
    pub fn resize_channels(&mut self, n_channels: NChannels) -> NChannels {
        if self.n_channels != n_channels {
            self.resize(n_channels, self.n_samples);
        }
        self.n_channels
    }
}


impl<S: Default+Copy+Add<Output=S>> Channels for ChannelsBuffer<S> {
    type Sample = S;

    fn len(&self) -> NSamples {
        self.n_samples
    }

    fn n_channels(&self) -> NChannels {
        self.n_channels
    }

    fn channel<'a>(&'a self, channel: NChannels) -> SampleSlice<'a,S> {
        let start = (channel as usize) * (self.n_samples as usize);
        &self.samples[start..(start + (self.n_samples as usize))]
    }

    fn channel_mut<'a>(&'a mut self, channel: NChannels) -> SampleSliceMut<'a,S> {
        let start = (channel as usize) * (self.n_samples as usize);
        &mut self.samples[start..(start + (self.n_samples as usize))]
    }

    /*pub fn frames(&self) -> Chunks<T> {
        self.samples.chunks(self.n_samples as usize)
    }

    pub fn frames_mut(&mut self) -> ChunksMut<T> {
        self.samples.chunks_mut(self.n_samples as usize)
    }*/
}

