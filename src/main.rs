#![feature(unboxed_closures)]

use jack as j;

mod format;
mod data;
mod dsp;

use dsp::jack::*;
use dsp::graph::Graph;

use data::samples::SampleRate;
use data::channels::ChannelLayout;
use format::reader::{MediaReader,Poll,ClosureReaderHandler};
use format::stream::StreamId;
use data::buffers::Buffers;



fn main() {
    format::init();

    let client = j::Client::new("foxlive", jack::ClientOptions::NO_START_SERVER)
                     .unwrap().0;

    let mut media = MediaReader::open("./test.mp3").unwrap();
    let stream = media.streams().next().unwrap();
    let stream = media.read_audio_stream(
        stream.id, 48000, ChannelLayout::LayoutStereo,
        ClosureReaderHandler::new(|_, buffers: &mut Buffers<f32>, poll: &Poll| {

        })
    );

    println!("start decoding...");
    loop {
        match media.poll() {
            Poll::Ready(Err(e)) => println!("decoding error: {:?}", e),
            Poll::Ready(Ok(_)) => break,
            _ => continue,
        }
    }
    println!("decoding done");


    // let mut graph = Graph::<f32, j::ProcessScope>::new();
    // let master = graph.add_node(JackOutput::acquire(&client, "master", 2));
    let master = JackOutput::acquire(&client, "master", 2);

    let process_handler = j::ClosureProcessHandler::new(
        move |client: &j::Client, scope: &j::ProcessScope| {
            // graph.process_nodes(scope);
            j::Control::Continue
        },
    );

    let active_client = client.activate_async((), process_handler).unwrap();
    loop {}
}
