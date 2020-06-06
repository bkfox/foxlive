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

//: type AVCodec
//: type AVCodecContext
//: type AVFormatContext
//: type AVDiscard
//: type AVRounding
//: type SwrContext
//
//: fn avcodec_register_all
//
//: fn av_strerror
//: fn av_register_all
//: fn av_rescale_rnd
//: fn av_rescale
//: fn av_get_bytes_per_sample
//
//: fn av_frame_alloc
//: fn av_frame_free
//: fn av_read_frame
//: fn av_seek_frame
//
//: fn av_packet_alloc
//: fn av_packet_free
//: fn av_packet_unref
//
//: fn avformat_open_input
//: fn avformat_find_stream_info
//: fn avformat_close_input
//
//: fn avcodec_alloc_context3
//: fn avcodec_find_decoder
//: fn avcodec_free_context
//: fn avcodec_open2
//: fn avcodec_send_packet
//: fn avcodec_receive_frame
//: fn avcodec_parameters_to_context
//
//: fn swr_alloc_set_opts
//: fn swr_convert
//: fn swr_free
//: fn swr_get_delay
//: fn swr_init
//
//: type AVDictionaryEntry
//: fn av_dict_get


