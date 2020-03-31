use std::ops::Deref;
use std::ptr::null_mut;

use crate::data::channels::ChannelLayout;

use super::ffi;
use super::error::Error;
use super::stream::Stream;
use super::futures::*;


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

    pub fn channel_layout(&self) -> ChannelLayout {
        ChannelLayout::from_bits(self.channel_layout).unwrap()
    }

    /// Send packet of data to decode to the codec.
    ///
    /// Return Poll:
    /// - `Poll::Pending`: more packets are welcome
    /// - `Poll::Ready(Ok(_))`: decoding has been completed
    /// - `Poll::Ready(Err(_))`: an error occurred
    pub fn send_packet(&self, packet: *mut ffi::AVPacket) -> Poll {
        let r = unsafe { ffi::avcodec_send_packet(self.context, packet) };
        if r == 0 {
            Poll::Pending
        }
        else {
            ToPoll!(Codec, r)
        }
    }

    /// Receive a frame from codec.
    ///
    /// Return Poll:
    /// - Poll::Pending: codec needs more packet inputs
    /// - Poll::Ready(Ok(_)): a frame has been decoded
    /// - Poll::Ready(Err(_)): an error occurred
    ///
    pub fn receive_frame(&self, frame: *mut ffi::AVFrame) -> Poll {
        let r = unsafe { ffi::avcodec_receive_frame(self.context, frame) };
        if r == 0 {
            Poll::Ready(Ok(()))
        }
        else {
            ToPoll!(Codec, r)
        }
    }
}


impl Drop for CodecContext {
    fn drop(&mut self) {
        /*if !self.context.is_null() {
            unsafe { ffi::avcodec_free_context(&mut self.context); }
        }*/
    }
}


impl Deref for CodecContext {
    type Target = ffi::AVCodecContext;

    fn deref(&self) -> &Self::Target {
        unsafe { self.context.as_ref().unwrap() }
    }
}

