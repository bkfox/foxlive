use std::fs::{File,read_dir};
use std::io::{BufReader,BufRead};
use std::path::Path;

/// Return true if a file should regenerated
pub fn source_changed<P: AsRef<Path>>(source: P, dest: P) -> bool {
    let file_mod = source.as_ref().metadata().unwrap().modified().unwrap();
    if let Ok(metadata) = dest.as_ref().metadata() {
        return metadata.modified().unwrap() > file_mod;
    }
    true
}


/// Scan directory for files with the following extension, using the provided `new`
/// method to create collected object infos.
pub fn scan_dir<F,T>(path: &str, extension: &str, new: F) -> Result<Vec<T>, String>
    where F: Fn(&str) -> Option<T>
{
    read_dir(path)
        .map_err(|e| format!("{}", e))
        .map(|dir|
            dir.filter_map(|entry|
                match entry {
                    Ok(entry) => match entry.path().to_str() {
                        Some(path) if !path.starts_with('.') && path.ends_with(extension) => new(path),
                        _ => None
                    },
                    _ => None
                }
            )
            .collect()
        )
}


/// Parse comments of a file for comment with the provided prefix (including comments
/// escape).
///
/// Return a tuple of `("operand", "value")`, such as comments would take
/// the following form:
///
/// ```
/// //: type my_binding_type
/// ```
///
pub fn parse(source: &str, prefix: &str) -> Option<Vec<(String,String)>> {
    File::open(source).ok().map(|file|
        BufReader::new(file).lines()
            .filter_map(move |l| {
                let l = &l.unwrap();
                let l = l.trim_start();
                match l.starts_with(prefix) {
                    true => Some(l.replace(prefix,"").trim().to_string()),
                    false => None
                }
            })
            .map(move |l| {
                let mut l = l.trim().splitn(2,' ');
                let (a,b) = (l.next().unwrap(), l.next().unwrap());
                (String::from(a.trim()), String::from(b.trim()))
            })
            .collect()
        )
}



