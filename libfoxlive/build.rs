extern crate bindgen;

use std::fs::File;
use std::io::{BufReader,BufRead};
use std::path::Path;


// TODO: more generic
fn parse(path: &str, prefix: &str, keys: &[&str]) -> Vec<(String,String)> {
    let file = File::open(path).unwrap();
    let prefix = "//: ".to_string() + prefix;

    BufReader::new(file).lines()
        .filter_map(move |l| {
            let l = &l.unwrap();
            let l = l.trim_start();
            match l.starts_with(&prefix) {
                true => Some(l.replace(&prefix,"").trim().to_string()),
                false => None
            }
        })
        .filter_map(move |l| {
            for s in keys.iter() {
                if l.starts_with(s) {
                    return Some((s.trim().to_string(),
                                 l.replace(s, "").trim().to_string()))
                }
            }
            None
        })
        .collect()
}


fn gen_bindings(path: &str) {
    let path = Path::new(path);
    let out = path.with_extension("rs");

    let file_mod = path.metadata().unwrap().modified().unwrap();
    if let Ok(metadata) = out.metadata() {
        if metadata.modified().unwrap() > file_mod {
            return;
        }
    }

    let path = path.to_str().unwrap();
    let out = out.to_str().unwrap();

    let mut bindings = bindgen::Builder::default().header(path);

    for (ref ty, ref val) in parse(path, "use", &["type","fn","var"]) {
        bindings = match ty.as_ref() {
            "type" => bindings.whitelist_type(val),
            "fn" => bindings.whitelist_function(val),
            "var" => bindings.whitelist_var(val),
            "enum" => bindings.rustified_enum(val),
            _ => bindings,
        }
    }

    bindings.derive_default(true)
            .generate()
            .expect(&format!("{}: unable to generate bindings", path))
            .write_to_file(out)
            .expect(&format!("{}: couldn't write bindings into {}", path, out));
}



fn main() {
    println!("cargo:rustc-link-lib=avcodec");
    println!("cargo:rustc-link-lib=avformat");
    println!("cargo:rustc-link-lib=swresample");
    println!("cargo:rustc-link-lib=avutil");

    gen_bindings("src/data/ffi.h");
    gen_bindings("src/format/ffi.h");
}

