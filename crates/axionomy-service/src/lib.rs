//! Interface-neutral commands, progress, and artifacts for Axionomy.
//!
//! This layer owns application orchestration, not economic truth. Every trace
//! in an artifact is derived by replay through the core; HTTP, CLI, MCP, and
//! Studio are adapters over the same commands and results.

mod adapters;

use axionomy::path::Path;
use axionomy_view::{
    DocumentPath, ExchangeFrame, PhasePath, ProposalView, SearchObservationView, ViewDocument,
};
use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{
    borrow::Cow,
    fmt,
    str::FromStr,
    sync::atomic::{AtomicBool, Ordering},
};
use thiserror::Error;

/// The kinds of identifiers the service defines.
pub mod kind {
    use axionomy::PathKind;

    /// A problem in the catalog.
    pub enum Problem {}
    /// A strategy for solving a problem.
    pub enum Strategy {}

    impl PathKind for Problem {
        const SCHEMA: &'static str = "ProblemPath";
    }
    impl PathKind for Strategy {
        const SCHEMA: &'static str = "StrategyPath";
    }
}

/// A problem, such as `maze` or `work-league`.
pub type ProblemPath = Path<kind::Problem>;
/// A strategy, such as `a-star` or `pareto-energy`.
pub type StrategyPath = Path<kind::Strategy>;

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
    MultiAgentCompetition,
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
    ReplayDerivedLeaderboards,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct StrategyDescriptor {
    pub key: StrategyPath,
    pub label: String,
    pub description: String,
    /// The algorithm in words, such as `breadth-first search`.
    pub algorithm: String,
}

/// How large an instance of a problem is. Every problem has one instance of
/// each size.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum InstanceProfile {
    Micro,
    Showcase,
    Stress,
}

impl InstanceProfile {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Micro => "micro",
            Self::Showcase => "showcase",
            Self::Stress => "stress",
        }
    }
}

impl fmt::Display for InstanceProfile {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for InstanceProfile {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        [Self::Micro, Self::Showcase, Self::Stress]
            .into_iter()
            .find(|profile| profile.as_str() == text)
            .ok_or_else(|| format!("`{text}` is not micro, showcase or stress"))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct InstanceDescriptor {
    pub profile: InstanceProfile,
    pub label: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ProblemDescriptor {
    pub key: ProblemPath,
    pub title: String,
    pub summary: String,
    pub family: ProblemFamily,
    pub default_instance: InstanceProfile,
    pub instances: Vec<InstanceDescriptor>,
    pub default_strategy: StrategyPath,
    pub strategies: Vec<StrategyDescriptor>,
    pub capabilities: Vec<Capability>,
}

/// A request to run a problem. The instance and strategy default to the
/// problem's own.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RunRequest {
    pub problem: ProblemPath,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instance: Option<InstanceProfile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strategy: Option<StrategyPath>,
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
    pub fn new(problem: ProblemPath) -> Self {
        Self {
            problem,
            instance: None,
            strategy: None,
            seed: default_seed(),
            budget: default_budget(),
        }
    }

    pub fn with_strategy(mut self, strategy: StrategyPath) -> Self {
        self.strategy = Some(strategy);
        self
    }

    pub fn with_instance(mut self, instance: InstanceProfile) -> Self {
        self.instance = Some(instance);
        self
    }
}

/// What an artifact was computed from: a request with its instance and
/// strategy decided. The same key always gives the same artifact.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct ArtifactKey {
    pub problem: ProblemPath,
    pub instance: InstanceProfile,
    pub strategy: StrategyPath,
    pub seed: u64,
    pub budget: u64,
}

impl fmt::Display for ArtifactKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} · {} · {} · seed {} · budget {}",
            self.problem, self.instance, self.strategy, self.seed, self.budget
        )
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct RunArtifact {
    pub key: ArtifactKey,
    pub problem: ProblemDescriptor,
    pub instance: InstanceDescriptor,
    /// The document of the requested strategy.
    pub selected_document: DocumentPath,
    pub documents: Vec<ViewDocument>,
    #[serde(default)]
    pub assessed_proposals: Vec<ProposalView>,
}

