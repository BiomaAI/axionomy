//! The shipped connect-four package.
use axionomy_engine::{Engine, EngineError};

/// Installs the package built from its bundled source.
pub fn write(engine: &Engine) -> Result<(), EngineError> {
    crate::package::write(engine, axionomy::path!("connect-four"))
}
