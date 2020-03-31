use std::ffi::CString;
use std::ptr::null_mut;

use super::error::Error;
use super::ffi;
use super::stream::{Stream,StreamId,StreamIter};


/// Wrapper around AVFormatContext.
///
/// Deref to the held AVFormatContext
pub struct FormatContext {
    pub context: *mut ffi::AVFormatContext,
}

impl FormatContext {
    /// Open input file with provided path
    pub fn open_input(path: &str) -> Result<Self, Error> {
        let c_path = match CString::new(path) {
            Ok(path) => path,
            Err(_) => return Err(Error::Format("invalid path (ffi::NulError)".to_string())),
        };

        let mut context = null_mut();
        let mut r = unsafe { ffi::avformat_open_input(&mut context, c_path.as_ptr(), null_mut(), null_mut()) };
        if r >= 0 {
            r = unsafe { ffi::avformat_find_stream_info(context, null_mut()) };
        }

        if r < 0 {
            Err(AVError!(Format, r))
        }
        else {
            Ok(Self{ context: context })
        }
    }

    /// Iterate over media's streams
    pub fn streams(&self) -> StreamIter {
        StreamIter::new(&self)
    }

    /// Return a Stream for the given index
    pub fn stream<'a>(&'a self, id: StreamId) -> Option<Stream<'a>> {
        let context = unsafe { &*self.context };
        if id >= context.nb_streams as i32 {
            return None
        }

        let streams = context.streams;
        Some(unsafe { Stream::new(*streams.offset(id as isize)) })
    }
}


impl Drop for FormatContext {
    fn drop(&mut self) {
        if !self.context.is_null() {
            unsafe { ffi::avformat_close_input(&mut self.context); }
            self.context = null_mut();
        }
    }
}




