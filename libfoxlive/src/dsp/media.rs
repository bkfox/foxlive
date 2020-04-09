use std::marker::PhantomData;

use ringbuf::*;

use crate as libfoxlive;
use libfoxlive_derive::foxlive_controller;
use crate::data::*;
use crate::data::time::*;
use crate::format::{Error,StreamInfo};
use crate::format::reader::*;

use super::controller::*;
use super::dsp::DSP;
use super::graph::ProcessScope;


/// View over a media
#[foxlive_controller("media")]
pub struct MediaView<S,PS>
    where S: Sample+Default+IntoSampleFmt+Unpin+IntoControlValue,
          S::Float: IntoControlValue,
          PS: ProcessScope,
{
    /// Reader. The only reason it is an Arc'ed is that it should be usable
    /// as future. MediaView will considered to be owner of the reader and
    /// handles its lifecycle.
    pub reader: SharedReader<S>,
    /// Cached data as ringbuffer consumer
    cache: Consumer<S>,
    /// Amplification
    #[control(I32(0,0,0), "amp")]
    amp: S::Float,
    /// Reading position
    #[control(Duration, "pos", get_pos, seek)]
    pos: NSamples,
    /// Stream information
    pub infos: Option<StreamInfo>,
    phantom: PhantomData<PS>,
}

impl<S,PS> MediaView<S,PS>
    where S: Sample+Default+IntoSampleFmt+Unpin+IntoControlValue,
          S::Float: IntoControlValue,
          PS: ProcessScope,
{
    pub fn new(rate: SampleRate, cache_duration: Duration) -> Self
    {
        let cache_size = ts_to_samples(cache_duration, rate) * 2 as NSamples;
        let (prod, cons) = RingBuffer::new(cache_size as usize).split();

        let reader = SharedReader::new(prod, rate, None);
        Self {
            reader: reader,
            cache: cons,
            amp: S::identity(),
            pos: 0,
            infos: None,
            phantom: PhantomData
        }
    }

    pub fn open<P: Into<String>>(&mut self, path: P) -> Result<(), Error> {
        let mut reader = self.reader.write().unwrap();
        match reader.open(&path.into(), None) {
            Ok(()) => {
                self.infos = Some(reader.stream().unwrap().infos());
                Ok(())
            },
            Err(e) => Err(e),
        }
    }

    pub fn seek(&mut self, pos: Duration) -> Result<Duration, Error> {
        let mut reader = self.reader.write().unwrap();
        self.pos = ts_to_samples(pos, reader.rate());
        self.cache.for_each(|_| {});
        reader.seek(pos)
    }

    fn get_pos(&self) -> Duration {
        samples_to_ts(self.pos, self.reader.read().unwrap().rate())
    }
}


impl<S,PS> Drop for MediaView<S,PS>
    where S: Sample+Default+IntoSampleFmt+Unpin+IntoControlValue,
          S::Float: IntoControlValue,
          PS: ProcessScope,
{
    fn drop(&mut self) {
        // stop reader
        self.reader.write().unwrap().stop();
    }
}


impl<S,PS> DSP for MediaView<S,PS>
    where S: 'static+Sample+Default+IntoSampleFmt+Unpin+IntoControlValue,
          S::Float: IntoControlValue,
          PS: ProcessScope,
{
    type Sample = S;
    type Scope = PS;

    fn process_audio(&mut self, _scope: &Self::Scope, _input: Option<&dyn BufferView<Sample=Self::Sample>>,
                     output: Option<&mut dyn BufferView<Sample=Self::Sample>>) -> usize
    {
        let output = output.unwrap();
        // ensure output is interleaved data buffer, since reading is
        output.set_interleaved(true);

        let (cache, n_channels) = (&mut self.cache, self.infos.as_ref().unwrap().n_channels);
        let count = (cache.remaining() - cache.remaining() % n_channels as usize)
                    .min(output.len());
        let slice = output.as_slice_mut();

        let count = cache.pop_slice(&mut slice[0..count]);
        for i in 0..count {
            slice[i] = slice[i].mul_amp(self.amp);
        }
        self.pos += count;
        count
    }

    fn n_channels(&self) -> NChannels {
        match self.infos {
            Some(ref infos) => infos.n_channels,
            None => 0,
        }
    }
    fn is_source(&self) -> bool { true }
}


