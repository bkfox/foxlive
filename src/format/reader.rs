use std::marker::PhantomData;
use std::ops::Deref;
use std::ptr::null_mut;
use std::collections::BTreeMap;

use futures;

use crate::data::buffers::Buffers;
use crate::data::samples::{Sample,SampleRate};
use crate::data::channels::ChannelLayout;

use super::ffi;
use super::error::Error;
use super::utils::*;

use super::codec::CodecContext;
use super::resampler::Resampler;
use super::media::Media;
use super::stream::{Stream,StreamId};


pub trait StreamReader {
    fn stream_id(&self) -> StreamId;

    /// Send packet to codec and read its content
    fn send_packet(&mut self, packet: *mut ffi::AVPacket) -> Poll;
}


/// Data handler for a stream reader.
pub trait ReaderHandler {
    type Buffer;

    fn data_received(&mut self, stream: StreamId, buffer: &mut Self::Buffer, poll: &Poll);
}


pub struct ClosureReaderHandler<B,F>
    where F: 'static+FnMut(StreamId, &mut B, &Poll)
{
    closure: F,
    phantom: PhantomData<B>
}

impl<B,F> ClosureReaderHandler<B,F>
    where F: 'static+FnMut(StreamId, &mut B, &Poll)
{
    pub fn new(closure: F) -> ClosureReaderHandler<B,F> {
        ClosureReaderHandler { closure: closure, phantom: PhantomData }
    }
}

impl<B,F> ReaderHandler for ClosureReaderHandler<B,F>
    where F: 'static+FnMut(StreamId, &mut B, &Poll)
{
    type Buffer = B;

    fn data_received(&mut self, stream: StreamId, buffer: &mut Self::Buffer, poll: &Poll) {
        (self.closure)(stream, buffer, poll)
    }
}


/// Stream reader for audio, decoding and resampling packets provided by MediaReader.
/// The provided handlers can access and reset buffers.
pub struct AudioStreamReader<S, H>
    where S: Sample,
          H: ReaderHandler<Buffer=Buffers<S>>
{
    stream_id: StreamId,
    codec: CodecContext,
    resampler: Resampler<S>,
    frame: *mut ffi::AVFrame,
    pub buffers: Buffers<S>,
    pub handler: H,
}


impl<S,H> AudioStreamReader<S,H>
    where S: Sample,
          H: ReaderHandler<Buffer=Buffers<S>>
{
    /// Create new stream in order to read data
    pub fn new(stream: &Stream, sample_rate: SampleRate, channel_layout: ChannelLayout,
               handler: H)
        -> Result<AudioStreamReader<S,H>, Error>
    {
        if !stream.media_type().is_audio() {
            return Err(FmtError!(Codec, "Stream media type is not supported by this reader"))
        }

        let codec = match CodecContext::from_stream(stream) {
            Ok(context) => context,
            Err(err) => return Err(err),
        };

        let resampler = match Resampler::new(&codec, sample_rate, channel_layout) {
            Ok(resampler) => resampler,
            Err(e) => return Err(e),
        };

        Ok(AudioStreamReader {
            stream_id: stream.index,
            resampler: resampler,
            codec: codec,
            frame: unsafe { ffi::av_frame_alloc() },
            buffers: Buffers::with_capacity(codec.channels as usize),
            handler: handler,
        })
    }
}


impl<S,H> Drop for AudioStreamReader<S,H>
    where S: Sample,
          H: ReaderHandler<Buffer=Buffers<S>>
{
    fn drop(&mut self) {
        if !self.frame.is_null() {
            unsafe { ffi::av_frame_free(&mut self.frame) };
        }
    }
}


impl<S,H> StreamReader for AudioStreamReader<S,H>
    where S: Sample,
          H: ReaderHandler<Buffer=Buffers<S>>
{
    fn stream_id(&self) -> StreamId {
        self.stream_id
    }

    fn send_packet(&mut self, packet: *mut ffi::AVPacket) -> Poll {
        let mut r = self.codec.send_packet(packet);
        r = match r {
            Poll::Pending => {
                loop {
                    r = self.codec.receive_frame(self.frame);
                    match r {
                        Poll::Pending => {
                            let frame = unsafe { &*self.frame };
                            self.resampler.convert(&mut self.buffers, frame);
                        },
                        Poll::Ready(r) => break,
                    }
                }
                r
            },
            Poll::Ready(r) => Poll::Ready(r)
        };

        self.handler.data_received(self.stream_id, &mut self.buffers, &r);
        r
    }
}



