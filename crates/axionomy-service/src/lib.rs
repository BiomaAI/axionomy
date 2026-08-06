//! Interface-neutral commands, progress, and artifacts for Axionomy.
//!
//! This layer owns application orchestration, not economic truth. Every trace
//! in an artifact is derived by replay through the core; HTTP, CLI, MCP, and
//! Studio are adapters over the same commands and results.

mod adapters;

use axionomy_view::{ProposalView, ViewDocument};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProblemFamily {
    Pathfinding,
    Constraint,
    Production,
    Scheduling,
    Allocation,
    Market,
    StochasticPlanning,
    AdversarialGame,
    PartialObservation,
    TemporalSimulation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    DeterministicSearch,
    WeightedSearch,
    SpecializedAlgorithm,
    ExactPareto,
    ApproximatePareto,
    FeasibilityAssessment,
    MultiAccountExchange,
    AtomicSettlement,
    BranchOptimization,
    MonteCarlo,
    Mcts,
    InformationSetSearch,
    PartialObservation,
    Chance,
    TemporalEffects,
    FungibleCohorts,
    NonFungibleFacts,
    RlProjection,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct StrategyDescriptor {
    pub key: String,
    pub label: String,
    pub description: String,
    pub algorithm: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ProblemDescriptor {
    pub key: String,
    pub title: String,
    pub summary: String,
    pub family: ProblemFamily,
    pub default_strategy: String,
    pub strategies: Vec<StrategyDescriptor>,
    pub capabilities: Vec<Capability>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RunRequest {
    pub problem: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strategy: Option<String>,
    #[serde(default = "default_seed")]
    pub seed: u64,
    #[serde(default = "default_budget")]
    pub budget: u64,
}

const fn default_seed() -> u64 {
    17
}

const fn default_budget() -> u64 {
    128
}

impl RunRequest {
    pub fn new(problem: impl Into<String>) -> Self {
        Self {
            problem: problem.into(),
            strategy: None,
            seed: default_seed(),
            budget: default_budget(),
        }
    }

    pub fn with_strategy(mut self, strategy: impl Into<String>) -> Self {
        self.strategy = Some(strategy.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct RunArtifact {
    pub id: String,
    pub problem: ProblemDescriptor,
    pub request: RunRequest,
    pub selected_document_id: String,
    pub documents: Vec<ViewDocument>,
    #[serde(default)]
    pub assessed_proposals: Vec<ProposalView>,
}

impl RunArtifact {
    pub fn selected_document(&self) -> Option<&ViewDocument> {
        self.documents
            .iter()
            .find(|document| document.id == self.selected_document_id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ServiceProgress {
    pub sequence: u64,
    pub phase: String,
    pub completed: u64,
    pub total: u64,
    pub message: String,
}

pub trait RunObserver {
    fn progress(&mut self, progress: ServiceProgress);
}

impl<F> RunObserver for F
where
    F: FnMut(ServiceProgress),
{
    fn progress(&mut self, progress: ServiceProgress) {
        self(progress);
    }
}

#[derive(Debug, Default)]
pub struct RunControl {
    cancelled: AtomicBool,
    paused: AtomicBool,
}

impl RunControl {
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    pub fn pause(&self) {
        self.paused.store(true, Ordering::Release);
    }

    pub fn resume(&self) {
        self.paused.store(false, Ordering::Release);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }

    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::Acquire)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ServiceError {
    #[error("unknown problem `{0}`")]
    UnknownProblem(String),
    #[error("unknown strategy `{strategy}` for problem `{problem}`")]
    UnknownStrategy { problem: String, strategy: String },
    #[error("run was cancelled")]
    Cancelled,
    #[error("run is paused")]
    Paused,
    #[error("problem `{problem}` could not produce an artifact: {message}")]
    Problem { problem: String, message: String },
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ReferenceService;

impl ReferenceService {
    pub fn catalog(&self) -> Vec<ProblemDescriptor> {
        adapters::catalog()
    }

    pub fn problem(&self, key: &str) -> Option<ProblemDescriptor> {
        self.catalog()
            .into_iter()
            .find(|problem| problem.key == key)
    }

    pub fn run(&self, request: RunRequest) -> Result<RunArtifact, ServiceError> {
        self.run_with(&request, &RunControl::default(), &mut |_| {})
    }

    pub fn run_with(
        &self,
        request: &RunRequest,
        control: &RunControl,
        observer: &mut dyn RunObserver,
    ) -> Result<RunArtifact, ServiceError> {
        if control.is_cancelled() {
            return Err(ServiceError::Cancelled);
        }
        if control.is_paused() {
            return Err(ServiceError::Paused);
        }
        adapters::run(request, control, observer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_covers_every_canonical_problem() {
        let catalog = ReferenceService.catalog();
        assert_eq!(catalog.len(), 12);
        assert!(catalog.iter().all(|problem| !problem.strategies.is_empty()));
    }

    #[test]
    fn every_default_problem_builds_a_selected_replay_artifact() {
        let service = ReferenceService;
        for problem in service.catalog() {
            let artifact = service
                .run(RunRequest::new(&problem.key).with_strategy(&problem.default_strategy))
                .unwrap_or_else(|error| panic!("{} failed: {error}", problem.key));
            assert!(artifact.selected_document().is_some(), "{}", problem.key);
            assert!(!artifact.documents.is_empty(), "{}", problem.key);
            assert!(
                artifact
                    .documents
                    .iter()
                    .all(|document| document.model.is_some()),
                "{}",
                problem.key
            );
        }
    }
}
