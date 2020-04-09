#![feature(unboxed_closures)]
use std::convert::TryInto;
use std::sync::{Arc,RwLock};
use std::time::{Duration,SystemTime};

use jack as j;
use futures::executor::LocalPool;

use libfoxlive::format;
use libfoxlive::dsp::jack::*;
use libfoxlive::dsp::graph::Graph;
use libfoxlive::dsp::media::MediaView;
use libfoxlive::dsp::controller::*;


fn main() {
    format::init();

    let client = j::Client::new("foxlive", jack::ClientOptions::NO_START_SERVER)
                     .unwrap().0;
    let mut graph = Graph::new();
    let mut media = MediaView::new(48000, Duration::from_millis(500));

    media.open("./test.opus").expect("can not open file");

    let reader = media.reader.clone();


    let media_view = graph.add_node(media);
    graph.add_child(media_view, JackOutput::acquire(&client, "master", 2));
    graph.updated();

    let graph = Arc::new(RwLock::new(graph));
    let graph_ = graph.clone();

    let mut now = SystemTime::now();
    let process_handler = j::ClosureProcessHandler::new(
        move |_client: &j::Client, scope: &j::ProcessScope| {
            let mut graph = graph_.write().unwrap();
            graph.process_nodes(scope);

            if let Ok(elapsed) = now.elapsed() {
                if elapsed.as_secs() > 3 {
                    let amp : f32 = graph.get_control(0).unwrap().try_into().unwrap();
                    graph.set_control(0, ControlValue::F32(amp * 0.90));

                    graph.set_control(1, ControlValue::Duration(Duration::from_secs(5)));
                    now = SystemTime::now();
                }
            }

            j::Control::Continue
        },
    );

    let active_client = client.activate_async((), process_handler).unwrap();

    let mut pool = LocalPool::new();
    println!("Start decoding...");
    pool.run_until(reader);
    println!("Decoding done...");
}
