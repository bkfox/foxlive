/// Provides types and utilities to manipulate samples.
use std::ops::{Add,Mul};
use std::marker::Unpin;

use super::ffi;


#[repr(i32)]
#[derive(Copy,Clone)]
pub enum SampleFmt {
    None = ffi::AVSampleFormat_AV_SAMPLE_FMT_NONE,
    U8 = ffi::AVSampleFormat_AV_SAMPLE_FMT_U8 ,
    S16 = ffi::AVSampleFormat_AV_SAMPLE_FMT_S16,
    S32 = ffi::AVSampleFormat_AV_SAMPLE_FMT_S32,
    Flt = ffi::AVSampleFormat_AV_SAMPLE_FMT_FLT,
    Dbl = ffi::AVSampleFormat_AV_SAMPLE_FMT_DBL,
    U8p = ffi::AVSampleFormat_AV_SAMPLE_FMT_U8P,
    S16p = ffi::AVSampleFormat_AV_SAMPLE_FMT_S16P,
    S32p = ffi::AVSampleFormat_AV_SAMPLE_FMT_S32P,
    Fltp = ffi::AVSampleFormat_AV_SAMPLE_FMT_FLTP,
    Dblp = ffi::AVSampleFormat_AV_SAMPLE_FMT_DBLP,
}


impl SampleFmt {
    fn as_ffi(&self) -> ffi::AVSampleFormat {
        *self as ffi::AVSampleFormat
    }
}


/// Sample to SampleFmt
pub trait IntoSampleFmt {
    fn into_sample_fmt() -> SampleFmt { SampleFmt::None }

    fn into_sample_ffi() -> ffi::AVSampleFormat {
        Self::into_sample_fmt().as_ffi()
    }
}

impl IntoSampleFmt for u8 {
    fn into_sample_fmt() -> SampleFmt { SampleFmt::U8p }
}

impl IntoSampleFmt for i16 {
    fn into_sample_fmt() -> SampleFmt { SampleFmt::S16p }
}

impl IntoSampleFmt for i32 {
    fn into_sample_fmt() -> SampleFmt { SampleFmt::S32p }
}

impl IntoSampleFmt for f32 {
    fn into_sample_fmt() -> SampleFmt { SampleFmt::Fltp }
}

impl IntoSampleFmt for f64 {
    fn into_sample_fmt() -> SampleFmt { SampleFmt::Dblp }
}

use std::fmt::Display;

/// Generic trait for samples
pub trait Sample: 'static+
                  Add<Output=Self>+Mul<Output=Self>+
                  Copy+Default+IntoSampleFmt+Unpin+Display
{}


impl Sample for u8 {}
impl Sample for i16 {}
impl Sample for i32 {}
impl Sample for f32 {}
impl Sample for f64 {}


/// Sample rate
pub type SampleRate = i32;

/// Number of frame since start
pub type NFrames = u32;

/// Number of samples in a sampleslice
pub type NSamples = usize;

/// Slice of samples
pub type SampleSlice<'a,T> = &'a[T];

/// Mutable slice of samples
pub type SampleSliceMut<'a,T> = &'a mut[T];


/// Map frames together and update `a` with resulting values.
// FIXME: func arg by ref or copy?
pub fn map_samples_inplace<S: Sample>(a: SampleSliceMut<S>, func: &impl Fn(S) -> S)
{
    for s in a.iter_mut() {
        *s = func(*s);
    }
}


/// Zip-Map frames together and update `a` with resulting values.
pub fn copy_samples_inplace<S: Sample>(a: SampleSliceMut<S>, b: SampleSlice<S>)
{
    for (s_a, s_b) in a.iter_mut().zip(b) {
        *s_a = *s_b;
    }
}


/// Zip-Map frames together and update `a` with resulting values.
pub fn zip_map_samples_inplace<S: Sample>(a: SampleSliceMut<S>, b: SampleSlice<S>, func: &impl Fn(S, S) -> S)
{
    for (s_a, s_b) in a.iter_mut().zip(b) {
        *s_a = func(*s_a, *s_b);
    }
}

/// Samples addition between two slices and
pub fn add_samples_inplace<S: Sample>(a: SampleSliceMut<S>, b: SampleSlice<S>) {
    zip_map_samples_inplace(a, b, &|a: S, b: S| a.add(b))
}



#[cfg(test)]
mod tests {
    /// Test: map_samples_inplace
    #[test]
    fn map_samples_inplace() {
        let mut a = [0, 1, 2];
        super::map_samples_inplace(&mut a, &|s| s*2);
        assert_eq!(a, [0, 2, 4]);
    }

    /// Test: add_samples_inplace, zip_map_samples_inplace
    #[test]
    fn add_samples_inplace() {
        let (mut a, b) = ([1, 2, 3], [1, 2, 3]);
        super::add_samples_inplace(&mut a, &b);

        assert_eq!(a, [2, 4, 6]);
    }
}



