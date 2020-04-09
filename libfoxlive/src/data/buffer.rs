use std::marker::PhantomData;
use std::ops::{Deref,DerefMut};
use std::ptr::*;

use super::samples::*;
use super::channels::*;


macro_rules! MakeChannelIter {
    ($name:ident $(, $mut:tt)?) => {
        pub struct $name<'a, S: 'a> {
            step: NChannels,
            ptr: NonNull<S>,
            end: *const S,
            phantom: PhantomData<&'a S>,
        }

        impl<'a,S: Sample+Default> Iterator for $name<'a,S> {
            type Item = &'a $($mut)* S;

            fn next(&mut self) -> Option<Self::Item> {
                let ptr = self.ptr.as_ptr();
                if (ptr as *const S) < self.end {
                    self.ptr = unsafe { NonNull::new(ptr.offset(self.step as isize)).unwrap() };
                    Some(unsafe { & $($mut)* *ptr })
                }
                else { None }
            }
        }
    }
}

MakeChannelIter!{ChannelIter}
MakeChannelIter!{ChannelIterMut, mut}


/// This trait provides methods to manipulate audio buffers.
///
pub trait BufferView {
    type Sample: Sample+Default;

    /// Total number of samples.
    fn len(&self) -> usize;

    /// Samples' count per channels.
    fn n_samples(&self) -> NSamples;

    /// Channels' count.
    fn n_channels(&self) -> NChannels;

    /// True if channels' samples are interleaved.
    fn interleaved(&self) -> bool;

    /// Set buffer `is_interleave` (invalidate buffer data).
    fn set_interleaved(&mut self, interleaved: bool);

    /// Get channel layout
    fn get_layout(&self) -> Option<ChannelLayout>;

    /// Iterator over a channel's samples
    fn channel(&self, channel: NChannels) -> Option<ChannelIter<Self::Sample>>;

    /// Mutable iterator over channel's samples.
    fn channel_mut<'a>(&'a mut self, channel: NChannels) -> Option<ChannelIterMut<'a,Self::Sample>>;

    /// Slice over internal buffer
    fn as_slice(&self) -> &[Self::Sample];

    /// Mutable slice over internal buffer
    fn as_slice_mut(&mut self) -> &mut[Self::Sample];

    /// Map function and update self consequently
    fn map_inplace(&mut self, func: &dyn Fn(NChannels, Self::Sample) -> Self::Sample) {
        let n = self.n_channels();
        let interleaved = self.interleaved();
        let slice = self.as_slice_mut();

        if interleaved {
            for i in 0..slice.len() {
                slice[i] = func((i%n as usize) as NChannels, slice[i])
            }
        }
        else {
            let len = slice.len();
            for i in 0..len {
                slice[i] = func((i as usize * n as usize/len) as NChannels, slice[i]);
            }
        }
    }

    /// Fill buffer with this value
    fn fill(&mut self, value: Self::Sample) {
        self.map_inplace(&|_,_| value)
    }

    /// Zip and map with other buffer, set resulting value into self
    fn zip_map_inplace(&mut self, src: &dyn BufferView<Sample=Self::Sample>, func: &dyn Fn(Self::Sample,Self::Sample) -> Self::Sample)
        where Self: Sized
    {
        zip_map(self, src, |a,b| *a = func(*a,*b))
    }

    /// Copy values from buffer
    fn copy_inplace(&mut self, src: &dyn BufferView<Sample=Self::Sample>)
        where Self: Sized
    {
        zip_map(self, src, |a,b| *a = *b)
    }

    /// Merge provided buffer to self
    fn merge_inplace(&mut self, src: &dyn BufferView<Sample=Self::Sample>)
        where Self: Sized
    {
        zip_map(self, src, |a,b| *a = a.add_amp(b.to_signed_sample()))
    }
}


/// Zip and map two input buffers, starting at b's sample index.
pub fn zip_map<S: Sample+Default>(a: &mut dyn BufferView<Sample=S>, b: &dyn BufferView<Sample=S>,
                          func: impl Fn(&mut S,&S))
{
    // TODO: this method should be profiled for cache misses and optimized
    let (a_nc, b_nc) = (a.n_channels(), b.n_channels());
    let n_channels = a_nc.min(b_nc);
    for c in 0..n_channels {
        for (s_a,s_b) in a.channel_mut(c).unwrap().zip(b.channel(c).unwrap()) {
            func(s_a,s_b)
        }
    }
}


/// Audio buffer implementation, handling interleaved and panned buffers.
///
/// Generic `S` is Sample type, `B` the actual buffer's type. This module
/// provides implementation for types: `Vec<S>`, `&[S]`, including:
/// - BufferView trait
/// - From<(interleaved,n_channels,buffer)>
///
pub struct Buffer<S,B>
    where S: Sample+Default,
{
    interleaved: bool,
    n_channels: NChannels,
    pub buffer: B,
    phantom: PhantomData<S>,
}


impl<S: Sample+Default,B> Deref for Buffer<S,B> {
    type Target = B;

