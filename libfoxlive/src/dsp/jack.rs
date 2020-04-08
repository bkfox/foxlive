//! Implement DSP for jack audio ports, as source and sink dsp nodes in the audio g`Graph`.
//! 
//! # Examples
//!
//! ```
//! use jack as j;
//! use futures::executor::LocalPool;
//!
//! use foxlive::dsp::jack::*;
//! use foxlive::dsp::graph::Graph;
//! use foxlive::dsp::media::MediaView;
//! use foxlive::format;
//! use foxlive::format::media::Media;
//!
//!
//! fn main() {
//!     format::init();

//!     let client = j::Client::new("foxlive", jack::ClientOptions::NO_START_SERVER)
//!                      .unwrap().0;
//!     let mut media = Box::new(Media::new("./test.opus"));
//!     let reader = media.read_audio(None, 48000, None);

//!     let mut graph = Graph::new();
//!     let media_view = graph.add_node(MediaView::new(media, 1.0));
//!     let master = graph.add_child(media_view, JackOutput::acquire(&client, "master", 2));
//!     graph.updated();

//!     let process_handler = j::ClosureProcessHandler::new(
//!         move |client: &j::Client, scope: &j::ProcessScope| {
//!             graph.process_nodes(scope);
//!             j::Control::Continue
//!         },
//!     );

//!     let active_client = client.activate_async((), process_handler).unwrap();

//!     let mut pool = LocalPool::new();
//!     pool.run_until(reader.unwrap());
//!     loop {}
//! }
//! ```
//!
use std::iter::FromIterator;

use jack as j;
use smallvec::SmallVec;

use crate as libfoxlive;
use libfoxlive_derive::foxlive_controller;
use crate::data::{BufferView,NChannels,NSamples,NFrames};
use crate::data::samples::*;
use super::dsp::DSP;
use super::graph::ProcessScope;


impl ProcessScope for j::ProcessScope {
    fn n_samples(&self) -> NSamples {
        <j::ProcessScope>::n_frames(self) as NSamples
    }

    fn last_frame_time(&self) -> NFrames {
        <j::ProcessScope>::last_frame_time(self)
    }
}


#[foxlive_controller("jack_input")]
pub struct JackInput {
    pub ports: SmallVec<[j::Port<j::AudioIn>; 2]>
}

impl DSP for JackInput {
    type Sample=f32;
    type Scope=j::ProcessScope;

    fn process_audio(&mut self, scope: &Self::Scope, _input: Option<&dyn BufferView<Sample=Self::Sample>>,
                     output: Option<&mut dyn BufferView<Sample=Self::Sample>>)
    {
        let output = output.expect("output not provided");
        for (index, port) in self.ports.iter().enumerate() {
            let slice = port.as_slice(scope);
            add_samples_inplace(output.channel_mut(index as NChannels).unwrap(), slice.iter());
        }
    }

    fn n_channels(&self) -> NChannels {
        self.ports.len() as NChannels
    }

    fn is_source(&self) -> bool { true }
}


#[foxlive_controller("jack_output")]
pub struct JackOutput {
    pub ports: SmallVec<[j::Port<j::AudioOut>; 2]>
}


impl JackOutput {
    /// Create and register a multichannel jack output
    pub fn acquire(client: &j::Client, name: &str, channels: NChannels) -> Self {
        let ports = (0..channels)
            .map(|channel| client.register_port(format!("{}_{}", name, channel).as_str(),
                                                j::AudioOut::default())
                                 .expect("port name too long"));

        JackOutput {
            ports: SmallVec::from_iter(ports)
        }
    }
}



impl DSP for JackOutput {
    type Sample=f32;
    type Scope=j::ProcessScope;

    fn process_audio(&mut self, scope: &Self::Scope, input: Option<&dyn BufferView<Sample=Self::Sample>>,
                     _output: Option<&mut dyn BufferView<Sample=Self::Sample>>)
    {
        let input = input.expect("input not provided");
        for (index, port) in self.ports.iter_mut().enumerate() {
            let slice = port.as_mut_slice(scope);
            // map_samples_inplace(slice, &|s| at.sin());
            copy_samples_inplace(slice.iter_mut(), input.channel(index as NChannels).unwrap());
        }
    }

    fn n_channels(&self) -> NChannels {
        self.ports.len() as NChannels
    }

    fn is_sink(&self) -> bool { true }
}

