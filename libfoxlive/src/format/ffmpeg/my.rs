use std::io::Read;
use std::iter::Iterator;
use std::slice;
use std::collections::BTreeMap;


use super::ogg as ffi;

enum Error {
    IO(&'static str),
    InvalidFormat(&'static str),
}

enum FlowState {
    Continue,
    End,
}

type Flow = Result<FlowState, Error>;


trait Format {
    type Options;

    fn reader(reader: Box<Read>, options: Self::Options) -> Self;
}

type StreamId = i32;

trait Packet {
    fn packet_number(&self) -> i64;
    fn stream(&self) -> StreamId;

    fn data(&self) -> &[u8];
    fn data_mut(&mut self) -> &mut [u8];
}



struct OggPacket {
    // stream' id
    pub stream: StreamId,
    pub packet: ffi::ogg_packet,
}

impl OggPacket {
    fn as_ptr(&self) -> *const ffi::ogg_packet {
        &self.packet as *const ffi::ogg_packet
    }

    fn as_mut_ptr(&mut self) -> *mut ffi::ogg_packet {
        &mut self.packet as *mut ffi::ogg_packet
    }
}

impl Packet for OggPacket {
    fn packet_number(&self) -> i64 {
        self.packet.packetno
    }

    fn stream(&self) -> StreamId {
        self.stream
    }

    fn data(&self) -> &[u8] {
        if self.packet.packet.is_null() { &[] }
        else { unsafe { slice::from_raw_parts(
            self.packet.packet, self.packet.bytes as usize
        )}}
    }

    fn data_mut(&mut self) -> &mut [u8] {
        if self.packet.packet.is_null() { &mut [] }
        else { unsafe { slice::from_raw_parts_mut(
            self.packet.packet, self.packet.bytes as usize
        )}}
    }
}


struct OggStream {
    // sequencial id of the stream
    pub id: StreamId,
    // stream id got from page
    pub index: StreamId,
    // stream itself
    pub stream: ffi::ogg_stream_state,
}

impl OggStream {
    pub fn as_ptr(&self) -> *const ffi::ogg_stream_state {
        &self.stream as *const ffi::ogg_stream_state
    }

    pub fn as_mut_ptr(&mut self) -> *mut ffi::ogg_stream_state {
        &mut self.stream as *mut ffi::ogg_stream_state
    }
}


pub struct OggFormat {
    sync: ffi::ogg_sync_state,
    page: ffi::ogg_page,
    buffer_size: usize,
    streams: BTreeMap<StreamId, OggStream>,
    src: Box<Read>,
}

impl OggFormat {
    fn open_input(src: Box<Read>) -> OggFormat {
        let mut format = OggFormat {
            sync: ffi::ogg_sync_state::default(),
            page: ffi::ogg_page::default(),
            streams: BTreeMap::new(),
            buffer_size: 4096,
            src: src,
        };
        let oy = &mut format.sync as *mut ffi::ogg_sync_state;
        unsafe { ffi::ogg_sync_init(oy) };
        format
    }

    /// Return current logical stream (create new one if required)
    fn get_stream(&mut self, id: StreamId) -> &mut OggStream {
        if !self.streams.contains_key(&id) {
            let mut stream = OggStream {
                id: self.streams.len() as StreamId,
                index: id,
                stream: ffi::ogg_stream_state::default(),
            };
            unsafe { ffi::ogg_stream_init(stream.as_mut_ptr(), id) };
        }
        return self.streams.get_mut(&id).unwrap();
    }

    // read next packet
    pub fn next(&mut self, packet: &mut OggPacket) -> Flow {
        let oy = &mut self.sync as *mut ffi::ogg_sync_state;
        let og = &mut self.page as *mut ffi::ogg_page;

        // read from src
        let buffer = unsafe { ffi::ogg_sync_buffer(oy, self.buffer_size as i64) };
        let mut slice = unsafe { slice::from_raw_parts_mut(
            buffer as *mut u8, self.buffer_size
        )};
        let bytes = match self.src.read(slice) {
            Err(e) => return Err(Error::IO("")),
            Ok(s) => s
        };

        unsafe { ffi::ogg_sync_wrote(oy, bytes as i64) };

        // sync page
        if unsafe { ffi::ogg_sync_pageout(oy, og) } != 1 {
            if bytes < self.buffer_size {
                return Ok(FlowState::End);
            }
            return Err(Error::InvalidFormat(""));
        }

        // get logical stream
        let id = unsafe { ffi::ogg_page_serialno(og) };
        let stream = self.get_stream(id);

        unsafe { ffi::ogg_stream_pagein(stream.as_mut_ptr(), og) };

        // get packet
        let op = packet.as_mut_ptr();
        if unsafe { ffi::ogg_stream_packetout(stream.as_mut_ptr(), op) } != 1 {
            return Err(Error::InvalidFormat("Invalid page"));
        }

        packet.stream = stream.id;
        Ok(FlowState::Continue)
    }
}



