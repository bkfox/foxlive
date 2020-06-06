use std::path::Path;

use bindgen;
use super::utils::*;


/// Create a bindings builder reading the provided header file. It reads
/// header file to add different type to the builder, such as:
///
/// ```
/// //: use fn function_or_object_name
/// ```
///
/// Where `fn` is one of the following item:
/// - `type`: whitelist type
/// - `fn`: whitelist function
/// - `var`: whitelist var
/// - `enum`: rustified enum
///
pub fn build(path: &str, write: bool) -> Option<bindgen::Builder> {
    let path = Path::new(path);
    let out = path.with_extension("rs");

    if !path.exists() || !source_changed(&path, &out.as_path()) {
        return None;
    }

    let path = path.to_str().unwrap();
    let out = out.to_str().unwrap();

    let mut bindings = bindgen::Builder::default().header(path);

    match parse(path, "//:") {
        Some(items) =>
            for (ref ty, ref val) in items.iter() {
                bindings = match ty.as_ref() {
                    "type" => bindings.whitelist_type(val),
                    "fn" => bindings.whitelist_function(val),
                    "var" => bindings.whitelist_var(val),
                    "enum" => bindings.rustified_enum(val),
                    _ => bindings,
                }
            },
        None => return None,
    };

    bindings = bindings.derive_default(true);
    if write {
        bindings.generate()
                .expect(&format!("{}: unable to generate bindings", path))
                .write_to_file(out)
                .expect(&format!("{}: couldn't write bindings into {}", path, out));
        None
    }
    else { Some(bindings) }
}


