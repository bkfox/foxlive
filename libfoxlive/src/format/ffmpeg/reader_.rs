use std::marker::PhantomData;
use std::ops::Deref;
use std::ptr::null_mut;
use std::collections::BTreeMap;

use futures;
use nix::errno::Errno;

use crate::data::buffers::Buffers;
use crate::data::samples::{Sample,SampleRate};
use crate::data::channels::ChannelLayout;

use super::ffi;
use super::error::Error;
use super::resampler::Resampler;
use super::media::Media;
use super::stream::{Stream,StreamId};


pub type Poll = futures::task::Poll<Result<(), Error>>;


/// Return a Poll<Result<(), Error>> from provided ffmpeg function result
macro_rules! ToPoll {
    ($err:ident, $r: ident) => {{
        // EOF
        if $r == -541478725 {
            return Poll::Ready(Ok(()));
        }

        // cf. AVERROR macros definitions
        let err = Errno::from_i32(if Errno::EDOM as i32 > 0 { -$r }
                                  else { $r });
        match err {
            Errno::EAGAIN => Poll::Pending,
            _ => Poll::Ready(Err(AVError!($err, $r))),
        }
    }}
}


pub trait StreamReader {
    /// Send packet to codec and read its content
    fn send_packet(&mut self, packet: *mut ffi::AVPacket) -> Poll;
}


/// Data handler for a stream reader.
pub trait ReaderHandler {
    type Reader: StreamReader;

    fn data_received(&mut self, reader: &mut Self::Reader, poll: &Poll);
}


/// Implements AudioReaderHandler for a closure
impl<R> ReaderHandler for dyn Fn(&mut R, &Poll)
    where R: StreamReader
{
    type Reader = R;

    fn data_received(&mut self, reader: &mut Self::Reader, poll: &Poll) {
        self(reader, poll)
    }
}

/// Implements AudioReaderHandler for a mutable closure
impl<R> ReaderHandler for dyn FnMut(&mut R, &Poll)
    where R: StreamReader
{
    type Reader = R;

    fn data_received(&mut self, reader: &mut Self::Reader, poll: &Poll) {
        self(reader, poll)
    }
}


/// Stream reader for audio, decoding and resampling packets provided by MediaReader.
/// The provided handlers can access and reset buffers.
pub struct AudioStreamReader<S, H>
    where S: Sample,
          H: ReaderHandler<Reader=Self>
{
    pub stream: StreamId,
    pub buffers: Buffers<S>,
    pub handler: H,
    codec_context: *mut ffi::AVCodecContext,
    frame: *mut ffi::AVFrame,
    resampler: Resampler<S>,
}


impl<S,H> AudioStreamReader<S,H>
    where S: Sample,
          H: ReaderHandler<Reader=Self>
{
    /// Create new stream in order to read data
    pub fn new(stream: &Stream, sample_rate: SampleRate, channel_layout: ChannelLayout,
               handler: H)
        -> Result<AudioStreamReader<S,H>, Error>
    {
        if !stream.media_type().is_audio() {
            return Err(FmtError!(Codec, "Stream media type is not supported by this reader"))
        }

        let codec = unsafe { ffi::avcodec_find_decoder(stream.codec_id()) };
        if codec.is_null() {
            return Err(FmtError!(Codec, "no codec found for codec id {}", stream.codec_id()));
        }

        // FIXME: stream.codec is deprecated, however, using avcodec_alloc_context3 does not
        //        provides context.sample_rate and maybe other values
        let codec_context = stream.codec; // unsafe { ffi::avcodec_alloc_context3(codec) };
        if codec_context.is_null() {
            return Err(FmtError!(Codec, "can not allocate codec context"));
        }

        match unsafe { ffi::avcodec_open2(codec_context, codec, null_mut()) } {
            r if r < 0 => return Err(AVError!(Codec, r)),
            _ => {},
        };

        // init resampler
        let resampler = match Resampler::new(codec_context, sample_rate, channel_layout) {
            Ok(resampler) => resampler,
            Err(e) => return Err(e),
        };

        Ok(AudioStreamReader {
            stream: stream.index,
            buffers: Buffers::with_capacity(unsafe { (*codec_context).channels as usize }),
            handler: handler,
            resampler: resampler,
            codec_context: codec_context,
            frame: unsafe { ffi::av_frame_alloc() },
        })
    }
}


impl<S,H> Drop for AudioStreamReader<S,H>
    where S: Sample,
          H: ReaderHandler<Reader=Self>
{
    fn drop(&mut self) {
        if !self.frame.is_null() {
            unsafe { ffi::av_frame_free(&mut self.frame) };
        }

        if !self.codec_context.is_null() {
            unsafe { ffi::avcodec_free_context(&mut self.codec_context); }
        }
    }
}


impl<S,H> StreamReader for AudioStreamReader<S,H>
    where S: Sample,
          H: ReaderHandler<Reader=Self>
{
    fn send_packet(&mut self, packet: *mut ffi::AVPacket) -> Poll {
        let mut r = unsafe { ffi::avcodec_send_packet(self.codec_context, packet) };
        let poll =
            if r != 0 {
                ToPoll!(Codec, r)
            }
            else {
                loop {
                    r = unsafe { ffi::avcodec_receive_frame(self.codec_context, self.frame) };
                    if r == 0 {
                        let frame = & unsafe { *self.frame };
                        self.resampler.convert(&mut self.buffers, frame);
                        continue;
                    }
                    break;
                }
                ToPoll!(Decoder, r)
            };

        self.handler.data_received(self, &poll);
        poll
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
    pub fn read_stream<T>(&mut self, id: StreamId, reader: T)
        -> Result<&Box<dyn StreamReader>, Error>
        where T: 'static + StreamReader
    {
        match self.media.stream(id) {
            None => Err(FmtError!(Generic, "Stream {} not found", id)),
            Some(_) => {
                self.readers.insert(id, Box::new(reader));
                Ok(self.readers.get(&id).unwrap())
            }
        }
    }

    /// Mark an audio stream to be decoded.
    pub fn read_audio_stream<S, H>(&mut self, stream: StreamId, sample_rate: SampleRate, channel_layout: ChannelLayout, handler: H)
        -> Result<&Box<dyn StreamReader>, Error>
        where S: 'static + Sample,
              H: ReaderHandler<Reader=AudioStreamReader<S,H>>
    {
        let stream = self.stream(stream).unwrap();
        let reader = AudioStreamReader::<S,H>::new(&stream, sample_rate, channel_layout, handler);
        match reader {
            Ok(reader) => self.read_stream(stream.index, reader),
            Err(err) => Err(err),
        }
    }

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


