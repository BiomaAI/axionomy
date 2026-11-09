//! Lean-generated fixtures replayed through the Rust kernel. The fixtures come
//! only from `lake exe conformance`; CI rejects any it did not generate.

mod baskets;
mod invariants;
mod kernel;
mod scripted_iterator;
mod transactions;

const FIXTURES: &str = include_str!("../../verification/conformance.json");
