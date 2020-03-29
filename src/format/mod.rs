//! Provides higher level utilities to manipulate media files, by interfacing with ffmpeg.
//!
//! Before using it, a diligent user must call `init()` function in order to initialize ffmpeg
//!


#[allow(warnings)]
mod ffi;
#[macro_use]
pub mod error;
#[macro_use]
pub mod utils;

pub mod resampler;

pub mod codec;
pub mod stream;
pub mod media;
pub mod reader;



/// Initialize crate, registering codecs and muxers.
pub fn init() {
    unsafe { ffi::av_register_all() };
    unsafe { ffi::avcodec_register_all() };
}


