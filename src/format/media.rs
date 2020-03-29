use std::ffi::CString;
use std::ptr::null_mut;

use super::error::Error;
use super::ffi;
use super::stream::{Stream,StreamId,StreamIter};


/// Type of the media/stream
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




/// An interface to manipulate media files.
///
/// Deref to the held AVFormatContext
pub struct Media {
    pub format: *mut ffi::AVFormatContext,
}

impl Media {
    /// Open input file with provided path
    pub fn open_input(path: &str) -> Result<Self, Error> {
        let c_path = match CString::new(path) {
            Ok(path) => path,
            Err(_) => return Err(Error::File("invalid path (ffi::NulError)".to_string())),
        };

        let mut format = null_mut();
        let mut r = unsafe { ffi::avformat_open_input(&mut format, c_path.as_ptr(), null_mut(), null_mut()) };
        if r >= 0 {
            r = unsafe { ffi::avformat_find_stream_info(format, null_mut()) };
        }

        if r < 0 {
            Err(AVError!(Format, r))
        }
        else {
            Ok(Media {
                format: format,
            })
        }
    }

    /// Iterate over media's streams
    pub fn streams(&self) -> StreamIter {
        StreamIter::new(&self)
    }

    /// Return a Stream for the given index
    pub fn stream<'a>(&'a self, id: StreamId) -> Option<Stream<'a>> {
        let format = unsafe { self.format.as_ref().unwrap() };
        if id >= format.nb_streams as i32 {
            return None
        }

        let streams = format.streams;
        Some(unsafe { Stream::new(*streams.offset(id as isize)) })
    }
}


impl Drop for Media {
    fn drop(&mut self) {
        if !self.format.is_null() {
            unsafe { ffi::avformat_close_input(&mut self.format); }
            self.format = null_mut();
        }
    }
}




