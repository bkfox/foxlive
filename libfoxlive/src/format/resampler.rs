use std::ptr::null_mut;
use std::marker::PhantomData;

use crate::data::{ChannelLayout,NChannels,NSamples,Sample,SampleRate};

use super::ffi;
use super::error::Error;
use super::codec::CodecContext;


/// Resample packets into an interleaved buffer to the provided rate and channel
/// layout.
pub struct Resampler<S: Sample> {
    swr: *mut ffi::SwrContext,
    src_rate: SampleRate,
    dst_rate: SampleRate,
    dst_n_channels: NChannels,
    phantom: PhantomData<S>,
}

impl<S: Sample> Resampler<S> {
    pub fn new(context: &CodecContext, sample_rate: SampleRate,
               layout: Option<ChannelLayout>)
        -> Result<Resampler<S>,Error>
    {
        let layout = layout.unwrap_or(context.channel_layout());
        unsafe {
            // TODO: take bufferview as argument, use it for into_sample_ffi's param
            //       and keep reference to it
            let swr = ffi::swr_alloc_set_opts(null_mut(),
                layout.signed(), S::into_sample_ffi(true), sample_rate,
                context.channel_layout().signed(), context.sample_fmt, context.sample_rate,
                0, null_mut()
            );

            match ffi::swr_init(swr) {
                r if r < 0 => Err(AVError!(Resampler, r)),
                _ => Ok(Resampler {
                    swr: swr,
                    src_rate: context.sample_rate,
                    dst_rate: sample_rate,
                    dst_n_channels: layout.n_channels(),
                    phantom: PhantomData,
                })
            }
        }
    }

    /// Source sample rate
    pub fn src_rate(&self) -> SampleRate {
        self.src_rate
    }

    /// Destination sample rate
    pub fn dst_rate(&self) -> SampleRate {
        self.dst_rate
    }

    /// Convert into destination sample rate
    pub fn into_dst_samples(&self, samples: NSamples) -> NSamples {
        unsafe{ ffi::av_rescale_rnd(samples as i64, self.dst_rate as i64, self.src_rate as i64,
                                    ffi::AVRounding_AV_ROUND_UP) as NSamples }
    }

    /// Convert into source sample rate
    pub fn into_src_samples(&self, samples: NSamples) -> NSamples {
        unsafe { ffi::av_rescale_rnd(samples as i64, self.src_rate as i64, self.dst_rate as i64,
                                     ffi::AVRounding_AV_ROUND_UP) as NSamples }
    }

    /// Convert given frame into output buffers
    pub fn convert(&mut self, out: &mut Vec<S>, frame: &ffi::AVFrame) {
        let src_nb_samples = frame.nb_samples;

        // destination number of samples
        let dst_nb_samples = unsafe { ffi::av_rescale_rnd(
            ffi::swr_get_delay(self.swr, self.src_rate as i64) +
                src_nb_samples as i64,
            self.dst_rate as i64, self.src_rate as i64,
            ffi::AVRounding_AV_ROUND_UP
        )};

        let offset = out.len();
        out.resize(offset + (dst_nb_samples * self.dst_n_channels as i64) as usize, S::default());

        // convert
        unsafe { ffi::swr_convert(
            self.swr,
            &mut (out.as_mut_ptr().offset(offset as isize) as *mut u8),
            dst_nb_samples as i32,
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


