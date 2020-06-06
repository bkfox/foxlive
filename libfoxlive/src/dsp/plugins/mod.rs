
use libfoxlive::dsp::DSP;

pub use echo;

pub fn list_plugins() -> Vec<String> {
    vec!["echo",]
}

pub fn new_plugin(name: &str) -> Option<Box<DSP>> {
    match name {
        "echo" => echo::Echo::new(),
    }
}
