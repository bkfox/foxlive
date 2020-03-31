#![feature(unboxed_closures)]

use jack as j;
use futures::executor::LocalPool;

use libfoxlive::format;
use libfoxlive::dsp::jack::*;
use libfoxlive::dsp::graph::Graph;
use libfoxlive::dsp::media::MediaView;

use libfoxlive::format::media::Media;



fn main() {
    format::init();

    let client = j::Client::new("foxlive", jack::ClientOptions::NO_START_SERVER)
                     .unwrap().0;
    let mut media = Box::new(Media::new("./test.opus"));
    let reader = media.read_audio(None, 48000, None);

    let mut graph = Graph::new();
    let media_view = graph.add_node(MediaView::new(media, 1.0));
    let master = graph.add_child(media_view, JackOutput::acquire(&client, "master", 2));
    graph.updated();

    let process_handler = j::ClosureProcessHandler::new(
        move |client: &j::Client, scope: &j::ProcessScope| {
            graph.process_nodes(scope);
            j::Control::Continue
        },
    );

    let active_client = client.activate_async((), process_handler).unwrap();

    let mut pool = LocalPool::new();
    println!("Start decoding...");
    pool.run_until(reader.unwrap());
    println!("Decoding done...");

    loop {}
}
