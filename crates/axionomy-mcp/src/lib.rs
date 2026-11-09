#![doc = include_str!("../README.md")]

mod authoring;
mod handles;
mod http;
mod messages;
mod read;
mod resources;
mod server;
mod store;

pub use handles::{AssessmentId, EconomyId, HandleError};
pub use http::{StatelessHttpService, stateless_http_config, stateless_http_service};
pub use messages::{
    Amounts, AssessResponse, ComputedEntry, Direction, DocumentSummary, EconomyCounts,
    EconomyCreateRequest, EconomyEntries, EconomyExtendRequest, EconomyStored, ExchangeRequest,
    ExchangedResponse, FailureCode, LineKind, ProblemCatalogRequest, ProblemCatalogResponse,
    QuantityLine, RateEntries, ReplayRequest, RunSummary, SearchOutcome, SearchRequest,
    SearchResponse, SearchSolved, StateRoot, ToolFailure, TraceFindRequest, TraceFindResponse,
    TraceStepRequest,
};
pub use server::AxionomyMcp;
