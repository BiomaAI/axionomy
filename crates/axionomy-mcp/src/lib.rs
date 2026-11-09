#![doc = include_str!("../README.md")]

mod authoring;
mod http;
mod read;
mod resources;
mod server;
mod store;
mod wire;

pub use http::{StatelessHttpService, stateless_http_config, stateless_http_service};
pub use server::AxionomyMcp;
pub use store::{
    MemorySnapshotStore, MemorySnapshotStoreError, Origin, Snapshot, SnapshotStore, StoredSnapshot,
};
pub use wire::{
    Amounts, AssessResponse, ComputedEntry, Direction, DocumentSummary, EconomyCounts,
    EconomyCreateRequest, EconomyEntries, EconomyExtendRequest, EconomyStored, ExchangeRequest,
    ExchangedResponse, FailureCode, LineKind, ProblemCatalogRequest, ProblemCatalogResponse,
    QuantityLine, RateEntries, ReplayRequest, RunSummary, SearchOutcome, SearchRequest,
    SearchResponse, SearchSolved, ToolFailure, WireApplyError, WireAssessment, WireCondition,
    WireEconomy, WireExchange, WireExpression, WireReceipt,
};
