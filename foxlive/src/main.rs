#![feature(unboxed_closures)]
use std::sync::{Arc,RwLock};
use std::time::Duration;

use jack as j;
use futures::executor::LocalPool;

use libfoxlive::format;
use libfoxlive::dsp::jack::*;
use libfoxlive::dsp::graph::Graph;
use libfoxlive::dsp::media::MediaView;


fn main() {
    format::init();

    let client = j::Client::new("foxlive", jack::ClientOptions::NO_START_SERVER)
                     .unwrap().0;
    let mut graph = Graph::new();
    let mut media = MediaView::new("./test.opus", 48000, Duration::from_millis(500));

    if let Err(err) = media {
        println!("could not open media: {:?}", err);
        return;
    }

    let (media, reader) = media.unwrap();
    let media_view = graph.add_node(media);
    let master = graph.add_child(media_view, JackOutput::acquire(&client, "master", 2));
    graph.updated();

    let graph = Arc::new(RwLock::new(graph));
    let graph_ = graph.clone();

    let process_handler = j::ClosureProcessHandler::new(
        move |client: &j::Client, scope: &j::ProcessScope| {
            graph_.write().unwrap().process_nodes(scope);
            j::Control::Continue
        },
    );

    let active_client = client.activate_async((), process_handler).unwrap();

    let mut pool = LocalPool::new();
    println!("Start decoding...");
    pool.run_until(reader);
    println!("Decoding done...");

    loop {}
}
