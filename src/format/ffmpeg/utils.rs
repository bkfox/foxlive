use std::ffi::{CString,CStr};
use std::ptr::null_mut;

use nix::Errno;

use super::ffi;
use super::Error;




/// [av_strerror] Return a description of the ffmpeg error code
pub fn strerror(code: i32) -> String {
    let mut s_ = [0i8; 64];
    let s = &mut s_[0] as *mut i8;
    unsafe {
        match ffi::av_strerror(code, s, s_.len()) {
            0 => CStr::from_ptr(s).to_str().unwrap().to_string(),
            _n => format!("unknown error {}", code),
        }
    }
}

macro_rules! Error {
    ($err:ident, $($format_args:tt)*) => {
        Err(Error::$err(format!($($format_args)*)))
    }
}


pub enum Flow<Error> {
    Next,
    Stop,
    Err(Error),
}


/// From an ffmpeg function result, return the corresponding Flow
/// information
macro_rules! ToFlow {
    ($err:ident, $r:ident) => {{
        // EOF
        if $r == -541478725 {
            return Flow::Stop;
        }

        // cf. AVERROR macros definitions
        let err = Errno::from_i32(if Errno::EDOM as i32 > 0 { -$r }
                                  else { $r });
        match err {
            Errno::EAGAIN => Flow::Next,
            _ => Flow::Err(Error::$err(strerror($r))),
        }
    }}
}


