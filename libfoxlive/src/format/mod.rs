//! Provides higher level utilities to manipulate media files, by interfacing with ffmpeg.
//!
//! Before using it, a diligent user must call `init()` function in order to initialize ffmpeg
//!


#[allow(warnings)]
mod ffi;
#[macro_use]
pub mod error;
#[macro_use]
pub mod futures;

pub mod resampler;

pub mod codec;
pub mod stream;
pub mod format;
pub mod reader;
pub mod media;


pub use error::Error;
pub use format::FormatContext;
pub use reader::Reader;
pub use stream::{StreamInfo,StreamId,Stream};


/// Initialize crate, registering codecs and muxers.
pub fn init() {
    unsafe { ffi::av_register_all() };
    unsafe { ffi::avcodec_register_all() };
}



// Impl From for TimeBase
use crate::data::time::TimeBase;
impl From<ffi::AVRational> for TimeBase {
    fn from(value: ffi::AVRational) -> TimeBase {
        TimeBase::from((value.den, value.num))
    }
}