pub struct MediaReader {
    pub media: Media,
    readers: BTreeMap<StreamId, Box<dyn StreamReader>>,
    packet: *mut ffi::AVPacket,
}


impl MediaReader {
    /// Create a new media reader.
    pub fn new(media: Media) -> MediaReader {
        MediaReader {
            media: media,
            readers: BTreeMap::new(),
            packet: unsafe { ffi::av_packet_alloc() },
        }
    }

    /// Create a new media reader for the provided file url
    pub fn open(path: &str) -> Result<Self, Error> {
        Media::open_input(path).and_then(|media| Ok(Self::new(media)))
    }

    /// Return stream reader by id
    pub fn reader(&self, id: StreamId) -> Option<&Box<dyn StreamReader>> {
        self.readers.get(&id)
    }

    /// Return mutable stream reader by id
    pub fn reader_mut(&mut self, id: StreamId) -> Option<&mut Box<dyn StreamReader>> {
        self.readers.get_mut(&id)
    }

    /// Return all stream readers
    pub fn readers<'a>(&'a self) -> &BTreeMap<StreamId, Box<dyn StreamReader>> {
        &self.readers
    }

    /// Mark a stream to be read by the specified StreamReader.
    pub fn read_stream<T>(&mut self, reader: T)
        -> Result<&Box<dyn StreamReader>, Error>
        where T: 'static + StreamReader
    {
        let id = reader.stream_id();
        match self.media.stream(id) {
            None => Err(FmtError!(Generic, "Stream {} not found", id)),
            Some(_) => {
                self.readers.insert(id, Box::new(reader));
                Ok(self.readers.get(&id).unwrap())
            }
        }
    }

    /// Read an audio stream
    pub fn read_audio_stream<S,H>(&mut self, stream: StreamId, sample_rate: SampleRate, channel_layout: ChannelLayout, handler: H)
        -> Result<&Box<dyn StreamReader>, Error>
        where S: 'static + Sample,
              H: 'static + ReaderHandler<Buffer=Buffers<S>>,
    {
        let stream = self.stream(stream).unwrap();
        let reader = AudioStreamReader::new(&stream, sample_rate, channel_layout, handler);
        match reader {
            Ok(reader) => self.read_stream(reader),
            Err(err) => Err(err),
        }
    }

    /// Read

    /// Read packets from file
    pub fn poll(&mut self) -> Poll {
        let r = unsafe { ffi::av_read_frame(self.media.format, self.packet) };
        if r >= 0 {
            let packet = unsafe { &*self.packet };
            let stream_id = packet.stream_index as StreamId;
            let r = match self.readers.get_mut(&stream_id) {
                /// FIXME: at stream end, remove it and poll::ready only if ok
                /// FIXME: does stream error cancels the entire decoding?
                Some(ref mut stream) => stream.send_packet(self.packet)
                ,
                None => Poll::Pending,
            };
            unsafe { ffi::av_packet_unref(self.packet); }
            r
        }
        else {
            self.finalize();
            ToPoll!(Decoder, r)
        }
    }

    /// Finalize decoding
    fn finalize(&mut self) {
        let packet = &mut unsafe { *self.packet };
        packet.data = null_mut();
        packet.size = 0;

        for reader in &mut self.readers.values_mut() {
            reader.send_packet(self.packet);
        }

        // release contexts
        self.readers.clear();
    }
}

impl Drop for MediaReader {
    fn drop(&mut self) {
        if !self.packet.is_null() {
            unsafe { ffi::av_packet_free(&mut self.packet) }
        }
    }
}

impl Deref for MediaReader {
    type Target = Media;

    fn deref(&self) -> &Self::Target {
        &self.media
    }
}


