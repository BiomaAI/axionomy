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
