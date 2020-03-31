/// This module implements interfaces using ffmpeg corresponding to our
/// needs. We don't seek to offer an ffmpeg binding, just have tools
/// that we can use.
///
use std::collections::BTreeMap;
use std::ffi::{CString,CStr};
use std::fs::File;
use std::io::Read;
use std::iter::Iterator;
use std::ops::Deref;
use std::ptr::null_mut;
use std::slice;
use std::str;

use nix::errno::Errno;
use smallvec::SmallVec;

use utils::Flow;
use utils::buffers::{Buffer,Buffers,SharedBuffers,PreBuffer};

use utils::strerror;


#[allow(dead_code)]
#[allow(non_upper_case_globals)]
#[allow(non_camel_case_types)]
#[allow(non_snake_case)]
#[allow(bad_style)]
mod ffi;

#[allow(dead_code)]
pub mod consts;
#[macro_use]
pub mod utils;



// TODO Error Struct
#[derive(Debug)]
pub enum Error {
    File(String),
    Format(String),
    Codec(String),
    Decoder(String),
    Resampler(String),
}


/// Type of the media/stream
pub enum MediaType {
    Audio,
    Video,
    Subtitle,
    Data,
    Metadata,
    Unknown,
}



/// Must be called before using anything else in this module:
/// initialize ffmpeg
pub fn init() {
    unsafe { ffi::av_register_all() };
    unsafe { ffi::avcodec_register_all() };
}


/// Index of a stream
pub type StreamId = u32;

/// Wrapper around AVStream
pub struct Stream {
    // AVStream is owned by the parent FormatContext
    pub stream: *mut ffi::AVStream,
}

impl Stream {
    /// Create a new stream wrapping given AVStream
    pub fn new(stream: *mut ffi::AVStream) -> Stream {
        Stream {
            stream: stream,
        }
    }

    /// Return index of the stream
    pub fn id(&self) -> StreamId {
        unsafe { (*self.stream).index as StreamId }
    }

    /// Return Stream's media type
    pub fn media_type(&self) -> MediaType {
        let codecpar = unsafe { &*self.codecpar };
        match codecpar.codec_type {
            n if n == ffi::AVMediaType_AVMEDIA_TYPE_AUDIO => MediaType::Audio,
            n if n == ffi::AVMediaType_AVMEDIA_TYPE_VIDEO => MediaType::Video,
            n if n == ffi::AVMediaType_AVMEDIA_TYPE_SUBTITLE => MediaType::Subtitle,
            n if n == ffi::AVMediaType_AVMEDIA_TYPE_DATA => MediaType::Data,
            _ => MediaType::Unknown,
        }
    }

    /// Return codec id of the stream
    pub fn codec_id(&self) -> ffi::AVCodecID {
        let codecpar = unsafe { &*self.codecpar };
        codecpar.codec_id
    }
}

impl Deref for Stream
{
    type Target = ffi::AVStream;

    fn deref(&self) -> &Self::Target {
        if self.stream.is_null() {
            panic!("Wrapper can't be deref: need to be opened for.");
        }
        unsafe { &*self.stream }
    }
}


/// Iterator over streams of a media
pub struct StreamIter<'a> {
    id: StreamId,
    media: &'a Media
}

impl<'a> StreamIter<'a> {
    pub fn new(media: &Media) -> StreamIter {
        StreamIter {
            id: 0,
            media: media
        }
    }
}

impl<'a> Iterator for StreamIter<'a> {
    type Item = Stream;

    fn next(&mut self) -> Option<Self::Item> {
        let stream = self.media.stream(self.id);
        self.id += 1;
        stream
    }
}



/// Generic structure to read a media file.
/// Deref to the held AVFormatContext
pub struct Media {
    pub path: String,
    pub format: *mut ffi::AVFormatContext,
}

