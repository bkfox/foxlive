//! Provide media file reader.
use std::marker::{PhantomData,Unpin};
use std::ops::Deref;
use std::ptr::null_mut;

use core::pin::Pin;
use futures;

use crate::data::buffers::Buffers;
use crate::data::channels::*;
use crate::data::samples::{Sample,SampleRate};

use super::ffi;
use super::error::Error;
use super::futures::*;

use super::codec::CodecContext;
use super::format::FormatContext;
use super::resampler::Resampler;
use super::stream::{Stream,StreamId};


/// Data handler for a stream reader.
pub trait ReaderHandler : 'static+Unpin {
    type Sample;

    fn data_received(&mut self, stream: StreamId, buffer: &mut Buffers<Self::Sample>, poll: &mut Poll);
}


pub struct ClosureReaderHandler<S,F>
    where S: Sample,
          F: 'static+FnMut(StreamId, &mut Buffers<S>, &mut Poll),
{
    closure: F,
    phantom: PhantomData<S>
}

impl<S,F> ClosureReaderHandler<S,F>
    where S: Sample,
          F: 'static+FnMut(StreamId, &mut Buffers<S>, &mut Poll)
{
    pub fn new(closure: F) -> ClosureReaderHandler<S,F> {
        ClosureReaderHandler { closure: closure, phantom: PhantomData }
    }
}

impl<S,F> ReaderHandler for ClosureReaderHandler<S,F>
    where S: Sample,
          F: 'static+Unpin+FnMut(StreamId, &mut Buffers<S>, &mut Poll)
{
    type Sample = S;

    fn data_received(&mut self, stream: StreamId, buffer: &mut Buffers<S>, poll: &mut Poll) {
        (self.closure)(stream, buffer, poll)
    }
}



/// Read a single audio stream of a media file.
///
/// # Example
///
/// ```
/// let reader = Reader::open("test.mp3").unwrap();
/// let stream = Reader::read_audio(
///     // target sample rate and channel layout
///     48000, ChannelLayout::LayoutStereo,
///     ClosureReaderHandler::new(|stream_id, buffers: &mut Buffers<f32>, poll) {
///         // do something with buffers
///     })
/// ```
///
pub struct Reader<S,H>
    where S: Sample,
          H: ReaderHandler<Sample=S>
{
    pub format: FormatContext,
    reader: StreamReader<S>,
    handler: H,
    packet: *mut ffi::AVPacket,
}


impl<S,H> Reader<S,H>
    where S: Sample,
          H: ReaderHandler<Sample=S>
{
    /// Create a new media reader.
    pub fn new(format: FormatContext, stream_id: Option<StreamId>, rate: SampleRate, layout: Option<ChannelLayout>, handler: H)
        -> Result<Self,Error>
    {
        let stream = match stream_id {
            Some(stream_id) => format.stream(stream_id),
            None => format.streams().find(|s| s.media_type().is_audio())
        };

        match stream {
            Some(stream) => StreamReader::new(&stream, rate, layout)
                .and_then(|reader| Ok(Self {
                    format: format,
                    reader: reader,
                    handler: handler,
                    packet: unsafe { ffi::av_packet_alloc() },
                })),
            None => Err(FmtError!(Reader, "audio stream not found")),
        }
    }

    /// Create a new media reader for the provided file url
    pub fn open(path: &str, stream_id: Option<StreamId>, rate: SampleRate, layout: Option<ChannelLayout>, handler: H) -> Result<Self, Error> {
        FormatContext::open_input(path)
            .and_then(|format| Self::new(format, stream_id, rate, layout, handler))
    }

    /// Return object as boxed future
    pub fn boxed(self) -> Box<Future> {
        Box::new(self)
    }

    /// Return stream being decoded
    pub fn stream<'a>(&'a self) -> Stream<'a> {
        self.format.stream(self.reader.stream_id).unwrap()
    }

    /// Read one packet
    pub fn read_packet(&mut self) -> Poll {
        let r = unsafe { ffi::av_read_frame(self.format.context, self.packet) };
        if r >= 0 {
            let packet = unsafe { &*self.packet };
            if self.reader.stream_id != packet.stream_index as StreamId {
                return Poll::Pending;
            }

            let mut r = self.reader.send_packet(self.packet);
            if let Poll::Pending = r {
                r = self.reader.receive_frame();
                if let Poll::Ready(Ok(_)) = r {
                    r = Poll::Pending;
                    // a frame is available
                    self.handler.data_received(self.reader.stream_id, &mut self.reader.buffers, &mut r);
                }
            }
            unsafe { ffi::av_packet_unref(self.packet); }
            r
        }
        else {
            let mut r = ToPoll!(Reader, r);
            self.reader.send_packet(null_mut());
            self.handler.data_received(self.reader.stream_id, &mut self.reader.buffers, &mut r);
            r
        }
    }
}