impl RunArtifact {
    pub fn selected(&self) -> Option<&ViewDocument> {
        self.document(&self.selected_document)
    }

    pub fn document(&self, id: &DocumentPath) -> Option<&ViewDocument> {
        self.documents.iter().find(|document| &document.id == id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ServiceProgress {
    pub sequence: u64,
    pub phase: PhasePath,
    pub completed: u64,
    pub total: u64,
    pub message: String,
}

pub trait RunObserver {
    fn progress(&mut self, progress: ServiceProgress);

    fn observation(&mut self, _observation: SearchObservationView) {}

    fn frame(&mut self, _document: &DocumentPath, _frame: ExchangeFrame) {}
}

impl<F> RunObserver for F
where
    F: FnMut(ServiceProgress),
{
    fn progress(&mut self, progress: ServiceProgress) {
        self(progress);
    }
}

/// A Studio run, written `run-` and a number. Each Studio engine numbers its
/// own runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StudioRunId(u64);

impl StudioRunId {
    pub const fn new(number: u64) -> Self {
        Self(number)
    }
}

impl fmt::Display for StudioRunId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "run-{}", self.0)
    }
}

/// Text that is not a Studio run ID.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("`{0}` is not a Studio run ID such as `run-7`")]
pub struct StudioRunIdError(String);

impl FromStr for StudioRunId {
    type Err = StudioRunIdError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        text.strip_prefix("run-")
            .filter(|digits| {
                !digits.is_empty()
                    && digits.bytes().all(|byte| byte.is_ascii_digit())
                    && (*digits == "0" || !digits.starts_with('0'))
            })
            .and_then(|digits| digits.parse().ok())
            .map(Self)
            .ok_or_else(|| StudioRunIdError(text.to_owned()))
    }
}

impl Serialize for StudioRunId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for StudioRunId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Cow::<str>::deserialize(deserializer)?
            .parse()
            .map_err(serde::de::Error::custom)
    }
}

impl JsonSchema for StudioRunId {
    fn schema_name() -> Cow<'static, str> {
        "StudioRunId".into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "description": "A Studio run: `run-` and a number, such as `run-7`.",
            "type": "string",
            "pattern": "^run-(0|[1-9][0-9]*)$"
        })
    }
}

/// Lifecycle events emitted by a Studio run. `sequence` orders transport
/// events; it is not economic time and has no effect on replay.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum StudioEvent {
    RunStarted {
        run_id: StudioRunId,
        sequence: u64,
        problem: ProblemPath,
        strategy: StrategyPath,
    },
    Progress {
        run_id: StudioRunId,
        sequence: u64,
        completed: u64,
        #[serde(skip_serializing_if = "Option::is_none")]
        total: Option<u64>,
        message: String,
    },
    SearchObservation {
        run_id: StudioRunId,
        sequence: u64,
        observation: SearchObservationView,
    },
    FrameAppended {
        run_id: StudioRunId,
        sequence: u64,
        document: DocumentPath,
        frame_index: u64,
        frame: Box<ExchangeFrame>,
    },
    /// The run's artifact can be fetched.
    ArtifactPublished {
        run_id: StudioRunId,
        sequence: u64,
        key: ArtifactKey,
        documents: u64,
    },
    RunPaused {
        run_id: StudioRunId,
        sequence: u64,
    },
    RunResumed {
        run_id: StudioRunId,
        sequence: u64,
    },
    RunCompleted {
        run_id: StudioRunId,
        sequence: u64,
        /// The artifact's selected document.
        document: DocumentPath,
    },
    RunCancelled {
        run_id: StudioRunId,
        sequence: u64,
    },
    RunFailed {
        run_id: StudioRunId,
        sequence: u64,
        message: String,
    },
}

impl StudioEvent {
    pub const fn sequence(&self) -> u64 {
        match self {
            Self::RunStarted { sequence, .. }
            | Self::Progress { sequence, .. }
            | Self::SearchObservation { sequence, .. }
            | Self::FrameAppended { sequence, .. }
            | Self::ArtifactPublished { sequence, .. }
            | Self::RunPaused { sequence, .. }
            | Self::RunResumed { sequence, .. }
            | Self::RunCompleted { sequence, .. }
            | Self::RunCancelled { sequence, .. }
            | Self::RunFailed { sequence, .. } => *sequence,
        }
    }

