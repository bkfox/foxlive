#include <libavcodec/avcodec.h>
#include <libavformat/avformat.h>
#include <libswresample/swresample.h>

#include <libavutil/avutil.h>
#include <libavutil/dict.h>
#include <libavutil/frame.h>
// FIXME
#include <libavutil/mem.h>
// FIXME
#include <libavutil/opt.h>

//: use type AVCodec
//: use type AVCodecContext
//: use type AVFormatContext
//: use type AVRounding
//: use type SwrContext
//
//: use fn avcodec_register_all
//
//: use fn av_strerror
//: use fn av_register_all
//: use fn av_rescale_rnd
//: use fn av_get_bytes_per_sample
//
//: use fn av_frame_alloc
//: use fn av_frame_free
//: use fn av_read_frame
//
//: use fn av_packet_alloc
//: use fn av_packet_free
//: use fn av_packet_unref
//
//: use fn avformat_open_input
//: use fn avformat_find_stream_info
//: use fn avformat_close_input
//
//: use fn avcodec_alloc_context3
//: use fn avcodec_find_decoder
//: use fn avcodec_free_context
//: use fn avcodec_open2
//: use fn avcodec_send_packet
//: use fn avcodec_receive_frame
//
//: use fn swr_alloc_set_opts
//: use fn swr_convert
//: use fn swr_free
//: use fn swr_get_delay
//: use fn swr_init
//


