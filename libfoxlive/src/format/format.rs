use std::ffi::{CStr, CString};
use std::ptr::{null_mut,null};

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
            Err(_) => return Err(Error::format("invalid path (ffi::NulError)".to_string())),
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
    pub fn stream(&self, id: StreamId) -> Option<Stream> {
        let context = unsafe { &*self.context };
        if id >= context.nb_streams as i32 {
            return None
        }

        let streams = context.streams;
        Some(unsafe { Stream::new(*streams.offset(id as isize)) })
    }

    /// Return iterator over metadata
    pub fn metadata(&self) -> MetadataIter {
        MetadataIter::new(self)
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


/// Iterator over a format's metadata.
pub struct MetadataIter<'a> {
    format: &'a FormatContext,
    entry: *const ffi::AVDictionaryEntry,
}

impl<'a> MetadataIter<'a> {
    pub fn new(format: &'a FormatContext) -> Self {
        Self { format: format, entry: null() }
    }
}

impl<'a> Iterator for MetadataIter<'a> {
    type Item = (&'a str, &'a str);

    fn next(&mut self) -> Option<Self::Item> {
        self.entry = unsafe { ffi::av_dict_get((*self.format.context).metadata,
                                               CString::new("").unwrap().as_ptr(), self.entry, 2) };
        if self.entry.is_null() { None }
        else {
            // FIXME: CStr::from_ptr will return an error when metadata are not UTF8 valid
            unsafe {
                Some((CStr::from_ptr((*self.entry).key).to_str().unwrap(),
                      CStr::from_ptr((*self.entry).value).to_str().unwrap()))
            }
        }
    }
}

