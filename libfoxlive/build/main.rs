extern crate bindgen;

mod bindings;
mod faust_generator;
mod utils;


fn main() {
    faust_generator::FaustGenerators::new("src/dsp/plugins/")
        .and_then(|mut g| g.build());
    // faust_generator::build_dir("src/dsp/plugins").unwrap();

    println!("cargo:rustc-link-lib=avcodec");
    println!("cargo:rustc-link-lib=avformat");
    println!("cargo:rustc-link-lib=swresample");
    println!("cargo:rustc-link-lib=avutil");

    bindings::build("src/data/ffi.h", true);
    bindings::build("src/format/ffi.h", true);
}