    pub const fn run_id(&self) -> StudioRunId {
        match self {
            Self::RunStarted { run_id, .. }
            | Self::Progress { run_id, .. }
            | Self::SearchObservation { run_id, .. }
            | Self::FrameAppended { run_id, .. }
            | Self::ArtifactPublished { run_id, .. }
            | Self::RunPaused { run_id, .. }
            | Self::RunResumed { run_id, .. }
            | Self::RunCompleted { run_id, .. }
            | Self::RunCancelled { run_id, .. }
            | Self::RunFailed { run_id, .. } => *run_id,
        }
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
    UnknownProblem(ProblemPath),
    #[error("unknown strategy `{strategy}` for problem `{problem}`")]
    UnknownStrategy {
        problem: ProblemPath,
        strategy: StrategyPath,
    },
    #[error("problem `{problem}` has no {instance} instance")]
    UnknownInstance {
        problem: ProblemPath,
        instance: InstanceProfile,
    },
    #[error("run was cancelled")]
    Cancelled,
    #[error("run is paused")]
    Paused,
    #[error("problem `{problem}` could not produce an artifact: {message}")]
    Problem {
        problem: ProblemPath,
        message: String,
    },
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ReferenceService;

impl ReferenceService {
    pub fn catalog(&self) -> Vec<ProblemDescriptor> {
        adapters::catalog()
    }

    pub fn problem(&self, key: &ProblemPath) -> Option<ProblemDescriptor> {
        self.catalog()
            .into_iter()
            .find(|problem| &problem.key == key)
    }

    /// Decides a request's instance and strategy, checking the problem
    /// offers them.
    pub fn resolve(&self, request: &RunRequest) -> Result<ArtifactKey, ServiceError> {
        let problem = self
            .problem(&request.problem)
            .ok_or_else(|| ServiceError::UnknownProblem(request.problem.clone()))?;
        let instance = request.instance.unwrap_or(problem.default_instance);
        if !problem
            .instances
            .iter()
            .any(|known| known.profile == instance)
        {
            return Err(ServiceError::UnknownInstance {
                problem: problem.key,
                instance,
            });
        }
        let strategy = request
            .strategy
            .clone()
            .unwrap_or_else(|| problem.default_strategy.clone());
        if !problem.strategies.iter().any(|known| known.key == strategy) {
            return Err(ServiceError::UnknownStrategy {
                problem: problem.key,
                strategy,
            });
        }
        Ok(ArtifactKey {
            problem: problem.key,
            instance,
            strategy,
            seed: request.seed,
            budget: request.budget,
        })
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
        adapters::run(&self.resolve(request)?, control, observer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axionomy_problems::lending;
    use axionomy_view::{SceneSurfaceView, ViewOntology, slug};
    use std::collections::HashSet;

    #[derive(Default)]
    struct FrameObserver {
        documents: Vec<DocumentPath>,
        frames: Vec<ExchangeFrame>,
    }

    impl RunObserver for FrameObserver {
        fn progress(&mut self, _: ServiceProgress) {}

        fn frame(&mut self, document: &DocumentPath, frame: ExchangeFrame) {
            self.documents.push(document.clone());
            self.frames.push(frame);
        }
    }

    fn assert_unique<'a>(problem: &str, kind: &str, ids: impl Iterator<Item = &'a str>) {
        let mut seen = HashSet::new();
        for id in ids {
            assert!(seen.insert(id), "{problem} repeats {kind} `{id}`");
        }
    }

    fn has_linked_entity_change(document: &ViewDocument) -> bool {
        document.frames.iter().any(|frame| {
            let Some(before) = frame.before.scene.as_ref() else {
                return false;
            };
            let Some(after) = frame.after.scene.as_ref() else {
                return false;
            };
            before.entities.iter().any(|left| {
                !left.evidence.is_empty()
                    && after
                        .entities
                        .iter()
                        .find(|right| right.id.key == left.id.key)
                        .is_none_or(|right| {
                            left.anchor != right.anchor
                                || left.status != right.status
                                || left.tone != right.tone
                                || left.metrics != right.metrics
                        })
            }) || after.entities.iter().any(|right| {
                !right.evidence.is_empty()
                    && !before
                        .entities
                        .iter()
                        .any(|left| left.id.key == right.id.key)
            })
        })
    }

    #[test]
    fn studio_events_are_discriminated_unions_of_minted_runs() {
        let event = StudioEvent::Progress {
            run_id: StudioRunId::new(1),
            sequence: 2,
            completed: 3,
            total: Some(5),
            message: "replaying".into(),
        };
        let value = serde_json::to_value(&event).unwrap();
        assert_eq!(value["kind"], "progress");
        assert_eq!(value["run_id"], "run-1");
        let decoded: StudioEvent = serde_json::from_value(value).unwrap();
        assert_eq!(decoded, event);
        assert_eq!(decoded.run_id(), StudioRunId::new(1));
        for forged in ["run-", "run-01", "run--1", "Run-1", "run-1x", "browser-1"] {
            assert!(forged.parse::<StudioRunId>().is_err(), "{forged}");
        }
    }

    #[test]
    fn catalog_covers_every_canonical_problem() {
        let catalog = ReferenceService.catalog();
        assert_eq!(catalog.len(), 15);
        assert!(catalog.iter().all(|problem| !problem.strategies.is_empty()));
        assert!(catalog.iter().all(|problem| {
            problem.default_instance == InstanceProfile::Showcase
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
        assert!(catalog.iter().all(|problem| {
            problem
                .instances
                .iter()
                .map(|instance| instance.label.as_str())
                .eq(["Micro", "Showcase", "Stress"])
                && problem.instances.iter().all(|instance| {
                    !instance.description.is_empty()
                        && !instance.description.contains("Larger seeded workload")
                })
                && problem.strategies.iter().all(|strategy| {
                    !strategy.algorithm.is_empty()
                        && strategy.algorithm != strategy.description.to_lowercase()
                })
        }));
    }

    #[test]
    fn maze_retains_strategy_specific_search_and_route_evidence() {
        use axionomy_view::{AssessmentStatusView, ScenePathStatusView, SearchObservationKindView};

        let artifact = ReferenceService
            .run(
                RunRequest::new(slug("maze"))
                    .with_instance(InstanceProfile::Showcase)
                    .with_strategy(slug("a-star")),
            )
            .unwrap();
        for (document, algorithm) in [
            ("breadth-first", "breadth-first"),
            ("dijkstra", "dijkstra"),
            ("a-star", "a-star"),
        ] {
            let document = artifact.document(&slug(document)).unwrap();
            assert!(
                document
                    .solve_observations
                    .iter()
                    .all(|observation| observation.algorithm.as_str() == algorithm)
            );
            let accepted = document.solve_observations.last().unwrap();
            assert_eq!(accepted.kind, SearchObservationKindView::Incumbent);
            assert!(accepted.label.starts_with("Solution accepted at Exit"));
            assert_eq!(accepted.subjects[0].key.as_str(), "node/exit");
        }
        for document in ["pareto-energy", "pareto-time"] {
            let document = artifact.document(&slug(document)).unwrap();
            let observation = document.solve_observations.last().unwrap();
            assert_eq!(observation.algorithm.as_str(), "exact-pareto-search");
            assert_eq!(observation.kind, SearchObservationKindView::Frontier);
            assert!(observation.label.contains("4 non-dominated tradeoffs"));
        }

        let selected = artifact.selected().unwrap();
        assert!(
            selected
                .initial
                .scene
                .as_ref()
                .unwrap()
                .paths
                .iter()
                .any(|path| path.status == ScenePathStatusView::Candidate)
        );
        assert_eq!(
            selected.frames[0]
                .after
                .scene
                .as_ref()
                .unwrap()
                .paths
                .iter()
                .filter(|path| path.status == ScenePathStatusView::Current)
                .count(),
            1
        );
        assert_eq!(
            selected.proposals[0].assessment.status,
            AssessmentStatusView::Infeasible
        );
        let missing = selected.proposals[0]
            .assessment
            .shortfalls
            .iter()
            .flat_map(|shortfall| shortfall.missing.iter())
            .map(|asset| asset.asset.label.as_str())
            .collect::<Vec<_>>();
        assert!(missing.contains(&"Standing in Gate"));
        assert!(missing.contains(&"Vault gate open"));
    }

    #[test]
    fn selected_replay_frames_stream_with_the_same_snapshot_contract() {
        let request = RunRequest::new(slug("work-league"))
            .with_instance(InstanceProfile::Micro)
            .with_strategy(slug("mixed-field"));
        let mut observer = FrameObserver::default();
        let artifact = ReferenceService
            .run_with(&request, &RunControl::default(), &mut observer)
            .unwrap();
        let selected = artifact.selected().unwrap();
        assert_eq!(observer.frames.len(), selected.frames.len());
        assert!(
            observer
                .documents
                .iter()
                .all(|document| document == &selected.id)
        );
        assert!(
            observer
                .frames
                .iter()
                .all(|frame| !frame.after.leaderboards.is_empty())
        );
        assert_eq!(observer.frames.last(), selected.frames.last());
    }

    #[test]
    fn formerly_aliased_stress_models_are_structurally_distinct() {
        use axionomy_problems::{bridge, exact_cover, marketplace, maze, rescue, sokoban};

        let maze_showcase = maze::initial_showcase();
        let maze_stress = maze::initial_stress();
        assert_ne!(maze_showcase.state_key(), maze_stress.state_key());
        assert_ne!(
            maze_showcase.rate_ids().count(),
            maze_stress.rate_ids().count()
        );

        let cover_showcase = exact_cover::initial_showcase();
        let cover_stress = exact_cover::initial_stress();
        assert_ne!(cover_showcase.state_key(), cover_stress.state_key());
        assert_ne!(
            cover_showcase.rate_ids().count(),
            cover_stress.rate_ids().count()
        );

        let warehouse_showcase = sokoban::initial_showcase();
        let warehouse_stress = sokoban::initial_stress();
        assert_eq!(
            warehouse_showcase
                .rate_ids()
                .filter(|rate| matches!(rate, sokoban::RateId::Latch { .. }))
                .count(),
            1
        );
        assert_eq!(
            warehouse_stress
                .rate_ids()
                .filter(|rate| matches!(rate, sokoban::RateId::Latch { .. }))
                .count(),
            2
        );
        assert!(
            warehouse_showcase
                .accounts()
                .any(|(_, account)| account.balance(&sokoban::Asset::ExitCell).to_u64() == 1)
        );

        let bridge_showcase = bridge::initial_showcase();
        let bridge_stress = bridge::initial_stress();
        assert_ne!(bridge_showcase.state_key(), bridge_stress.state_key());

        let market_showcase = marketplace::initial_showcase();
        let market_stress = marketplace::initial_stress();
        assert_ne!(market_showcase.state_key(), market_stress.state_key());
        assert_ne!(
            market_showcase.rate_ids().count(),
            market_stress.rate_ids().count()
        );

        let rescue_showcase = rescue::uniform_uncertain_showcase();
        let rescue_stress = rescue::uniform_uncertain_stress();
        assert_ne!(rescue_showcase.state_key(), rescue_stress.state_key());
        assert_ne!(
            rescue_showcase.rate_ids().count(),
            rescue_stress.rate_ids().count()
        );
    }

    #[test]
    fn every_micro_problem_remains_a_fast_replayable_contract_fixture() {
        let service = ReferenceService;
        for problem in service.catalog() {
            let mut request = RunRequest::new(problem.key.clone())
                .with_instance(InstanceProfile::Micro)
                .with_strategy(problem.default_strategy.clone());
            request.budget = 8;
            let artifact = service
                .run(request)
                .unwrap_or_else(|error| panic!("{} micro failed: {error}", problem.key));
            assert_eq!(artifact.instance.profile, InstanceProfile::Micro);
            let selected = artifact.selected().expect("selected document");
            assert!(selected.frames.iter().all(|frame| {
                !frame.exchange.rate.label.contains('{')
                    && !frame.exchange.rate.label.contains("encoded")
                    && frame
                        .cues
                        .first()
                        .is_some_and(|cue| cue.label == frame.exchange.rate.label)
            }));
            if matches!(
                problem.key.as_str(),
                "maze"
                    | "bridge"
                    | "workshop"
                    | "marketplace"
                    | "logistics"
                    | "mission"
                    | "rescue"
                    | "work-league"
            ) {
                assert!(
                    std::iter::once(&selected.initial)
                        .chain(selected.frames.iter().map(|frame| &frame.after))
                        .filter_map(|snapshot| snapshot.scene.as_ref())
                        .flat_map(|scene| &scene.entities)
                        .any(|entity| !entity.evidence.is_empty()),
                    "{} graph has no entity linked to economic evidence",
                    problem.key
                );
                assert!(
                    has_linked_entity_change(selected),
                    "{} graph has no replay-visible linked entity change",
                    problem.key
                );
            }
        }
    }

    #[test]
    fn scaled_assets_are_read_in_their_units() {
        let artifact = ReferenceService
            .run(RunRequest::new(slug("lending")))
            .expect("lending runs");
        let document = artifact.selected().expect("a selected document");
        let key = |asset| {
            axionomy_view::DebugOntology::<lending::Ontology>::default()
                .asset(&asset)
                .key
        };
        let base_rate = key(lending::Asset::BaseRate);
        let format = &document.formats[&base_rate];
        let quantity = document
            .initial
            .accounts
            .iter()
            .flat_map(|account| &account.balances)
            .find(|balance| balance.asset.key == base_rate)
            .map(|balance| balance.quantity.0.parse().expect("an exact quantity"))
            .expect("the market holds its base rate");
        assert_eq!(format.write(quantity), "0.2 %");
        assert!(
            document
                .formats
                .contains_key(&key(lending::Asset::BorrowIndex))
        );
        // Counted assets need no entry.
        assert!(!document.formats.contains_key(&key(lending::Asset::Share)));
    }

    /// Studio labels read as words, not code, and each names one thing: no two
    /// different accounts, assets, rates or roles of a document share a label.
    fn assert_readable_labels(problem: &str, artifact: &RunArtifact) {
        use axionomy_view::{ItemPath, ViewId};
        use std::collections::{BTreeMap, BTreeSet};
        type Labels<'a> = BTreeMap<(&'static str, &'a str), BTreeSet<&'a ItemPath>>;
        fn name<'a>(labels: &mut Labels<'a>, kind: &'static str, id: &'a ViewId) {
            labels
                .entry((kind, id.label.as_str()))
                .or_default()
                .insert(&id.key);
        }
        let reads_as_code = |label: &str| {
            label.contains("::")
                || label.contains('_')
                || label
                    .as_bytes()
                    .windows(2)
                    .any(|pair| pair[0].is_ascii_lowercase() && pair[1].is_ascii_uppercase())
        };
        for document in &artifact.documents {
            let mut labels = Labels::new();
            let snapshots = std::iter::once(&document.initial)
                .chain(document.frames.iter().map(|frame| &frame.after));
            for snapshot in snapshots {
                for account in &snapshot.accounts {
                    name(&mut labels, "account", &account.account);
                    for balance in &account.balances {
                        name(&mut labels, "asset", &balance.asset);
                    }
                }
            }
            for frame in &document.frames {
                for binding in &frame.exchange.bindings {
                    name(&mut labels, "role", &binding.role);
                }
            }
            for rate in document.model.iter().flat_map(|model| &model.rates) {
                name(&mut labels, "rate", &rate.rate);
            }
            for ((kind, label), keys) in &labels {
                assert!(
                    !reads_as_code(label),
                    "{problem}: {kind} label `{label}` reads as code"
                );
                assert!(
                    keys.len() == 1,
                    "{problem}: {} different {kind}s are all labeled `{label}`: {keys:?}",
                    keys.len()
                );
            }
        }
    }

    #[test]
    fn every_default_problem_builds_a_selected_replay_artifact() {
        let service = ReferenceService;
        for problem in service.catalog() {
            let artifact = service
                .run(
                    RunRequest::new(problem.key.clone())
                        .with_strategy(problem.default_strategy.clone()),
                )
                .unwrap_or_else(|error| panic!("{} failed: {error}", problem.key));
            assert!(artifact.selected().is_some(), "{}", problem.key);
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
            assert_readable_labels(problem.key.as_str(), &artifact);
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
                "sokoban" => (20, 36, 100),
                "exact-cover" => (5, 1, 40),
                "workshop" => (7, 1, 4),
                "scheduling" => (20, 6, 80),
                "rescue" => (7, 2, 70),
                "bridge" => (10, 3, 18),
                "marketplace" => (4, 14, 4),
                "logistics" => (40, 7, 50),
                "connect-four" => (15, 50, 200),
                "mission" => (6, 5, 39),
                "perishables" => (10, 5, 13),
                "work-league" => (60, 20, 900),
                "amm" => (14, 11, 10),
                "lending" => (14, 10, 14),
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
                    .any(|series| series.algorithm == "Model size")
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
            for document in &artifact.documents {
                let model = document.model.as_ref().expect("model definition");
                assert_unique(
                    problem.key.as_str(),
                    "invariant name",
                    model
                        .invariants
                        .iter()
                        .map(|invariant| invariant.name.as_str()),
                );
                for scene in std::iter::once(&document.initial)
                    .chain(document.frames.iter().map(|frame| &frame.after))
                    .filter_map(|snapshot| snapshot.scene.as_ref())
                {
                    assert_unique(
                        problem.key.as_str(),
                        "scene entity ID",
                        scene.entities.iter().map(|entity| entity.id.key.as_str()),
                    );
                    assert_unique(
                        problem.key.as_str(),
                        "scene path ID",
                        scene.paths.iter().map(|path| path.id.as_str()),
                    );
                    assert_unique(
                        problem.key.as_str(),
                        "scene annotation ID",
                        scene
                            .annotations
                            .iter()
                            .map(|annotation| annotation.id.as_str()),
                    );
                    if let SceneSurfaceView::Graph { nodes, edges, .. } = &scene.surface {
                        assert_unique(
                            problem.key.as_str(),
                            "graph node ID",
                            nodes.iter().map(|node| node.id.key.as_str()),
                        );
                        assert_unique(
                            problem.key.as_str(),
                            "graph edge ID",
                            edges.iter().map(|edge| edge.id.as_str()),
                        );
                    }
                }
            }
            assert!(
                artifact
                    .documents
                    .iter()
                    .all(|document| !document.solve_observations.is_empty())
            );
            if problem.key.as_str() == "connect-four" {
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
                &RunRequest::new(slug("maze")).with_strategy(slug("a-star")),
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
                problem: slug("logistics"),
                instance: Some(InstanceProfile::Micro),
                strategy: Some(slug("reliable")),
                seed: 42,
                budget: 64,
            },
            &control,
            &mut |progress: ServiceProgress| {
                if progress.phase.as_str() == "monte-carlo" && progress.completed >= 16 {
                    control.cancel();
                }
                observed.push(progress);
            },
        );

        assert_eq!(result, Err(ServiceError::Cancelled));
        let monte_carlo = observed
            .iter()
            .filter(|progress| progress.phase.as_str() == "monte-carlo")
            .collect::<Vec<_>>();
        assert!(monte_carlo.len() >= 2);
        assert!(
            monte_carlo
                .windows(2)
                .all(|pair| pair[0].completed < pair[1].completed)
        );
        assert!(monte_carlo.iter().all(|progress| progress.total == 32));
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
                problem: slug("logistics"),
                instance: Some(InstanceProfile::Micro),
                strategy: Some(slug("reliable")),
                seed: 42,
                budget: 16,
            },
            &control,
            &mut |progress: ServiceProgress| {
                if !paused_once && progress.phase.as_str() == "monte-carlo" {
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
