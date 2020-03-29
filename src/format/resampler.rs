use std::ptr::null_mut;
use std::marker::PhantomData;

use smallvec::SmallVec;


use crate::data::buffers::{Buffers,Buffer};
use crate::data::channels::ChannelLayout;
use crate::data::samples::{Sample,SampleRate};

use super::ffi;
use super::error::Error;
use super::codec::CodecContext;


/// Resample packets
pub struct Resampler<S: Sample> {
    swr: *mut ffi::SwrContext,
    src_rate: SampleRate,
    dst_rate: SampleRate,
    // pointer to output buffers; capacity set to the number of channels
    out_bufs: SmallVec<[*mut u8; 8]>,
    phantom: PhantomData<S>,
}

impl<S: Sample> Resampler<S> {
    pub fn new(context: &CodecContext, sample_rate: SampleRate,
               channel_layout: ChannelLayout)
        -> Result<Resampler<S>,Error>
    {
        unsafe {
            println!("Resampler with {}, codec {}", sample_rate, context.sample_rate);
            let swr = ffi::swr_alloc_set_opts(null_mut(),
                channel_layout.bits() as i64, S::into_sample_ffi(), sample_rate,
                context.channel_layout as i64, context.sample_fmt, context.sample_rate,
                0, null_mut()
            );


            match ffi::swr_init(swr) {
                r if r < 0 => Err(AVError!(Resampler, r)),
                _ => Ok(Resampler {
                    swr: swr,
                    src_rate: context.sample_rate,
                    dst_rate: sample_rate,
                    out_bufs: SmallVec::with_capacity(channel_layout.n_channels() as usize),
                    phantom: PhantomData,
                })
            }
        }
    }

    /// Convert given frame into output buffers
    pub fn convert(&mut self, out: &mut Buffers<S>, frame: &ffi::AVFrame) {
        let src_nb_samples = frame.nb_samples;

        // destination number of samples
        let dst_nb_samples = unsafe { ffi::av_rescale_rnd(
            ffi::swr_get_delay(self.swr, self.src_rate as i64) +
                src_nb_samples as i64,
            self.dst_rate as i64, self.src_rate as i64,
            ffi::AVRounding_AV_ROUND_UP
        )};

        // clear pointer to output buffers
        self.out_bufs.clear();

        // FIXME: bottleneck
        // ensure output buffers have the right number of channels
        out.resize(self.out_bufs.capacity() as usize,
                   Buffer::with_capacity(dst_nb_samples as usize));

        for i in 0..self.out_bufs.capacity() {
            let ref mut buffer = &mut out[i];
            let n = buffer.len();
            buffer.resize(n + dst_nb_samples as usize, S::default());
            let offset = unsafe { buffer.as_mut_ptr().offset(n as isize) };
            self.out_bufs.push(offset as *mut u8);
        }

        // convert
        unsafe { ffi::swr_convert(
            self.swr, self.out_bufs.as_mut_ptr(), dst_nb_samples as i32,
            frame.extended_data as *mut *const u8, src_nb_samples
        )};
    }
}


impl<S: Sample> Drop for Resampler<S> {
    fn drop(&mut self) {
        if !self.swr.is_null() {
            unsafe { ffi::swr_free(&mut self.swr) };
        }
    }
}


