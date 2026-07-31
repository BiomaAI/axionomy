#![doc = include_str!("../README.md")]

mod server;
mod store;
mod wire;

pub use server::AxionomyMcp;
pub use store::{SqliteStore, StoreError};
pub use wire::{
    ApplyRequest, ApplyResponse, AssessRequest, AssessResponse, EconomyHandle, EconomyPutRequest,
    EconomyPutResponse, ReplayRequest, ReplayResponse, SearchRequest, SearchResponse, WireEconomy,
    WireExchange, WireGoal, WireReceipt, WireTrace,
};
