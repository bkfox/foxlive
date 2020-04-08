/// Provides Channels trait used to manipulate multi-channels frames.
use bitflags::bitflags;

use super::ffi;


bitflags! {
    /// Channel layouts
    pub struct ChannelLayout : u64 {
        const FRONT_LEFT = 0x00000001;
        const FRONT_RIGHT = 0x00000002;
        const FRONT_CENTER = 0x00000004;
        const LOW_FREQUENCY = 0x00000008;
        const BACK_LEFT = 0x00000010;
        const BACK_RIGHT = 0x00000020;
        const FRONT_LEFT_OF_CENTER = 0x00000040;
        const FRONT_RIGHT_OF_CENTER = 0x00000080;
        const BACK_CENTER = 0x00000100;
        const SIDE_LEFT = 0x00000200;
        const SIDE_RIGHT = 0x00000400;
        const TOP_CENTER = 0x00000800;
        const TOP_FRONT_LEFT = 0x00001000;
        const TOP_FRONT_CENTER = 0x00002000;
        const TOP_FRONT_RIGHT = 0x00004000;
        const TOP_BACK_LEFT = 0x00008000;
        const TOP_BACK_CENTER = 0x00010000;
        const TOP_BACK_RIGHT = 0x00020000;
        const STEREO_LEFT = 0x20000000;
        const STEREO_RIGHT = 0x40000000;
//        const WIDE_LEFT = 0x0000000080000000;
//        const WIDE_RIGHT = 0x0000000100000000;
//        const SURROUND_DIRECT_LEFT = 0x0000000200000000;
//        const SURROUND_DIRECT_RIGHT = 0x0000000400000000;
//        const LOW_FREQUENCY_2 = 0x0000000800000000;
//        const LAYOUT_NATIVE = 0x8000000000000000;

        const LAYOUT_MONO = (Self::FRONT_CENTER.bits);
        const LAYOUT_STEREO = (Self::FRONT_LEFT.bits|Self::FRONT_RIGHT.bits);
        const LAYOUT_2POINT1 = (Self::LAYOUT_STEREO.bits|Self::LOW_FREQUENCY.bits);
        const LAYOUT_2_1 = (Self::LAYOUT_STEREO.bits|Self::BACK_CENTER.bits);
        const LAYOUT_SURROUND = (Self::LAYOUT_STEREO.bits|Self::FRONT_CENTER.bits);
        const LAYOUT_3POINT1 = (Self::LAYOUT_SURROUND.bits|Self::LOW_FREQUENCY.bits);
        const LAYOUT_4POINT0 = (Self::LAYOUT_SURROUND.bits|Self::BACK_CENTER.bits);
        const LAYOUT_4POINT1 = (Self::LAYOUT_4POINT0.bits|Self::LOW_FREQUENCY.bits);
        const LAYOUT_2_2 = (Self::LAYOUT_STEREO.bits|Self::SIDE_LEFT.bits|Self::SIDE_RIGHT.bits);
        const LAYOUT_QUAD = (Self::LAYOUT_STEREO.bits|Self::BACK_LEFT.bits|Self::BACK_RIGHT.bits);
        const LAYOUT_5POINT0 = (Self::LAYOUT_SURROUND.bits|Self::SIDE_LEFT.bits|Self::SIDE_RIGHT.bits);
        const LAYOUT_5POINT1 = (Self::LAYOUT_5POINT0.bits|Self::LOW_FREQUENCY.bits);
        const LAYOUT_5POINT0_BACK = (Self::LAYOUT_SURROUND.bits|Self::BACK_LEFT.bits|Self::BACK_RIGHT.bits);
        const LAYOUT_5POINT1_BACK = (Self::LAYOUT_5POINT0_BACK.bits|Self::LOW_FREQUENCY.bits);
        const LAYOUT_6POINT0 = (Self::LAYOUT_5POINT0.bits|Self::BACK_CENTER.bits);
        const LAYOUT_6POINT0_FRONT = (Self::LAYOUT_2_2.bits|Self::FRONT_LEFT_OF_CENTER.bits|Self::FRONT_RIGHT_OF_CENTER.bits);
        const LAYOUT_HEXAGONAL = (Self::LAYOUT_5POINT0_BACK.bits|Self::BACK_CENTER.bits);
        const LAYOUT_6POINT1 = (Self::LAYOUT_5POINT1.bits|Self::BACK_CENTER.bits);
        const LAYOUT_6POINT1_BACK = (Self::LAYOUT_5POINT1_BACK.bits|Self::BACK_CENTER.bits);
        const LAYOUT_6POINT1_FRONT = (Self::LAYOUT_6POINT0_FRONT.bits|Self::LOW_FREQUENCY.bits);
        const LAYOUT_7POINT0 = (Self::LAYOUT_5POINT0.bits|Self::BACK_LEFT.bits|Self::BACK_RIGHT.bits);
        const LAYOUT_7POINT0_FRONT = (Self::LAYOUT_5POINT0.bits|Self::FRONT_LEFT_OF_CENTER.bits|Self::FRONT_RIGHT_OF_CENTER.bits);
        const LAYOUT_7POINT1 = (Self::LAYOUT_5POINT1.bits|Self::BACK_LEFT.bits|Self::BACK_RIGHT.bits);
        const LAYOUT_7POINT1_WIDE = (Self::LAYOUT_5POINT1.bits|Self::FRONT_LEFT_OF_CENTER.bits|Self::FRONT_RIGHT_OF_CENTER.bits);
        const LAYOUT_7POINT1_WIDE_BACK = (Self::LAYOUT_5POINT1_BACK.bits|Self::FRONT_LEFT_OF_CENTER.bits|Self::FRONT_RIGHT_OF_CENTER.bits);
        const LAYOUT_OCTAGONAL = (Self::LAYOUT_5POINT0.bits|Self::BACK_LEFT.bits|Self::BACK_CENTER.bits|Self::BACK_RIGHT.bits);
//         const LAYOUT_HEXADECAGONAL = (Self::LAYOUT_OCTAGONAL.bits|Self::WIDE_LEFT.bits|Self::WIDE_RIGHT.bits|Self::TOP_BACK_LEFT.bits|Self::TOP_BACK_RIGHT.bits|Self::TOP_BACK_CENTER.bits|Self::TOP_FRONT_CENTER.bits|Self::TOP_FRONT_LEFT.bits|Self::TOP_FRONT_RIGHT.bits);
        const LAYOUT_STEREO_DOWNMIX = (Self::STEREO_LEFT.bits|Self::STEREO_RIGHT.bits);
    }
}


impl ChannelLayout {
    /// Return layout for the given number of channels
    pub fn from_n_channels(n_channels: NChannels) -> Option<Self> {
        let layout = unsafe { ffi::av_get_default_channel_layout(n_channels as i32) };
        Self::from_bits(layout as u64)
    }

    /// Return bits as signed
    pub fn signed(&self) -> i64 {
        self.bits() as i64
    }

    /// Return number of channels for this channel layout
    pub fn n_channels(&self) -> NChannels {
        unsafe { ffi::av_get_channel_layout_nb_channels(self.bits()) as NChannels }
    }
}



/// Number of channels
pub type NChannels = u8;


