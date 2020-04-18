//! Provide media file reader.
use std::ops::{Deref};
use std::ptr::null_mut;
use std::time::Duration;
use std::sync::*;

use core::pin::Pin;
use futures;
use ringbuf::Producer;

use crate::data::*;

use super::ffi;
use super::error::{Error,av_strerror};
use super::futures::*;

use super::codec::CodecContext;
use super::format::FormatContext;
use super::resampler::Resampler;
use super::stream::{Stream,StreamId};


pub struct ReaderContext<S>
    where S: Sample+Default+IntoSampleFmt+Unpin,
{
    pub format: FormatContext,
    pub codec: CodecContext,
    pub resampler: Resampler<S>,
    pub stream_id: StreamId,
    pub frame: *mut ffi::AVFrame,
    pub packet: *mut ffi::AVPacket,
}


impl<S> ReaderContext<S>
    where S: Sample+Default+IntoSampleFmt+Unpin,
{
    /// Create a new media reader.
    pub fn new(format: FormatContext, stream_id: Option<StreamId>, rate: SampleRate, layout: Option<ChannelLayout>)
        -> Result<Self,Error>
    {
        let stream = format.audio_stream(stream_id);
        if let Some(stream) = stream {
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

            let stream_id = stream.index;
            Ok(Self {
                format: format,
                stream_id: stream_id,
                codec: codec,
                resampler: resampler,
                frame: unsafe { ffi::av_frame_alloc() },
                packet: unsafe { ffi::av_packet_alloc() },
            })
        }
        else { Err(FmtError!(Reader, "no audio stream found")) }
    }
}


impl<S> Drop for ReaderContext<S>
    where S: Sample+Default+IntoSampleFmt+Unpin,
{
    fn drop(&mut self) {
        self.codec.send_packet(null_mut());

        if !self.frame.is_null() {
            unsafe { ffi::av_frame_free(&mut self.frame) };
        }

        if !self.packet.is_null() {
            unsafe { ffi::av_packet_free(&mut self.packet) }
        }
    }
}


impl<S> Deref for ReaderContext<S>
    where S: Sample+Default+IntoSampleFmt+Unpin,
{
    type Target = FormatContext;

    fn deref(&self) -> &Self::Target {
        &self.format
    }
}


/*
pub struct ReadFrame<S> {
    pub pos: Duration,
    pub count: u16,
    pub samples: [S;1024],
}
*/


/// Audio file reader, reading data in an interleaved buffer.
///
/// By itself it doesn't handle multithreading, but the provided
/// ReaderHandler can do the thing.
pub struct Reader<S>
    where S: Sample+Default+IntoSampleFmt+Unpin,
{
    context: Option<ReaderContext<S>>,
    cache: Producer<S>,
    buffer: VecBuffer<S>,
    rate: SampleRate,
    layout: Option<ChannelLayout>,
    stopped: bool,
}


