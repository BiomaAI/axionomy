#![doc = include_str!("../README.md")]

mod experiments;
mod ids;
mod packages;
mod points;
mod refs;
mod refusal;

pub use experiments::{
    ArmResultView, CompareRequest, ComparisonView, EndedView, ExperimentEvent, PointValues,
    ResultsView, TrialView, check_experiment, compare, results, run_experiment,
};
pub use ids::{Natural, ObjectiveList, PackageVersion, PointId, PointIdError};
pub use packages::{
    ArmView, ExperimentView, KeptView, LoadedView, ObjectiveView, PackageList, PackageView,
    ReleaseView, SamplePackages, load, packages,
};
pub use points::{
    AccountBalances, Ancestor, AncestorQuery, Applied, ApplyRequest, AssessRequest, Balances,
    Candidate, Candidates, CandidatesQuery, OutcomeView, RanView, StrategyEvent, StrategyRequest,
    ancestor, apply, assess, balances, candidates, check_strategy, draw, run_strategy,
};
pub use refs::{
    MAX_FRAMES, MAX_WAIT_SECONDS, RefAdvanceRequest, RefCreateRequest, RefDeleteQuery, RefFrame,
    RefFrames, RefFramesQuery, RefView, advance_ref, create_ref, delete_ref, frames, read_ref,
};
pub use refusal::{ErrorResponse, RefMoved, Refusal, RefusalBody, Refused};
