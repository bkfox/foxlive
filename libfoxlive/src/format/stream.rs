use std::marker::PhantomData;
use std::ops::Deref;

use crate::data::{Duration,NChannels,SampleRate,TimeBase};

use super::ffi;
use super::format::FormatContext;


/// Type of the stream
pub enum MediaType {
    Audio,
    Video,
    Subtitle,
    Data,
    Metadata,
    Unknown,
}


impl MediaType {
    /// MediaType from FFMPEG's AVMediaType
    pub fn from_av(media_type: ffi::AVMediaType) -> MediaType {
        match media_type {
            ffi::AVMediaType_AVMEDIA_TYPE_AUDIO => MediaType::Audio,
            ffi::AVMediaType_AVMEDIA_TYPE_VIDEO => MediaType::Video,
            ffi::AVMediaType_AVMEDIA_TYPE_SUBTITLE => MediaType::Subtitle,
            ffi::AVMediaType_AVMEDIA_TYPE_DATA => MediaType::Data,
            _ => MediaType::Unknown,
        }
    }

    pub fn is_audio(&self) -> bool {
        if let MediaType::Audio = self { true }
        else { false }
    }

    pub fn is_video(&self) -> bool {
        if let MediaType::Video = self { true }
        else{ false }
    }

    pub fn is_subtitle(&self) -> bool {
        if let MediaType::Subtitle = self { true }
        else{ false }
    }

    pub fn is_data(&self) -> bool {
        if let MediaType::Data = self { true }
        else{ false }
    }

    pub fn is_metadata(&self) -> bool {
        if let MediaType::Metadata = self { true }
        else{ false }
    }

    pub fn is_unknown(&self) -> bool {
        if let MediaType::Unknown = self { true }
        else{ false }
    }
}


/// Stream information
pub struct StreamInfo {
    pub n_channels: NChannels,
    pub rate: SampleRate,
    pub duration: Duration,
}


/// Stream index
pub type StreamId = i32;


/// Wrapper around AVStream.
pub struct Stream<'a> {
    stream: *mut ffi::AVStream,
    phantom: PhantomData<&'a FormatContext>,
}


impl<'a> Stream<'a> {
    pub fn new(stream: *mut ffi::AVStream) -> Stream<'a> {
        Stream {
            stream: stream,
            phantom: PhantomData,
        }
    }

    /// Codec parameters as reference
    pub fn codecpar(&'a self) -> &'a ffi::AVCodecParameters {
        unsafe { &*self.codecpar }
    }

    /// Shortcut to codec id
    pub fn codec_id(&self) -> ffi::AVCodecID {
        self.codecpar().codec_id
    }

    /// Stream codec type
    pub fn media_type(&self) -> MediaType {
        MediaType::from_av(self.codecpar().codec_type)
    }

    /// Number of channels
    pub fn n_channels(&self) -> NChannels {
        self.codecpar().channels as NChannels
    }

    /// Stream duration
    pub fn duration(&self) -> Duration {
        TimeBase::from((self.time_base.num, self.time_base.den))
            .ts_to_duration(unsafe { (*self.stream).duration })
    }

    /// Stream information as plain struct
    pub fn infos(&self) -> StreamInfo {
        StreamInfo {
            n_channels: self.n_channels(),
            rate: self.codecpar().sample_rate,
            duration: self.duration(),
        }
    }

    /// Set stream's discard
    pub fn set_discard(&self, discard: ffi::AVDiscard) {
        unsafe { (*self.stream).discard = discard; }
    }
}


impl<'a> Deref for Stream<'a> {
    type Target = ffi::AVStream;

    fn deref(&self) -> &Self::Target {
        unsafe { self.stream.as_ref().unwrap() }
    }
}


/// Iterator over a `Media`'s streams
pub struct StreamIter<'a> {
    format: &'a FormatContext,
    id: StreamId,
}

impl<'a> StreamIter<'a> {
    pub fn new(format: &'a FormatContext) -> Self {
        Self { format: format, id: 0, }
    }
}

impl<'a> Iterator for StreamIter<'a> {
    type Item = Stream<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let stream = self.format.stream(self.id);
        self.id += 1;
        stream
    }
}

