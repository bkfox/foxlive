use std::ffi::CStr;
use super::ffi;


#[derive(Clone,Debug)]
#[repr(u8)]
pub enum ErrorCode {
    Media,
    Format,
    Codec,
    Reader,
    Resampler,
    Generic,
}


#[derive(Clone,Debug)]
pub struct Error {
    pub code: ErrorCode,
    pub msg: String,
}


impl Error {
    pub fn media<T: Into<String>>(msg: T) -> Error {
        Error { code: ErrorCode::Media, msg: msg.into() }
    }

    pub fn format<T: Into<String>>(msg: T) -> Error {
        Error { code: ErrorCode::Format, msg: msg.into() }
    }

    pub fn codec<T: Into<String>>(msg: T) -> Error {
        Error { code: ErrorCode::Codec, msg: msg.into() }
    }

    pub fn reader<T: Into<String>>(msg: T) -> Error {
        Error { code: ErrorCode::Reader, msg: msg.into() }
    }

    pub fn resampler<T: Into<String>>(msg: T) -> Error {
        Error { code: ErrorCode::Resampler, msg: msg.into() }
    }

    pub fn generic<T: Into<String>>(msg: T) -> Error {
        Error { code: ErrorCode::Generic, msg: msg.into() }
    }
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
    ($err:ident, $($format_args:tt)*) => {{
        use crate::format::error::*;
        Error { code: ErrorCode::$err, msg: format!($($format_args)*) }
    }}
}

macro_rules! AVError {
    ($err:ident, $code:ident) => {{
        use crate::format::error::*;
        Error { code: ErrorCode::$err, msg: super::error::av_strerror($code) }
    }}
}



