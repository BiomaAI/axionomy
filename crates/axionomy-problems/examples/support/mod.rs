//! Shared presentation policy for the consumer examples.

use std::io::{self, IsTerminal};
use tracing::info;
use tracing_subscriber::EnvFilter;

pub fn init(example: &'static str, purpose: &'static str) {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .without_time()
        .with_ansi(io::stderr().is_terminal())
        .compact()
        .try_init()
        .expect("an example installs its tracing subscriber once");

    info!(example, "{purpose}");
}
