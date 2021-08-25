/// Provides types and utilities to manipulate samples.
pub use sample::Sample;

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
    /// Return ffmpeg's SampleFormat
    fn as_ffi(&self) -> ffi::AVSampleFormat {
        *self as ffi::AVSampleFormat
    }
}

/// Sample to SampleFmt conversion
pub trait IntoSampleFmt {
    /// Return SampleFormat for implemented type. `interleaved` argument indicates
    /// wether sample channels are interleaved.
    fn sample_fmt(_interleaved: bool) -> SampleFmt { SampleFmt::None }

    /// Return ffmpeg's SampleFormat equivalent to this type.
    fn sample_ffi(interleaved: bool) -> ffi::AVSampleFormat {
        Self::sample_fmt(interleaved).as_ffi()
    }
}

impl IntoSampleFmt for u8 {
    fn sample_fmt(interleaved: bool) -> SampleFmt {
        if interleaved { SampleFmt::U8 }
        else { SampleFmt::U8p }
    }
}

impl IntoSampleFmt for i16 {
    fn sample_fmt(interleaved: bool) -> SampleFmt {
        if interleaved { SampleFmt::S16 }
        else { SampleFmt::S16p }
    }
}

impl IntoSampleFmt for i32 {
    fn sample_fmt(interleaved: bool) -> SampleFmt {
        if interleaved { SampleFmt::S32 }
        else { SampleFmt::S32p }
    }
}

impl IntoSampleFmt for f32 {
    fn sample_fmt(interleaved: bool) -> SampleFmt {
        if interleaved { SampleFmt::Flt }
        else { SampleFmt::Fltp }
    }
}

impl IntoSampleFmt for f64 {
    fn sample_fmt(interleaved: bool) -> SampleFmt {
        if interleaved { SampleFmt::Dbl }
        else { SampleFmt::Dblp }
    }
}


/*
/// Generic trait for samples
pub trait Sample: 'static+sample::Sample+
                  Add<Output=Self>+Mul<Output=Self>+
                  Default+IntoSampleFmt+Unpin+Display+Debug {}


impl Sample for u8 {}

impl Sample for i16 {}

impl Sample for i32 {}

impl Sample for f32 {}

impl Sample for f64 {}
*/


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


pub fn fill_samples<S: Sample>(a: SampleSliceMut<S>, value: S)
{
    for s in a.iter_mut() {
        *s = value;
    }
}

/// Map frames together and update `a` with resulting values.
// FIXME: func arg by ref or copy?
pub fn map_samples<S: Sample>(a: SampleSliceMut<S>, func: &impl Fn(S) -> S)
{
    for s in a.iter_mut() {
        *s = func(*s);
    }
}


/// Zip-Map frames together and update `a` with resulting values.
pub fn copy_samples<'a,S: 'a+Sample>(a: impl Iterator<Item=&'a mut S>, b: impl Iterator<Item=&'a S>)
{
    for (s_a, s_b) in a.zip(b) {
        *s_a = *s_b;
    }
}


/// Zip-Map frames together and update `a` with resulting values.
pub fn zip_map_samples<'a, S: 'a+Sample>(a: impl Iterator<Item=&'a mut S>, b: impl Iterator<Item=&'a S>, func: &impl Fn(S, S) -> S)
{
    for (s_a, s_b) in a.zip(b) {
        *s_a = func(*s_a, *s_b);
    }
}

/// Samples addition between two slices and
pub fn merge_samples<'a,S: 'a+Sample>(a: impl Iterator<Item=&'a mut S>, b: impl Iterator<Item=&'a S>) {
    zip_map_samples(a, b, &|a: S, b: S| a.add_amp(b.to_signed_sample()))
}


#[cfg(test)]
mod tests {
    /// Test: map_samples
    #[test]
    fn map_samples() {
        let mut a = [0, 1, 2];
        super::map_samples(&mut a, &|s| s*2);
        assert_eq!(a, [0, 2, 4]);
    }

    /// Test: merge_samples, zip_map_samples
    #[test]
    fn merge_samples() {
        let (mut a, b) = ([1, 2, 3], [1, 2, 3]);
        super::merge_samples(a.iter_mut(), b.iter());

        assert_eq!(a, [2, 4, 6]);
    }
}