impl<S,H> Drop for Reader<S,H>
    where S: Sample,
          H: ReaderHandler<Sample=S>
{
    fn drop(&mut self) {
        if !self.packet.is_null() {
            unsafe { ffi::av_packet_free(&mut self.packet) }
        }
    }
}

impl<S,H> Deref for Reader<S,H>
    where S: Sample,
          H: ReaderHandler<Sample=S>
{
    type Target = FormatContext;

    fn deref(&self) -> &Self::Target {
        &self.format
    }
}


impl<S,H> futures::Future for Reader<S,H>
    where S: Sample,
          H: ReaderHandler<Sample=S>
{
    type Output = PollValue;

    fn poll(self: Pin<&mut Self>, cx: &mut futures::task::Context) -> Poll {
        let r = self.get_mut().read_packet();
        if let Poll::Pending = r {
            cx.waker().clone().wake();
        }
        r
    }
}



/// Handle audio stream reading.
pub struct StreamReader<S>
    where S: Sample,
{
    pub stream_id: StreamId,
    codec: CodecContext,
    resampler: Resampler<S>,
    frame: *mut ffi::AVFrame,
    pub buffers: Buffers<S>,
}


impl<S> StreamReader<S>
    where S: Sample,
{
    /// Create new stream in order to read data
    pub fn new(stream: &Stream, rate: SampleRate, layout: Option<ChannelLayout>)
        -> Result<StreamReader<S>, Error>
    {
        if !stream.media_type().is_audio() {
            return Err(FmtError!(Codec, "Stream media type is not supported by this reader"))
        }

        let codec = match CodecContext::from_stream(stream) {
            Ok(context) => context,
            Err(err) => return Err(err),
        };

        let resampler = match Resampler::new(&codec, rate, layout) {
            Ok(resampler) => resampler,
            Err(e) => return Err(e),
        };

        let n_channels = codec.channels as usize;
        Ok(StreamReader {
            stream_id: stream.index,
            resampler: resampler,
            codec: codec,
            frame: unsafe { ffi::av_frame_alloc() },
            buffers: Buffers::with_capacity(n_channels as usize),
        })
    }

    /// Send a packet to stream codec
    fn send_packet(&mut self, packet: *mut ffi::AVPacket) -> Poll {
        self.codec.send_packet(packet)
    }

    /// Receive a frame from codec, return `codec.receive_frame()` result.
    fn receive_frame(&mut self) -> Poll {
        let r = self.codec.receive_frame(self.frame);
        if let Poll::Ready(Ok(_)) = r {
            let frame = unsafe { &*self.frame };
            self.resampler.convert(&mut self.buffers, frame);
        }
        r
    }
}


impl<S> Drop for StreamReader<S>
    where S: Sample,
{
    fn drop(&mut self) {
        if !self.frame.is_null() {
            unsafe { ffi::av_frame_free(&mut self.frame) };
        }
    }
}


