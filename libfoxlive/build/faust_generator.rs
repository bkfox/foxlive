///! This module provides utilities to build Faust dsp files in order to use them
///! as foxlive's DSP.
use std::collections::BTreeMap;
use std::fs;
use std::iter::FromIterator;
use std::path::Path;
use std::process::Command;

use inflector::cases::classcase::to_class_case;

use super::utils::*;


///! This struct handles generating foxlive DSP from Faust's dsp file.
pub struct FaustGenerator {
    pub source: String,
    pub dest: String,
    pub name: String,
    pub struct_name: String,
    pub faust_code: String,
}


impl FaustGenerator {
    ///! Create a new generator for the given Faust dsp file.
    pub fn new(source: &str) -> Self {
        let source = String::from(source);
        let dest = String::from(Path::new(&source).with_extension("rs").to_str().unwrap());
        let name = String::from(Path::new(&source).file_stem().and_then(|p| p.to_str()).unwrap());
        let struct_name = to_class_case(&name);
        FaustGenerator { source, dest, name, struct_name,
                         faust_code: String::new() }
    }

    /// Build dsp only if source file has changed since last build.
    pub fn lazy_build(&mut self) -> Result<(), String> {
        if source_changed(&self.source, &self.dest) {
            self.build()
        }
        else { Ok(()) }
    }

    /// Build Foxlive DSP file.
    pub fn build(&mut self) -> Result<(),String> {
        let output = Command::new("faust").args(&[&self.source, "-lang", "rust"])
                        .output();
        match output {
            Ok(output) => String::from_utf8(output.stdout)
                              .map_err(|_| format!("can't get output code from utf8."))
                              .and_then(|stdout| self.render(stdout)),
            Err(err) => Err(format!("an error occurred when generating {}: {}", self.source, err))
        }
    }

    /// Render rust source code into its final form.
    fn render(&self, source: String) -> Result<(), String> {
        let source = source.replace("mydsp", &self.struct_name);
        fs::write(&self.dest, source).map_err(|_| format!("can't write into {}", self.dest))
    }
}


///! Generates all Faust DSP of a given directory. The build process will try to
///! build all files and creates a `mod.rs` (overwrites existing file).
pub struct FaustGenerators {
    pub path: String,
    pub generators: Vec<FaustGenerator>,
}

impl FaustGenerators {
    /// Create new generator for the given directory path.
    pub fn new(path: &str) -> Result<Self,String> {
        scan_dir(path, "dsp", |s| Some(FaustGenerator::new(s)))
            .map(|generators| Self { path: String::from(path), generators })
    }

    /// Run generators and build module file.
    pub fn build(&mut self) -> Result<(), String> {
        self.generators.iter_mut().try_for_each(|g| g.build())
            .and_then(|_| self.build_module())
    }

    /// Build `mod.rs`.
    pub fn build_module(&self) -> Result<(), String> {
        let (mut use_modules, mut list_plugins, mut new_plugins) : (Vec<String>, Vec<String>, Vec<String>)
            = (Vec::new(), Vec::new(), Vec::new());

        for generator in self.generators.iter() {
            use_modules.push(format!("pub use {};", generator.name));
            list_plugins.push(format!("\"{}\",", generator.name));
            new_plugins.push(format!("\"{}\" => {}::{}::new(),",
                             generator.name, generator.name, generator.struct_name));
        }

        let modules = self.generators.iter().map(|g| g.name.clone()).collect::<Vec<_>>();
        let content = format!(r#"
            use libfoxlive::dsp::DSP;

            {}

            pub fn list_plugins() -> Vec<String> {{
                vec![{}]
            }}

            pub fn new_plugin(name: &str) -> Option<Box<DSP>> {{
                match name {{
                    {}
                }}
            }}
            "#,
            use_modules.join("\n"),
            list_plugins.join(" "),
            new_plugins.join("\n"),
        );
        let content = content.replace("\n            ", "\n");

        let path = self.path.clone() + "mod.rs";
        fs::write(&path, content).map_err(|_| format!("can't write {}", path))
    }
}


// TODO:
// - impl DSP for dsp struct
// - impl Object for dsp struct

