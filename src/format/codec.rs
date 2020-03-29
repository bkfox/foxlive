use std::ops::Deref;
use std::ptr::null_mut;
use nix::errno::Errno;

use super::ffi;
use super::error::Error;
use super::stream::Stream;
use super::utils::*;


pub struct CodecContext {
    pub context: *mut ffi::AVCodecContext,
}

impl CodecContext {
    pub fn from_stream(stream: &Stream)
        -> Result<CodecContext, Error>
    {
        let codec = unsafe { ffi::avcodec_find_decoder(stream.codec_id()) };
        if codec.is_null() {
            return Err(FmtError!(Codec, "no codec found for codec id {}", stream.codec_id()));
        }

        // FIXME: stream.codec is deprecated, however, using avcodec_alloc_context3 does not
        //        provides context.sample_rate and maybe other values
        let context = stream.codec; // unsafe { ffi::avcodec_alloc_context3(codec) };
        if context.is_null() {
            return Err(FmtError!(Codec, "can not allocate codec context"));
        }

        match unsafe { ffi::avcodec_open2(context, codec, null_mut()) } {
            r if r < 0 => return Err(AVError!(Codec, r)),
            _ => {},
        };

        Ok(Self { context: context })
    }

    pub fn send_packet(&self, packet: *mut ffi::AVPacket) -> Poll {
        let mut r = unsafe { ffi::avcodec_send_packet(self.context, packet) };
        ToPoll!(Codec, r)
    }

    pub fn receive_frame(&self, frame: *mut ffi::AVFrame) -> Poll {
        let mut r = unsafe { ffi::avcodec_receive_frame(self.context, frame) };
        ToPoll!(Codec, r)
    }
}


impl Drop for CodecContext {
    fn drop(&mut self) {
        if !self.context.is_null() {
            unsafe { ffi::avcodec_free_context(&mut self.context); }
        }
    }
}


impl Deref for CodecContext {
    type Target = ffi::AVCodecContext;

    fn deref(&self) -> &Self::Target {
        unsafe { self.context.as_ref().unwrap() }
    }
}

