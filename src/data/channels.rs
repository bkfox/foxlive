/// Provides Channels trait used to manipulate multi-channels frames.
use std::ops::Add;
use bitflags::bitflags;

use super::ffi;
use super::samples::*;


bitflags! {
    /// Channel layouts
    pub struct ChannelLayout : u64 {
        const FrontLeft = 0x00000001;
        const FrontRight = 0x00000002;
        const FrontCenter = 0x00000004;
        const LowFrequency = 0x00000008;
        const BackLeft = 0x00000010;
        const BackRight = 0x00000020;
        const FrontLeftOfCenter = 0x00000040;
        const FrontRightOfCenter = 0x00000080;
        const BackCenter = 0x00000100;
        const SideLeft = 0x00000200;
        const SideRight = 0x00000400;
        const TopCenter = 0x00000800;
        const TopFrontLeft = 0x00001000;
        const TopFrontCenter = 0x00002000;
        const TopFrontRight = 0x00004000;
        const TopBackLeft = 0x00008000;
        const TopBackCenter = 0x00010000;
        const TopBackRight = 0x00020000;
        const StereoLeft = 0x20000000;
        const StereoRight = 0x40000000;
        // Fixme: those values require an u64
        /*const WideLeft = 0x0000000080000000;
        const WideRight = 0x0000000100000000;
        const SurroundDirectLeft = 0x0000000200000000;
        const SurroundDirectRight = 0x0000000400000000;
        const LowFrequency_2 = 0x0000000800000000;
        const LayoutNative = 0x8000000000000000;*/

        const LayoutMono = (Self::FrontCenter.bits);
        const LayoutStereo = (Self::FrontLeft.bits|Self::FrontRight.bits);
        const Layout2Point1 = (Self::LayoutStereo.bits|Self::LowFrequency.bits);
        const Layout_2_1 = (Self::LayoutStereo.bits|Self::BackCenter.bits);
        const LayoutSurround = (Self::LayoutStereo.bits|Self::FrontCenter.bits);
        const Layout3Point1 = (Self::LayoutSurround.bits|Self::LowFrequency.bits);
        const Layout4Point0 = (Self::LayoutSurround.bits|Self::BackCenter.bits);
        const Layout4Point1 = (Self::Layout4Point0.bits|Self::LowFrequency.bits);
        const Layout2_2 = (Self::LayoutStereo.bits|Self::SideLeft.bits|Self::SideRight.bits);
        const LayoutQuad = (Self::LayoutStereo.bits|Self::BackLeft.bits|Self::BackRight.bits);
        const Layout5Point0 = (Self::LayoutSurround.bits|Self::SideLeft.bits|Self::SideRight.bits);
        const Layout5Point1 = (Self::Layout5Point0.bits|Self::LowFrequency.bits);
        const Layout5Point0Back = (Self::LayoutSurround.bits|Self::BackLeft.bits|Self::BackRight.bits);
        const Layout5Point1Back = (Self::Layout5Point0Back.bits|Self::LowFrequency.bits);
        const Layout6Point0 = (Self::Layout5Point0.bits|Self::BackCenter.bits);
        const Layout6Point0Front = (Self::Layout2_2.bits|Self::FrontLeftOfCenter.bits|Self::FrontRightOfCenter.bits);
        const LayoutHexagonal = (Self::Layout5Point0Back.bits|Self::BackCenter.bits);
        const Layout6Point1 = (Self::Layout5Point1.bits|Self::BackCenter.bits);
        const Layout6Point1Back = (Self::Layout5Point1Back.bits|Self::BackCenter.bits);
        const Layout6Point1Front = (Self::Layout6Point0Front.bits|Self::LowFrequency.bits);
        const Layout7Point0 = (Self::Layout5Point0.bits|Self::BackLeft.bits|Self::BackRight.bits);
        const Layout7Point0Front = (Self::Layout5Point0.bits|Self::FrontLeftOfCenter.bits|Self::FrontRightOfCenter.bits);
        const Layout7Point1 = (Self::Layout5Point1.bits|Self::BackLeft.bits|Self::BackRight.bits);
        const Layout7Point1Wide = (Self::Layout5Point1.bits|Self::FrontLeftOfCenter.bits|Self::FrontRightOfCenter.bits);
        const Layout7Point1WideBack = (Self::Layout5Point1Back.bits|Self::FrontLeftOfCenter.bits|Self::FrontRightOfCenter.bits);
        const LayoutOctagonal = (Self::Layout5Point0.bits|Self::BackLeft.bits|Self::BackCenter.bits|Self::BackRight.bits);
        // const LayoutHexadecagonal = (Self::LayoutOctagonal.bits|Self::WideLeft.bits|Self::WideRight.bits|Self::TopBackLeft.bits|Self::TopBackRight.bits|Self::TopBackCenter.bits|Self::TopFrontCenter.bits|Self::TopFrontLeft.bits|Self::TopFrontRight.bits);
        const LayoutStereoDownmix = (Self::StereoLeft.bits|Self::StereoRight.bits);
    }
}


impl ChannelLayout {
    /// Return number of channels for this channel layout
    pub fn n_channels(&self) -> NChannels {
        unsafe { ffi::av_get_channel_layout_nb_channels(self.bits()) as NChannels }
    }
}



/// Number of channels
pub type NChannels = u8;


/// Multi-channels samples manipulation
pub trait Channels {
    type Sample: Default+Copy;

    /// Total number of samples per channel
    fn len(&self) -> NSamples;

    /// Total number
    fn n_channels(&self) -> NChannels;

    /// Return channel
    fn channel(&self, channel: NChannels) -> SampleSlice<Self::Sample>;

    /// Return mutable channel
    fn channel_mut(&mut self, channel: NChannels) -> SampleSliceMut<Self::Sample>;

    /// Fill audio buffer with the provided value
    fn fill(&mut self, value: Self::Sample)
        where Self: Sized
    {
        self.map_inplace(&|_a| value);
    }

    /// Map buffer inplace with the provided buffer and function.
    fn map_inplace(&mut self, func: &dyn Fn(Self::Sample) -> Self::Sample)
        where Self: Sized
    {
        for i in 0..self.n_channels() {
            map_samples_inplace(self.channel_mut(i), &func);
        }
    }

    /// Map buffer inplace with the provided buffer and function.
    fn zip_map_inplace(&mut self, b: &dyn Channels<Sample=Self::Sample>,
                       func: &dyn Fn(Self::Sample, Self::Sample) -> Self::Sample)
        where Self: Sized
    {
        if self.n_channels() == b.n_channels() {
            for i in 0..self.n_channels() {
                zip_map_samples_inplace(self.channel_mut(i), b.channel(i), &func)
            }
        }
    }

    /// Merge with the provided Channels using simple addition.
    fn merge_inplace(&mut self, b: &impl Channels<Sample=Self::Sample>)
        where Self: Sized,
              Self::Sample: Default+Copy+Add<Output=Self::Sample>
    {
        self.zip_map_inplace(b, &|a, b| a.add(b))
    }
}


