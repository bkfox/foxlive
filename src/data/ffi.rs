/* automatically generated by rust-bindgen */

pub type __uint64_t = :: std :: os :: raw :: c_ulong ; extern "C" { pub fn av_get_channel_layout_nb_channels ( channel_layout : u64 ) -> :: std :: os :: raw :: c_int ; } extern "C" { pub fn av_get_channel_layout_channel_index ( channel_layout : u64 , channel : u64 ) -> :: std :: os :: raw :: c_int ; } pub const AVSampleFormat_AV_SAMPLE_FMT_NONE : AVSampleFormat = -1 ; pub const AVSampleFormat_AV_SAMPLE_FMT_U8 : AVSampleFormat = 0 ; pub const AVSampleFormat_AV_SAMPLE_FMT_S16 : AVSampleFormat = 1 ; pub const AVSampleFormat_AV_SAMPLE_FMT_S32 : AVSampleFormat = 2 ; pub const AVSampleFormat_AV_SAMPLE_FMT_FLT : AVSampleFormat = 3 ; pub const AVSampleFormat_AV_SAMPLE_FMT_DBL : AVSampleFormat = 4 ; pub const AVSampleFormat_AV_SAMPLE_FMT_U8P : AVSampleFormat = 5 ; pub const AVSampleFormat_AV_SAMPLE_FMT_S16P : AVSampleFormat = 6 ; pub const AVSampleFormat_AV_SAMPLE_FMT_S32P : AVSampleFormat = 7 ; pub const AVSampleFormat_AV_SAMPLE_FMT_FLTP : AVSampleFormat = 8 ; pub const AVSampleFormat_AV_SAMPLE_FMT_DBLP : AVSampleFormat = 9 ; pub const AVSampleFormat_AV_SAMPLE_FMT_S64 : AVSampleFormat = 10 ; pub const AVSampleFormat_AV_SAMPLE_FMT_S64P : AVSampleFormat = 11 ; pub const AVSampleFormat_AV_SAMPLE_FMT_NB : AVSampleFormat = 12 ; pub type AVSampleFormat = i32 ;