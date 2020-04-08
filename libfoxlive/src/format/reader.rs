//! Provide media file reader.
use std::ops::Deref;
use std::ptr::null_mut;
use std::time::Duration;

use core::pin::Pin;
use futures;

use crate::data::*;

use super::ffi;
use super::error::{Error,av_strerror};
use super::futures::*;

use super::codec::CodecContext;
use super::format::FormatContext;
use super::resampler::Resampler;
use super::stream::{Stream,StreamId};


/// Handler a stream reader.
pub trait ReaderHandler : 'static+Unpin {
    type Sample: Sample;

    /// Data have been received
    fn data_received(&mut self, buffer: &mut VecBuffer<Self::Sample>);

    /// Called at each reader's poll in order to do stuff (seek, request data, etc)
    /// It returns Poll::Ready when Reader is no needed anymore.
    fn poll(&mut self, reader: &mut Reader<Self::Sample>) -> Poll;
}


/// Audio file reader, reading data in an interleaved buffer.
///
/// By itself it doesn't handle multithreading, but the provided
/// ReaderHandler can do the thing.
pub struct Reader<S>
    where S: Sample,
{
    pub format: FormatContext,
    pub buffer: VecBuffer<S>,
    stream_id: StreamId,
    fetch_count: NSamples,
    codec: CodecContext,
    resampler: Resampler<S>,
    frame: *mut ffi::AVFrame,
    packet: *mut ffi::AVPacket,
    handler: Option<Box<dyn ReaderHandler<Sample=S>>>,
}


