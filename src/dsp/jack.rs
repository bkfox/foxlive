use std::iter::FromIterator;

use jack as j;
use smallvec::SmallVec;

use crate::data::channels::*;
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


pub struct JackInput {
    ports: SmallVec<[j::Port<j::AudioIn>; 2]>
}

impl DSP for JackInput {
    type Sample=f32;
    type Scope=j::ProcessScope;

    fn process_audio(&mut self, scope: &Self::Scope, input: Option<&dyn Channels<Sample=Self::Sample>>,
                     output: Option<&mut dyn ChannelsMut<Sample=Self::Sample>>)
    {
        let output = output.expect("output not provided");
        for (index, port) in self.ports.iter().enumerate() {
            let slice = port.as_slice(scope);
            add_samples_inplace(output.channel_mut(index as u8), slice);
        }
    }

    fn n_channels(&self) -> NChannels {
        self.ports.len() as NChannels
    }

    fn is_source(&self) -> bool { true }
}



pub struct JackOutput {
    ports: SmallVec<[j::Port<j::AudioOut>; 2]>
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

    fn process_audio(&mut self, scope: &Self::Scope, input: Option<&dyn Channels<Sample=Self::Sample>>,
                     output: Option<&mut dyn ChannelsMut<Sample=Self::Sample>>)
    {
        let input = input.expect("input not provided");
        for (index, port) in self.ports.iter_mut().enumerate() {
            let at = scope.last_frame_time() as f32;
            let slice = port.as_mut_slice(scope);
            // map_samples_inplace(slice, &|s| at.sin());
            copy_samples_inplace(slice, input.channel(index as u8));
        }
    }

    fn n_channels(&self) -> NChannels {
        self.ports.len() as NChannels
    }

    fn is_sink(&self) -> bool { true }
}