impl<S> Reader<S>
    where S: Sample+Default+IntoSampleFmt+Unpin,
{
    /// Create a new media reader.
    pub fn new(cache: Producer<S>, rate: SampleRate, layout: Option<ChannelLayout>) -> Self
    {
        Self {
            context: None,
            cache: cache,
            buffer: VecBuffer::new(true, 1),
            rate: rate,
            layout: layout,
            stopped: false,
        }
    }

    /// Open file for reading, close previously opened file
    pub fn open(&mut self, path: &str, stream_id: Option<StreamId>) -> Result<(), Error> {
        if self.context.is_some() {
            self.close();
        }

        FormatContext::open_input(path)
            .and_then(|format| ReaderContext::new(format, stream_id, self.rate, self.layout))
            .and_then(|context| {
                self.context = Some(context);
                Ok(())
            })
    }

    pub fn close(&mut self) {
        if self.context.is_some() {
            self.buffer.clear();
            self.context = None;
        }
    }

    /// Stop reading forever, futures will `Poll::Ready(Ok())`. This should be
    /// used only when there is no more use of reader.
    pub fn stop(&mut self) {
        self.stopped = true;
    }

    /// Get current sample rate
    pub fn rate(&self) -> SampleRate {
        self.rate
    }

    /// Current stream being decoded
    pub fn stream<'a>(&'a self) -> Option<Stream<'a>> {
        if self.context.is_some() {
            let context = self.context.as_ref().unwrap();
            context.format.stream(context.stream_id)
        }
        else { None }
    }

    /// Poll reader.
    ///
    /// Panics if there is no assigned handler because once the reader becomes
    /// a future, there is no way to assign one.
    pub fn poll_once(&mut self) -> Poll {
        if self.stopped {
            Poll::Ready(Ok(()))
        }
        else if self.context.is_some() && self.cache.remaining() > self.cache.len() / 2 {
            pending_or_err(self.read_packet())
        }
        else { Poll::Pending }
    }

    /// Read a single packet
    fn read_packet(&mut self) -> Poll {
        let ctx = self.context.as_ref().unwrap();
        let r = unsafe { ffi::av_read_frame(ctx.format.context, ctx.packet) };
        if r >= 0 {
            let mut r = ctx.codec.send_packet(ctx.packet);
            if let Poll::Pending = r {
                r = self.receive_frame();
                if let Poll::Ready(Ok(_)) = r {
                    r = Poll::Pending;

                    // requested cache filled: send to handler and reset buffers
                    if self.buffer.len() >= 1024 {
                        self.data_received(true);
                    }
                }
            }
            let ctx = self.context.as_ref().unwrap();
            unsafe { ffi::av_packet_unref(ctx.packet); }
            r
        }
        else {
            self.data_received(false);
            ToPoll!(Reader, r)
        }
    }

    /// Data received, send handler and update self's stuff.
    fn data_received(&mut self, _has_more: bool) {
        /*
        let ctx = self.context.as_ref().unwrap();
        let frame = unsafe { *ctx.frame };
        let timebase = TimeBase::from(self.stream().unwrap().time_base);
        let pos_step = samples_to_ts(1024, self.rate);
        let end =  timebase.ts_to_duration(frame.pkt_pts + frame.pkt_duration);
        let chunks = if has_more { self.buffer.chunks_exact(1024) }
                     else { self.buffer.chunks(1024) }

        let mut pos = end_pos - pos_step * (chunks.len() as u32);
        let count = 0;
        for chunk in self.buffer.chunks(1024) {
            self.cache.push(ReadFrame {
                pos: pos,
                count: chunk.len(),
                // problem: data are copied when written to ringbuf
                data: Array::from(chunk),
            }
        }
        */


        let count = self.cache.push_slice(&self.buffer);
        if self.buffer.len() == count {
            self.buffer.clear();
        }
        else {
            self.buffer.drain(0..count).count();
        }
    }

    /// Receive a frame from codec, return `codec.receive_frame()` result.
    fn receive_frame(&mut self) -> Poll {
        let ctx = self.context.as_mut().unwrap();
        let r = ctx.codec.receive_frame(ctx.frame);
        if let Poll::Ready(Ok(_)) = r {
            let frame = unsafe { &*ctx.frame };
            ctx.resampler.convert(&mut self.buffer.buffer, frame);
        }
        r
    }

    /// Seek to position (as resampled position), returning seeked position
    /// in case of success.
    ///
    /// Internal buffer is cleared, but not shared cache which must be cleared
    /// manually.
    pub fn seek(&mut self, pos: Duration) -> Result<Duration, Error> {
        if let Some(ref ctx) = self.context {
            let tb = self.stream().unwrap().time_base;
            let real_pos = TimeBase::from((tb.num, tb.den)).duration_to_ts(pos);
            // 4 = AVSEEK_FLAG_ANY
            let r = unsafe { ffi::av_seek_frame(ctx.format.context, ctx.stream_id, real_pos, 4) };
            if r >= 0 {
                Ok(pos)
            }
            else {
                Err(Error::reader(av_strerror(r)))
            }
        }
        else { Err(Error::reader("not opened")) }
    }
}


impl<S> futures::Future for Reader<S>
    where S: Sample+Default+IntoSampleFmt+Unpin,
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


/// Arced reader with an rwlock in order to make it shareable around threads
#[derive(Clone)]
pub struct SharedReader<S>
    where S: Sample+Default+IntoSampleFmt+Unpin,
{
    pub reader: Arc<RwLock<Reader<S>>>,
}


impl<S> SharedReader<S>
    where S: Sample+Default+IntoSampleFmt+Unpin,
{
    pub fn new(cache: Producer<S>, rate: SampleRate, layout: Option<ChannelLayout>) -> Self {
        Self::from(Reader::new(cache, rate, layout))
    }

    pub fn read(&self) -> LockResult<RwLockReadGuard<Reader<S>>> {
        self.reader.read()
    }

    pub fn try_read(&self) -> TryLockResult<RwLockReadGuard<Reader<S>>> {
        self.reader.try_read()
    }

    pub fn write(&self) -> LockResult<RwLockWriteGuard<Reader<S>>> {
        self.reader.write()
    }

    pub fn try_write(&self) -> TryLockResult<RwLockWriteGuard<Reader<S>>> {
        self.reader.try_write()
    }
}

impl<S> From<Reader<S>> for SharedReader<S>
    where S: Sample+Default+IntoSampleFmt+Unpin,
{
    fn from(reader: Reader<S>) -> Self {
        Self { reader: Arc::new(RwLock::new(reader)) }
    }
}

impl<S> futures::Future for SharedReader<S>
    where S: Sample+Default+IntoSampleFmt+Unpin,
{
    type Output = PollValue;

    fn poll(self: Pin<&mut Self>, cx: &mut futures::task::Context) -> Poll {
        let reader = self.get_mut().reader.write();
        match reader {
            Ok(mut reader) => {
                let r = reader.poll_once();
                if let Poll::Pending = r {
                    cx.waker().clone().wake();
                }
                r
            },
            Err(_) => Poll::Ready(Err(Error::reader("reader poisoned"))),
        }
    }

}


