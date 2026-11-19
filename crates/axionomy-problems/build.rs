//! Bundle the canonical gallery source for installed binaries and wasm.
use std::{
    env, fs,
    io::Write,
    path::{Path, PathBuf},
};
fn collect(dir: &Path, files: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("read package sources") {
        let path = entry.expect("read source entry").path();
        if path.is_dir() {
            collect(&path, files);
        } else {
            files.push(path);
        }
    }
}
fn main() {
    println!("cargo:rerun-if-changed=packages");
    if env::var_os("CARGO_FEATURE_PACKAGES").is_none() {
        return;
    }
    let mut files = Vec::new();
    collect(Path::new("packages"), &mut files);
    files.sort();
    let mut out =
        fs::File::create(PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("gallery.rs"))
            .unwrap();
    writeln!(out, "&[").unwrap();
    for path in files {
        let mut parts = path.strip_prefix("packages").unwrap().components();
        let package = parts.next().unwrap().as_os_str().to_str().unwrap();
        let file = parts.as_path().to_str().unwrap().replace('\\', "/");
        writeln!(out,"({package:?},{file:?},include_str!(concat!(env!(\"CARGO_MANIFEST_DIR\"),\"/packages/\",{package:?},\"/\",{file:?}))),").unwrap();
    }
    writeln!(out, "]").unwrap();
}
