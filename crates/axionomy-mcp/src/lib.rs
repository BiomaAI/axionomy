#![doc = include_str!("../README.md")]

mod authoring;
mod handles;
mod http;
mod issues;
mod messages;
mod read;
mod resources;
mod server;
mod store;
mod studies;

pub use handles::{AssessmentId, EconomyId, HandleError};
pub use http::{StatelessHttpService, stateless_http_config, stateless_http_service};
pub use messages::{
    Amounts, AssessResponse, AssessmentStatus, ComputedEntry, Direction, DocumentSummary,
    EconomyCounts, EconomyCreateRequest, EconomyEntries, EconomyExtendRequest, EconomyStored,
    ExchangeRequest, ExchangedResponse, FailureCode, Issue, IssueKind, LineKind,
    ProblemCatalogRequest, ProblemCatalogResponse, QuantityLine, RateEntries, ReplayRequest,
    SearchOutcome, SearchRequest, SearchResponse, SearchSolved, StateRoot, StudySummary,
    ToolFailure, TraceFindRequest, TraceFindResponse, TraceStepRequest, TraceStepResponse,
};
pub use server::AxionomyMcp;
