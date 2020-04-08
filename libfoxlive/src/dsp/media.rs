use std::marker::PhantomData;

use smallvec::SmallVec;
use ringbuf::*;

use crate as libfoxlive;
use libfoxlive_derive::foxlive_controller;
use crate::data::{BufferView,NChannels,NSamples,Sample,SampleRate,VecBuffer};
use crate::data::time::*;
use crate::format::{Error,Reader,StreamInfo};
use crate::format::reader::*;
use crate::format::futures::*;

use super::controller::*;
use super::dsp::DSP;
use super::graph::ProcessScope;


struct MediaReaderHandler<S: Sample> {
    // cache buffer
    cache: Producer<S>,
    // max cache size before fetching data
    fetch_at: usize,
    // fetch this sample count
    fetch_count: usize,
    // fetching
    fetching: bool
}

impl<S: Sample> MediaReaderHandler<S> {
    fn new(cache: Producer<S>) -> Self {
        let cap = cache.capacity();
        Self {
            cache: cache,
            fetch_at: (cap / 8),
            fetch_count: 1024*10,
            fetching: false,
        }
    }
}

impl<S: Sample> ReaderHandler for MediaReaderHandler<S> {
    type Sample = S;

    fn data_received(&mut self, buffer: &mut VecBuffer<S>) {
        self.cache.push_slice(buffer.as_slice());
        self.fetching = false;
    }

    fn poll(&mut self, reader: &mut Reader<Self::Sample>) -> Poll {
        if !self.fetching && self.cache.len() <= self.fetch_at as usize {
            reader.fetch(self.fetch_count, false);
            self.fetching = true;
        }
        Poll::Pending
    }
}


/// View over a media
#[foxlive_controller("media")]
pub struct MediaView<S,PS>
    where S: Sample+IntoControlValue,
          PS: ProcessScope,
{
    cache: Consumer<S>,
    /// Amplification
    amp: S,
    /// Stream information
    infos: StreamInfo,
    /// Reading position
    pos: NSamples,
    phantom: PhantomData<PS>,
}

impl<S,PS> MediaView<S,PS>
    where S: Sample+IntoControlValue,
          PS: ProcessScope,
{
    pub fn new<P>(path: P, rate: SampleRate, cache_duration: Duration)
        -> Result<(Self,Box<Future>),Error>
        where P: Into<String>
    {
        let path = path.into();
        Reader::open(&path, None, rate, None).map(|mut reader| {
            let infos = reader.stream().infos();
            let cache_size = ts_to_samples(cache_duration, rate) * infos.n_channels as NSamples;

            let (prod, cons) = RingBuffer::new(cache_size as usize).split();
            let handler = MediaReaderHandler::new(prod);
            reader.start_read(handler).unwrap();
            (Self {
                cache: cons,
                amp: S::identity(),
                pos: 0,
                infos: infos,
                phantom: PhantomData,
            }, reader.boxed())
        })
    }
}


impl<S,PS> DSP for MediaView<S,PS>
    where S: Sample+IntoControlValue,
          PS: ProcessScope,
{
    type Sample = S;
    type Scope = PS;

    fn process_audio(&mut self, scope: &Self::Scope, _input: Option<&dyn BufferView<Sample=Self::Sample>>,
                     output: Option<&mut dyn BufferView<Sample=Self::Sample>>)
    {
        let output = output.unwrap();
        // ensure output is interleaved data buffer, since reading does
        output.set_interleaved(true);

        let (cache, n_channels) = (&mut self.cache, self.infos.n_channels);
        let count = (cache.remaining() - cache.remaining() % n_channels as usize)
                    .min(output.len());
        let slice = output.as_slice_mut();

        let count = cache.pop_slice(&mut slice[0..count]);
        self.pos += count;
    }

    fn n_channels(&self) -> NChannels { self.infos.n_channels }
    fn is_source(&self) -> bool { true }
}


