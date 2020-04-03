//! Provide a simple interface to read and manipulate audio files.
use std::sync::{Arc,RwLock};

use crate::data::buffers::Buffers;
use crate::data::channels::*;
use crate::data::samples::{Sample,SampleRate};

use super::futures::*;
use super::error::Error;
use super::reader::{Reader,ClosureReaderHandler};
use super::stream::StreamId;


#[repr(u8)]
pub enum MediaState {
    /// Media is closed
    Closed,
    /// Media is closed
    Open,
    /// Media stream are being read
    Reading,
    /// Media is being written
    Writing,
    /// Media has been fully loaded
    Ready,
}


pub struct Media<S: Sample> {
    // TODO: shared state or AtomicState?
    pub state: Arc<RwLock<MediaState>>,
    pub path: String,
    pub buffers: Arc<RwLock<Buffers<S>>>,
    n_channels: NChannels,
}


impl<S: Sample> Media<S> {
    pub fn new<T: Into<String>>(path: T) -> Self {
        Media {
            state: Arc::new(RwLock::new(MediaState::Closed)),
            path: path.into(),
            buffers: Arc::new(RwLock::new(Buffers::new())),
            n_channels: 0,
        }
    }

    pub fn n_channels(&self) -> NChannels {
        self.n_channels
    }

    /// Read media stream, returning a future to poll in order to decode and load
    /// it.
    pub fn read_audio(&mut self, stream_id: Option<StreamId>, rate: SampleRate, layout: Option<ChannelLayout>) -> Result<Box<Future>,Error> {
        let mut state = self.state.write().unwrap();
        match *state {
            MediaState::Closed|MediaState::Open => (),
            _ => return Err(Error::media("Invalid media state")),
        }

        *state = MediaState::Reading;

        let media_buffers = self.buffers.clone();
        let media_state = self.state.clone();
        let handler = ClosureReaderHandler::new(move |_, buffers: &mut Buffers<S>, poll: &mut Poll| {
            // media has been closed/dropped
            if let MediaState::Closed = *media_state.read().unwrap() {
                *poll = Poll::Ready(Ok(()));
                return;
            }

            match poll {
                Poll::Ready(Err(_)) => {

                },
                Poll::Pending|Poll::Ready(Ok(_)) => {
                    let mut media_buffers_lock = media_buffers.write().unwrap();
                    let ref mut media_buffers = *media_buffers_lock;
                    media_buffers.resize_channels(buffers.n_channels());

                    for (buffer,media_buffer) in buffers.iter_mut()
                                                        .zip(media_buffers.iter_mut()) {
                        media_buffer.extend_from_slice(&buffer);
                    }

                    buffers.clear();
                },
            };
        });

        let n_channels = &mut self.n_channels;
        Reader::open(self.path.as_str(), stream_id, rate, layout, handler)
            .and_then(|reader| {
                *n_channels = reader.stream().n_channels();
                Ok(reader.boxed())
            })
    }
}

impl<S: Sample> Drop for Media<S> {
    fn drop(&mut self) {
        let mut state = self.state.write().unwrap();
        *state = MediaState::Closed;
    }
}


