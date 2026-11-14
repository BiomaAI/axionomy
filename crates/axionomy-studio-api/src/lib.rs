#![doc = include_str!("../README.md")]

mod divergence;
mod experiments;
mod histories;
mod ids;
mod packages;
mod points;
mod refs;
mod refusal;

pub use divergence::{
    AccountDifferenceView, AlignedView, BalanceDifferenceView, DivergenceQuery, DivergenceView,
    MAX_STEPS, SideView, TipView, divergence,
};
pub use experiments::{
    ArmResultView, CompareRequest, ComparisonView, EndedView, ExperimentEvent, ExperimentProgress,
    PointValues, ResultsView, TrialView, check_experiment, compare, results, run_experiment,
};
pub use histories::{ChangeView, HistoriesView, SegmentView, histories};
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