impl Media {
    /// Open input file with provided path
    pub fn open_input(&mut self, path: String) -> Result<(), Error> {
        let c_path = match CString::new(&path as &str) {
            Ok(path) => path,
            Err(_) => return Err(Error::File("invalid path (ffi::NulError)".to_string())),
        };

        unsafe {
            match ffi::avformat_open_input(&mut self.format, c_path.as_ptr(),
                                           null_mut(), null_mut())
            {
                n if n < 0 => Err(Error::Format(strerror(n))),
                _ =>
                    match ffi::avformat_find_stream_info(self.format, null_mut()) {
                        n if n < 0 => Err(Error::Format(strerror(n))),
                        _ => {
                            self.path = path;
                            Ok(())
                        }
                    }
            }
        }
    }

    /// Return media streams as an iterator
    pub fn streams(&self) -> StreamIter {
        StreamIter::new(&self)
    }

    /// Return a Stream for the given index
    pub fn stream(&self, id: StreamId) -> Option<Stream> {
        let format = unsafe { self.format.as_ref().unwrap() };
        if id >= format.nb_streams {
            return None
        }

        let streams = format.streams;
        Some(Stream::new(unsafe { *streams.offset(id as isize) }))
    }

    /// Reset and clean up
    fn reset(&mut self) {
        if !self.format.is_null() {
            unsafe { ffi::avformat_close_input(&mut self.format); }
        }
    }
}

impl Deref for Media
{
    type Target = ffi::AVFormatContext;

    fn deref(&self) -> &Self::Target {
        if self.format.is_null() {
            panic!("Can't be deref: self.format is null.");
        }
        unsafe { &*self.format }
    }
}

impl Drop for Media
{
    fn drop(&mut self) {
        self.reset();
    }
}



/// Resample packets
struct Resampler {
    swr: *mut ffi::SwrContext,
    src_rate: i32,
    dst_rate: i32,
    out_bufs: SmallVec<[*mut u8; 4]>,
}

impl Resampler {
    fn new(context: *const ffi::AVCodecContext, sample_rate: i32,
               layout: i64)
        -> Result<Resampler,Error>
    {
        if context.is_null() {
            panic!("codec context is null");
        }

        let swr = unsafe {
            let c = *context;
            let swr = ffi::swr_alloc_set_opts(null_mut(),
                layout, ffi::AVSampleFormat_AV_SAMPLE_FMT_FLTP, sample_rate,
                c.channel_layout as i64, c.sample_fmt, c.sample_rate,
                0, null_mut()
            );

            let r = ffi::swr_init(swr);
            if r < 0 {
                return Err(Error::Resampler(strerror(r)));
            }
            swr
        };

        Ok(Resampler {
            swr: swr,
            src_rate: unsafe { (*context).sample_rate },
            dst_rate: sample_rate,
            out_bufs: SmallVec::new(),
        })
    }

    /// Convert given frame into output buffers
    fn convert(&mut self, out: &mut Buffers, frame: &ffi::AVFrame) {
        let src_nb_samples = frame.nb_samples;

        // destination number of samples
        let dst_nb_samples = unsafe { ffi::av_rescale_rnd(
            ffi::swr_get_delay(self.swr, self.src_rate as i64) +
                src_nb_samples as i64,
            self.dst_rate as i64, self.src_rate as i64,
            ffi::AVRounding_AV_ROUND_UP
        )};

        // resize
        self.out_bufs.clear();
        for buffer in &mut out.iter_mut() {
            let n = buffer.len();
            buffer.resize(n + dst_nb_samples as usize, 0.0);
            let offset = unsafe { buffer.as_mut_ptr().offset(n as isize) };
            self.out_bufs.push(offset as *mut u8);
        }

        // convert
        unsafe { ffi::swr_convert(
            self.swr, self.out_bufs.as_mut_ptr(), dst_nb_samples as i32,
            frame.extended_data as *mut *const u8, src_nb_samples
        )};
    }
}

impl Drop for Resampler {
    fn drop(&mut self) {
        if !self.swr.is_null() {
            unsafe { ffi::swr_free(&mut self.swr) };
        }
    }
}


/// Hold context for a given stream.
struct StreamContext {
    id: StreamId,
    resampler: Resampler,
    context: *mut ffi::AVCodecContext,
    frame: *mut ffi::AVFrame,
    buffers: PreBuffer,
}


