#![doc = include_str!("../README.md")]

mod http;
mod server;
mod store;
mod wire;

pub use http::{StatelessHttpService, stateless_http_config, stateless_http_service};
pub use server::AxionomyMcp;
pub use store::{MemorySnapshotStore, MemorySnapshotStoreError, SnapshotStore, StoredSnapshot};
pub use wire::{
    ApplyRequest, ApplyResponse, AssessRequest, AssessResponse, EconomyHandle, EconomyPutRequest,
    EconomyPutResponse, ProblemCatalogRequest, ProblemCatalogResponse, ReplayRequest,
    ReplayResponse, SearchRequest, SearchResponse, WireAssessment, WireEconomy, WireExchange,
    WireGoal, WireReceipt, WireTrace,
};
