use std::marker::PhantomData;
use std::ops::Deref;

use super::ffi;
use super::media::{Media,MediaType};


/// Stream index
pub type StreamId = i32;


/// Wrapper around AVStream
pub struct Stream<'a> {
    stream: *mut ffi::AVStream,
    phantom: PhantomData<&'a Media>,
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

    // TODO:
    // - channel_layout
    // - n_channels
}


impl<'a> Deref for Stream<'a> {
    type Target = ffi::AVStream;

    fn deref(&self) -> &Self::Target {
        unsafe { self.stream.as_ref().unwrap() }
    }
}


/// Iterator over a `Media`'s streams
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
    type Item = Stream<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let stream = self.media.stream(self.id);
        self.id += 1;
        stream
    }
}



