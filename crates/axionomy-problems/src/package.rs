//! What every sample's package builder shares.

use axionomy_engine::ScriptCall;
use serde::{Serialize, de::DeserializeOwned};

/// The function `function` of the script at `script`.
pub(crate) fn call(script: &str, function: &str) -> ScriptCall {
    ScriptCall::new(
        script.parse().expect("a script's name is a path"),
        function.parse().expect("a function name"),
    )
}

/// A typed value written as paths, which D-044 makes the same bytes.
pub(crate) fn written<T: DeserializeOwned>(value: &impl Serialize) -> T {
    postcard::from_bytes(&postcard::to_allocvec(value).expect("values encode"))
        .expect("a typed value reads as paths")
}

/// The UTF-8 source that a shipped sample builds from, also available to
/// binaries installed outside a checkout and to the Studio worker.
pub fn source(package: &axionomy_engine::PackagePath) -> axionomy_engine::source::SourceFiles {
    const GALLERY: &[(&str, &str, &str)] = include!(concat!(env!("OUT_DIR"), "/gallery.rs"));
    GALLERY
        .iter()
        .filter(|(name, _, _)| *name == package.as_str())
        .map(|(_, file, text)| {
            (
                file.parse().expect("a gallery source file"),
                (*text).to_owned(),
            )
        })
        .collect()
}
