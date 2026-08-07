//! Interface-neutral commands, progress, and artifacts for Axionomy.
//!
//! This layer owns application orchestration, not economic truth. Every trace
//! in an artifact is derived by replay through the core; HTTP, CLI, MCP, and
//! Studio are adapters over the same commands and results.

mod adapters;

use axionomy_view::{ProposalView, SearchObservationView, ViewDocument};
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum InstanceProfile {
    Micro,
    Showcase,
    Stress,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct InstanceDescriptor {
    pub key: String,
    pub label: String,
    pub description: String,
    pub profile: InstanceProfile,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ProblemDescriptor {
    pub key: String,
    pub title: String,
    pub summary: String,
    pub family: ProblemFamily,
    pub default_instance: String,
    pub instances: Vec<InstanceDescriptor>,
    pub default_strategy: String,
    pub strategies: Vec<StrategyDescriptor>,
    pub capabilities: Vec<Capability>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RunRequest {
    pub problem: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instance: Option<String>,
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
            instance: None,
            strategy: None,
            seed: default_seed(),
            budget: default_budget(),
        }
    }

    pub fn with_strategy(mut self, strategy: impl Into<String>) -> Self {
        self.strategy = Some(strategy.into());
        self
    }

    pub fn with_instance(mut self, instance: impl Into<String>) -> Self {
        self.instance = Some(instance.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct RunArtifact {
    pub id: String,
    pub problem: ProblemDescriptor,
    pub instance: InstanceDescriptor,
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

    fn observation(&mut self, _observation: SearchObservationView) {}
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

    /// Cooperatively waits at an application boundary until the caller resumes
    /// or cancels the run. Search algorithms with finer-grained sessions can
    /// call this between work-budget advances.
    pub fn checkpoint(&self) -> Result<(), ServiceError> {
        while self.is_paused() {
            if self.is_cancelled() {
                return Err(ServiceError::Cancelled);
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        if self.is_cancelled() {
            Err(ServiceError::Cancelled)
        } else {
            Ok(())
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ServiceError {
    #[error("unknown problem `{0}`")]
    UnknownProblem(String),
    #[error("unknown strategy `{strategy}` for problem `{problem}`")]
    UnknownStrategy { problem: String, strategy: String },
    #[error("unknown instance `{instance}` for problem `{problem}`")]
    UnknownInstance { problem: String, instance: String },
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
        control.checkpoint()?;
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
        assert!(catalog.iter().all(|problem| {
            problem.default_instance == "showcase"
                && problem
                    .instances
                    .iter()
                    .map(|instance| instance.profile)
                    .eq([
                        InstanceProfile::Micro,
                        InstanceProfile::Showcase,
                        InstanceProfile::Stress,
                    ])
        }));
    }

    #[test]
    fn every_micro_problem_remains_a_fast_replayable_contract_fixture() {
        let service = ReferenceService;
        for problem in service.catalog() {
            let mut request = RunRequest::new(&problem.key)
                .with_instance("micro")
                .with_strategy(&problem.default_strategy);
            request.budget = 8;
            let artifact = service
                .run(request)
                .unwrap_or_else(|error| panic!("{} micro failed: {error}", problem.key));
            assert_eq!(artifact.instance.profile, InstanceProfile::Micro);
            assert!(artifact.selected_document().is_some());
        }
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
            assert_eq!(artifact.instance.profile, InstanceProfile::Showcase);
            assert!(
                artifact
                    .documents
                    .iter()
                    .all(|document| document.model.is_some()),
                "{}",
                problem.key
            );
            let maximum_frames = artifact
                .documents
                .iter()
                .map(|document| document.frames.len())
                .max()
                .unwrap_or(0);
            let maximum_accounts = artifact
                .documents
                .iter()
                .map(|document| document.initial.accounts.len())
                .max()
                .unwrap_or(0);
            let maximum_rates = artifact
                .documents
                .iter()
                .filter_map(|document| document.model.as_ref())
                .map(|model| model.rates.len())
                .max()
                .unwrap_or(0);
            let (minimum_frames, minimum_accounts, minimum_rates) = match problem.key.as_str() {
                "maze" => (8, 2, 19),
                "sokoban" => (10, 36, 200),
                "exact_cover" => (5, 1, 40),
                "workshop" => (7, 1, 4),
                "scheduling" => (7, 20, 100),
                "rescue" => (7, 2, 70),
                "bridge" => (10, 3, 18),
                "marketplace" => (4, 14, 4),
                "logistics" => (40, 7, 50),
                "connect_four" => (15, 50, 200),
                "mission" => (6, 5, 39),
                "perishables" => (10, 5, 13),
                other => panic!("missing showcase pressure threshold for {other}"),
            };
            assert!(
                maximum_frames >= minimum_frames,
                "{} trace regressed",
                problem.key
            );
            assert!(
                maximum_accounts >= minimum_accounts,
                "{} account surface regressed",
                problem.key
            );
            assert!(
                maximum_rates >= minimum_rates,
                "{} rate surface regressed",
                problem.key
            );
            assert!(artifact.documents.iter().all(|document| {
                document
                    .telemetry
                    .iter()
                    .any(|series| series.algorithm == "artifact complexity")
            }));
            assert!(artifact.documents.iter().all(|document| {
                document.initial.scene.is_some()
                    && document.frames.iter().all(|frame| {
                        frame.before.scene.is_some()
                            && frame.after.scene.is_some()
                            && !frame.cues.is_empty()
                    })
                    && std::iter::once(&document.initial)
                        .chain(document.frames.iter().map(|frame| &frame.after))
                        .filter_map(|snapshot| snapshot.scene.as_ref())
                        .all(|scene| !scene.metrics.is_empty())
            }));
            assert!(artifact.documents.iter().all(|document| {
                std::iter::once(&document.initial)
                    .chain(document.frames.iter().map(|frame| &frame.after))
                    .filter_map(|snapshot| snapshot.scene.as_ref())
                    .any(|scene| !scene.entities.is_empty())
            }));
            assert!(
                artifact
                    .documents
                    .iter()
                    .all(|document| !document.solve_observations.is_empty())
            );
            if problem.key == "connect_four" {
                assert!(maximum_rates < 300, "compact standard board regressed");
            }
        }
    }

    #[test]
    fn paused_run_waits_and_resumes_without_restarting_its_command() {
        let control = std::sync::Arc::new(RunControl::default());
        control.pause();
        let worker_control = std::sync::Arc::clone(&control);
        let worker = std::thread::spawn(move || {
            ReferenceService.run_with(
                &RunRequest::new("maze").with_strategy("a_star"),
                &worker_control,
                &mut |_| {},
            )
        });
        std::thread::sleep(std::time::Duration::from_millis(25));
        assert!(!worker.is_finished());
        control.resume();
        assert!(worker.join().unwrap().is_ok());
    }

    #[test]
    fn logistics_reports_algorithm_progress_and_observes_mid_run_cancellation() {
        let control = RunControl::default();
        let mut observed = Vec::new();
        let result = ReferenceService.run_with(
            &RunRequest {
                problem: "logistics".into(),
                instance: None,
                strategy: Some("reliable".into()),
                seed: 42,
                budget: 64,
            },
            &control,
            &mut |progress: ServiceProgress| {
                if progress.phase == "monte_carlo" && progress.completed >= 16 {
                    control.cancel();
                }
                observed.push(progress);
            },
        );

        assert_eq!(result, Err(ServiceError::Cancelled));
        let monte_carlo = observed
            .iter()
            .filter(|progress| progress.phase == "monte_carlo")
            .collect::<Vec<_>>();
        assert!(monte_carlo.len() >= 2);
        assert!(
            monte_carlo
                .windows(2)
                .all(|pair| pair[0].completed < pair[1].completed)
        );
        assert!(monte_carlo.iter().all(|progress| progress.total == 128));
    }

    #[test]
    fn a_mid_run_pause_blocks_the_next_chunk_until_resume() {
        let control = std::sync::Arc::new(RunControl::default());
        let resume_control = std::sync::Arc::clone(&control);
        let (paused_tx, paused_rx) = std::sync::mpsc::sync_channel(1);
        let resumer = std::thread::spawn(move || {
            paused_rx.recv().unwrap();
            std::thread::sleep(std::time::Duration::from_millis(30));
            assert!(resume_control.is_paused());
            resume_control.resume();
        });
        let mut paused_once = false;
        let result = ReferenceService.run_with(
            &RunRequest {
                problem: "logistics".into(),
                instance: None,
                strategy: Some("reliable".into()),
                seed: 42,
                budget: 16,
            },
            &control,
            &mut |progress: ServiceProgress| {
                if !paused_once && progress.phase == "monte_carlo" {
                    paused_once = true;
                    control.pause();
                    paused_tx.send(()).unwrap();
                }
            },
        );
        resumer.join().unwrap();

        assert!(paused_once);
        assert!(result.is_ok());
    }
}