impl StreamContext {
    fn new(stream: &Stream, sample_rate: i32, layout: i64,
           buffers: SharedBuffers)
        -> Result<StreamContext, Error>
    {
        // init codec context
        let codec_id = stream.codec_id();
        let codec = unsafe { ffi::avcodec_find_decoder(codec_id) };
        if codec.is_null() {
            return Error!(Codec, "decoder not found for codec {}", codec_id);
        }

        let context = stream.codec;
        match unsafe { ffi::avcodec_open2(context, codec, null_mut()) } {
            r if r < 0 => return Err(Error::Codec(strerror(r))),
            _ => {},
        };

        // init resampler
        let resampler = match Resampler::new(context, sample_rate, layout) {
            Ok(resampler) => resampler,
            Err(e) => return Err(e),
        };

        Ok(StreamContext {
            id: stream.index as StreamId,
            resampler: resampler,
            context: context,
            frame: unsafe { ffi::av_frame_alloc() },
            buffers: PreBuffer::new(
                unsafe { (*context).channels as usize },
                buffers
            ),
        })
    }

    /// Decode a single packet into self's buffers
    fn decode_packet(&mut self, packet: *mut ffi::AVPacket)
        -> Flow<Error>
    {
        let r = unsafe { ffi::avcodec_send_packet(
            self.context, packet,
        )};
        if r < 0 {
            return ToFlow!(Decoder, r);
        }

        loop {
            let r = unsafe { ffi::avcodec_receive_frame(self.context, self.frame) };
            if r == 0 {
                let frame = & unsafe { *self.frame };
                self.resampler.convert(&mut self.buffers.caches, frame);
                continue;
            }
            return ToFlow!(Decoder, r);
        };
    }
}

impl Drop for StreamContext {
    fn drop(&mut self) {
        if !self.context.is_null() {
            unsafe { ffi::avcodec_free_context(&mut self.context) };
        }

        if !self.frame.is_null() {
            unsafe { ffi::av_frame_free(&mut self.frame); }
        }
    }
}


/// Decode a stream data
pub struct Decoder<'a> {
    media: &'a Media,
    /// Context for streams per id
    streams: BTreeMap<StreamId, StreamContext>,
    /// packet
    packet: *mut ffi::AVPacket,
}

impl<'a> Decoder<'a> {
    pub fn new (media: &Media) -> Decoder
    {
        Decoder {
            media: media,
            streams: BTreeMap::new(),
            packet: unsafe { ffi::av_packet_alloc() },
        }
    }

    /// Add a new stream to decode. Must be called before `decode()`
    pub fn add(&mut self, stream: &Stream, sample_rate: i32, layout: i64,
               buffers: SharedBuffers) -> Result<StreamId, Error>
    {
        StreamContext::new(stream, sample_rate, layout, buffers)
            .map(|s| { let id = s.id; self.streams.insert(s.id, s); id } )
    }

    /// Decode a single packet and put result into the correct stream
    /// Return Ok(true) if this function should be called again
    pub fn decode(&mut self) -> Flow<Error> {
        // TODO: reserve buffers
        // TODO: handle end of file with av_read_frame
        let r = unsafe { ffi::av_read_frame(self.media.format, self.packet) };
        if r < 0 {
            return ToFlow!(Decoder, r);
        }

        let packet = & unsafe { *self.packet };
        let stream_id = packet.stream_index as StreamId;
        match self.streams.get_mut(&stream_id) {
            Some(ref mut stream) => {
                let r = stream.decode_packet(self.packet);
                stream.buffers.flush(false);
                r
            },
            None => Flow::Next,
        }
    }

    /// Finalize decoding, flush buffers, reset streams, etc.
    pub fn finalize(mut self) {
        let packet = &mut unsafe { *self.packet };
        packet.data = null_mut();
        packet.size = 0;

        for (id, stream) in &mut self.streams {
            // FIXME: packet's stream_id match?
            stream.decode_packet(self.packet);
            stream.buffers.flush(true);
        }

        // release contexts
        self.streams.clear();
    }
}

impl<'a> Drop for Decoder<'a>
{
    fn drop(&mut self) {
        if !self.packet.is_null() {
            unsafe { ffi::av_packet_free(&mut self.packet) };
        }
    }
}