    fn deref(&self) -> &Self::Target {
        &self.buffer
    }
}

impl<S: Sample+Default,B> DerefMut for Buffer<S,B> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.buffer
    }
}

macro_rules! ImplBuffer {
    ($alias:ident, $buffer_ty:ty $(, $lifetime:lifetime)?) => {
        /// Implement `From<(interleaved,n_channels,$buffer_ty)>` for Buffer
        impl<$($lifetime,)?S> From<(bool,NChannels,$buffer_ty)> for Buffer<S,$buffer_ty>
            where S: Sample+Default,
        {
            fn from(v: (bool,NChannels,$buffer_ty)) -> Buffer<S,$buffer_ty> {
                Buffer {
                    interleaved: v.0,
                    n_channels: v.1,
                    buffer: v.2,
                    phantom: PhantomData,
                }
            }
        }

        pub type $alias<$($lifetime,)?S> = Buffer<S,$buffer_ty>;

        impl<$($lifetime,)?S> BufferView for Buffer<S,$buffer_ty>
            where S: Sample+Default,
        {
            type Sample = S;

            fn len(&self) -> usize {
                self.buffer.len()
            }

            fn n_samples(&self) -> NSamples {
                self.buffer.len() / self.n_channels as usize
            }

            fn n_channels(&self) -> NChannels {
                self.n_channels
            }

            fn interleaved(&self) -> bool {
                self.interleaved
            }

            fn set_interleaved(&mut self, interleaved: bool) {
                self.interleaved = interleaved;
            }

            fn get_layout(&self) -> Option<ChannelLayout> {
                ChannelLayout::from_n_channels(self.n_channels)
            }

            fn channel(&self, channel: NChannels) -> Option<ChannelIter<Self::Sample>> {
                if self.n_channels != 0 && channel < self.n_channels {
                    let pos = unsafe { self.buffer.as_ptr().offset(self.n_channels as isize - 1) };
                    Some(ChannelIter {
                        step: self.n_channels,
                        ptr: unsafe { NonNull::from(pos.as_ref().unwrap()) },
                        end: unsafe { self.buffer.as_ptr().offset(self.buffer.len() as isize) },
                        phantom: PhantomData,
                    })
                }
                else { None }
            }

            fn channel_mut(&mut self, channel: NChannels) -> Option<ChannelIterMut<Self::Sample>> {
                if self.n_channels != 0 && channel < self.n_channels {
                    let pos = unsafe { self.buffer.as_ptr().offset(self.n_channels as isize - 1) };
                    Some(ChannelIterMut {
                        step: self.n_channels,
                        ptr: unsafe { NonNull::from(pos.as_ref().unwrap()) },
                        end: unsafe { self.buffer.as_ptr().offset(self.buffer.len() as isize) },
                        phantom: PhantomData,
                    })
                }
                else { None }
            }

            fn as_slice(&self) -> &[Self::Sample] {
                &self.buffer
            }

            fn as_slice_mut(&mut self) -> &mut[Self::Sample] {
                &mut self.buffer
            }
        }
    }
}


ImplBuffer!{SliceBuffer, &'a mut [S], 'a}
ImplBuffer!{VecBuffer, Vec<S>}


impl<S: Sample+Default> Buffer<S,Vec<S>> {
    /// New empty buffer.
    pub fn new(interleaved: bool, n_channels: NChannels) -> Self {
        Buffer {
            interleaved: true,
            n_channels: n_channels,
            buffer: Vec::new(),
            phantom: PhantomData,
        }
    }

    /// New buffer with capacity of `n_samples*n_channels` samples.
    pub fn with_capacity(interleaved: bool, n_channels: NChannels, n_samples: NSamples) -> Self {
        Self::with_real_capacity(interleaved, n_channels, n_channels as usize*n_samples as usize)
    }

    /// New buffer with real capacity of `cap` samples.
    pub fn with_real_capacity(interleaved: bool, n_channels: NChannels, cap: usize) -> Self {
        Buffer {
            interleaved: true,
            n_channels: n_channels,
            buffer: Vec::with_capacity(cap),
            phantom: PhantomData,
        }
    }

    /// Clear buffer's samples
    pub fn clear(&mut self) {
        self.buffer.clear();
    }

    /// Update channels count (invalidate buffer content)
    pub fn resize(&mut self, n_channels: NChannels, n_samples: NSamples) {
        let cap = n_channels as usize * n_samples as usize;
        self.buffer.resize(cap, S::default());
        self.n_channels = n_channels;
    }

    /// Update channels count (invalidate buffer content)
    pub fn resize_channels(&mut self, n_channels: NChannels) {
        if self.n_channels != n_channels {
            self.buffer.resize(n_channels as usize * self.n_samples(), S::default());
            self.n_channels = n_channels;
        }
    }

    /// Update channels count (invalidate buffer content)
    pub fn resize_samples(&mut self, n_samples: NSamples) {
        self.resize(self.n_channels, n_samples);
    }
}