impl<S> Reader<S>
    where S: Sample,
{
    /// Create a new media reader.
    pub fn new(format: FormatContext, stream_id: Option<StreamId>, rate: SampleRate, layout: Option<ChannelLayout>)
        -> Result<Self,Error>
    {
        let stream = match stream_id {
            Some(stream_id) => format.stream(stream_id),
            None => format.streams().find(|s| s.media_type().is_audio())
        };

        if let Some(stream) = stream {
            if !stream.media_type().is_audio() {
                return Err(FmtError!(Reader, "Stream is not audio"))
            }

            // discard unused streams
            for stream in format.streams() {
                if stream.index != stream.index {
                    stream.set_discard(ffi::AVDiscard_AVDISCARD_ALL);
                }
            }

            let codec = match CodecContext::from_stream(&stream) {
                Ok(context) => context,
                Err(err) => return Err(err),
            };

            let resampler = match Resampler::new(&codec, rate, layout) {
                Ok(resampler) => resampler,
                Err(e) => return Err(e),
            };

            let n_channels = codec.channels as NChannels;
            let stream_id = stream.index;
            Ok(Self {
                format: format,
                buffer: VecBuffer::new(true, n_channels),
                stream_id: stream_id,
                fetch_count: 0,
                codec: codec,
                resampler: resampler,
                frame: unsafe { ffi::av_frame_alloc() },
                packet: unsafe { ffi::av_packet_alloc() },
                handler: None,
            })
        }
        else { Err(FmtError!(Reader, "audio stream not found")) }
    }

    /// Create a new media reader for the provided file url
    pub fn open(path: &str, stream_id: Option<StreamId>, rate: SampleRate, layout: Option<ChannelLayout>) -> Result<Self, Error> {
        FormatContext::open_input(path)
            .and_then(|format| Self::new(format, stream_id, rate, layout))
    }

    /// Return object as boxed future
    pub fn boxed(self) -> Box<Future> {
        Box::new(self)
    }

    /// Current stream being decoded
    pub fn stream<'a>(&'a self) -> Stream<'a> {
        self.format.stream(self.stream_id).unwrap()
    }

    /// Start reading by providing a reader
    pub fn start_read(&mut self, handler: impl ReaderHandler<Sample=S>) -> Result<(), Error> {
        if self.handler.is_some() {
            Err(Error::reader("already reading"))
        }
        else {
            self.handler = Some(Box::new(handler));
            Ok(())
        }
    }

    /// Poll
    pub fn poll_once(&mut self) -> Poll {
        let handler = self.handler.take();
        let r = match handler {
            None => Poll::Pending,
            Some(mut handler) => {
                let r = handler.poll(self);
                self.handler = Some(handler);
                if let Poll::Pending = r {
                    if self.fetch_count == 0 {
                        Poll::Pending
                    }
                    else {
                        self.read_packet()
                    }
                }
                else { r }
            }
        };
        r
    }

    /// Read a single packet
    pub fn read_packet(&mut self) -> Poll {
        let r = unsafe { ffi::av_read_frame(self.format.context, self.packet) };
        if r >= 0 {
            let mut r = self.codec.send_packet(self.packet);
            if let Poll::Pending = r {
                r = self.receive_frame();
                if let Poll::Ready(Ok(_)) = r {
                    r = Poll::Pending;

                    // requested cache filled: send to handler and reset buffers
                    if self.buffer.n_samples() >= self.fetch_count {
                        self.data_received();
                    }
                }
            }
            unsafe { ffi::av_packet_unref(self.packet); }
            r
        }
        else {
            // close codec (FIXME: what if we seek afterward?) and send the remaining buffer data
            self.codec.send_packet(null_mut());
            self.data_received();
            ToPoll!(Reader, r)
        }
    }

    /// Data received, send handler and update self's stuff.
    fn data_received(&mut self) {
        // pos = self.frame.pkt_pts + self.frame.pkt_duration
        // timebase = TimeBase::from(self.stream().time_base)
        // pos = timebase.ts_to_duration(self.frame.pkt_pts + self.frame.pkt_duration)
        let handler = self.handler.as_mut();
        handler.unwrap().data_received(&mut self.buffer);
        self.fetch_count = 0;
        self.buffer.clear();
    }

    /// Receive a frame from codec, return `codec.receive_frame()` result.
    fn receive_frame(&mut self) -> Poll {
        let r = self.codec.receive_frame(self.frame);
        if let Poll::Ready(Ok(_)) = r {
            let frame = unsafe { &*self.frame };
            self.resampler.convert(&mut self.buffer.buffer, frame);
        }
        r
    }

    /// Request provided sample count (added to current count if add is `true`).
    pub fn fetch(&mut self, count: NSamples, add: bool) {
        match add {
            true => self.fetch_count += count,
            false => self.fetch_count = count,
        }
    }

    /// Seek to position (as resampled position), returning seeked position
    /// in case of success.
    pub fn seek(&mut self, pos: Duration) -> Result<Duration, Error> {
        let tb = self.stream().time_base;
        let real_pos = TimeBase::from((tb.num, tb.den)).duration_to_ts(pos);
        // 4 = AVSEEK_FLAG_ANY
        let r = unsafe { ffi::av_seek_frame(self.format.context, self.stream_id, real_pos, 4) };
        if r >= 0 {
            Ok(pos)
        }
        else {
            Err(Error::reader(av_strerror(r)))
        }
    }
}

impl<S> Drop for Reader<S>
    where S: Sample,
{
    fn drop(&mut self) {
        if !self.frame.is_null() {
            unsafe { ffi::av_frame_free(&mut self.frame) };
        }

        if !self.packet.is_null() {
            unsafe { ffi::av_packet_free(&mut self.packet) }
        }
    }
}

impl<S> Deref for Reader<S>
    where S: Sample,
{
    type Target = FormatContext;

    fn deref(&self) -> &Self::Target {
        &self.format
    }
}


impl<S> futures::Future for Reader<S>
    where S: Sample,
{
    type Output = PollValue;

    fn poll(self: Pin<&mut Self>, cx: &mut futures::task::Context) -> Poll {
        let r = self.get_mut().poll_once();
        if let Poll::Pending = r {
            cx.waker().clone().wake();
        }
        r
    }
}


