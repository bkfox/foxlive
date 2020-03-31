use std::ffi::CStr;
use super::ffi;


#[derive(Debug)]
#[repr(u8)]
pub enum ErrorCode {
    Media,
    Format,
    Codec,
    Reader,
    Resampler,
    Generic,
}


#[derive(Debug)]
pub struct Error {
    pub code: ErrorCode,
    pub msg: String,
}


impl Error {
    pub fn Media<T: Into<String>>(msg: T) -> Error {
        Error { code: ErrorCode::Media, msg: msg.into() }
    }

    pub fn Format<T: Into<String>>(msg: T) -> Error {
        Error { code: ErrorCode::Format, msg: msg.into() }
    }

    pub fn Codec<T: Into<String>>(msg: T) -> Error {
        Error { code: ErrorCode::Codec, msg: msg.into() }
    }

    pub fn Reader<T: Into<String>>(msg: T) -> Error {
        Error { code: ErrorCode::Reader, msg: msg.into() }
    }

    pub fn Resampler<T: Into<String>>(msg: T) -> Error {
        Error { code: ErrorCode::Resampler, msg: msg.into() }
    }

    pub fn Generic<T: Into<String>>(msg: T) -> Error {
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



