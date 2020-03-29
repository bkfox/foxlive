use std::ffi::CStr;
use super::ffi;


#[derive(Debug)]
#[repr(u8)]
pub enum ErrorCode {
    File,
    Format,
    Codec,
    Decoder,
    Resampler,
    Generic,
}


#[derive(Debug)]
pub struct Error {
    pub code: ErrorCode,
    pub msg: String,
}


impl Error {
    pub fn File(msg: String) -> Error { Error { code: ErrorCode::File, msg: msg } }
    pub fn Format(msg: String) -> Error { Error { code: ErrorCode::Format, msg: msg } }
    pub fn Codec(msg: String) -> Error { Error { code: ErrorCode::Codec, msg: msg } }
    pub fn Decoder(msg: String) -> Error { Error { code: ErrorCode::Decoder, msg: msg } }
    pub fn Resampler(msg: String) -> Error { Error { code: ErrorCode::Resampler, msg: msg } }
    pub fn Generic(msg: String) -> Error { Error { code: ErrorCode::Generic, msg: msg } }
}

/// [av_strerror] Return a msg of the ffmpeg error code
pub fn av_strerror(code: i32) -> String {
    let mut s_ = [0i8; 64];
    let s = &mut s_[0] as *mut i8;
    unsafe {
        match ffi::av_strerror(code, s, s_.len() as u64) {
            0 => CStr::from_ptr(s).to_str().unwrap().to_string(),
            _n => format!("unknown error {}", code),
        }
    }
}


macro_rules! FmtError {
    ($err:ident, $($format_args:tt)*) => {
        // Error::$err(format!($($format_args)*))
        Error::$err(format!($($format_args)*))
    }
}

macro_rules! AVError {
    ($err:ident, $code:ident) => {
        // Error::$err(super::error::av_strerror($code))
        Error::$err(super::error::av_strerror($code))
    }
}



