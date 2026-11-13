//! The engine in Studio's browser Worker (P11, D-047): [`WorkerEngine`],
//! exported as `Engine`, over the Worker's store on the origin private file
//! system or in memory, offering the operations the Studio server offers,
//! the same functions of `axionomy-studio-api` giving the same JSON (P12).
//!
//! This crate does not introduce a JavaScript world model. Requests,
//! progress, refusals and documents cross the boundary as Rust-owned
//! contracts; the JavaScript Worker only schedules and transports.

#[cfg(target_arch = "wasm32")]
mod opfs;
mod worker;

pub use worker::{Source, Validators, WorkerEngine};
