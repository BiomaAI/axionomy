//! Interface-neutral commands, progress, and studies for Axionomy.
//!
//! A study is what the service computes for a request: a problem's instance
//! under its strategies, with a seed and a budget. Each of its documents holds
//! a run, one trajectory, and the evidence of how it was found. An observer
//! sees each trajectory written as paths, so a study's runs can be recorded
//! and explored like any economy.
//!
//! This layer owns application orchestration, not economic truth. Every trace
//! in a study is derived by replay through the core; HTTP, CLI, MCP, and
//! Studio are adapters over the same commands and results.

mod documents;
mod problem;
mod progress;

use axionomy::path::Path;
use axionomy_view::{
    DocumentPath, ExchangeFrame, PhasePath, ProposalView, SearchObservationView, ViewDocument,
};
pub use documents::{
    DocumentSpec, ProposalSpec, document, document_with_leaderboards_observed, frontier_trace,
    proposal, proposal_at, simulate, telemetry,
};
pub use problem::{
    DuplicateProblem, Problem, Registry, problem_error, strategy, strategy_document,
};
pub use progress::{StudyProgress, Work, graph_metrics, retain_observation};
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
        const EXAMPLE: &'static str = "maze";
    }
    impl PathKind for Strategy {
        const SCHEMA: &'static str = "StrategyPath";
        const EXAMPLE: &'static str = "a-star";
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

/// A request to study a problem. The instance and strategy default to the
/// problem's own.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StudyRequest {
    /// The problem to study: a key the catalog lists, such as `maze`.
    pub problem: ProblemPath,
    /// The size of the problem to study; the problem's default instance when
    /// omitted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instance: Option<InstanceProfile>,
    /// The strategy whose run the study selects as its document; the
    /// problem's default strategy when omitted. The study holds the problem's
    /// other outcomes beside it, usually its other strategies' runs and
    /// cases such as a deadlock.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strategy: Option<StrategyPath>,
    /// Seeds every random choice of the problem and its strategies, so the
    /// same request always gives the same study.
    #[serde(default = "default_seed")]
    pub seed: u64,
    /// How much work each strategy may spend, at least 1: each problem scales
    /// it to its instance as a search, sampling or planning budget.
    #[serde(default = "default_budget")]
    #[schemars(range(min = 1))]
    pub budget: u64,
}

const fn default_seed() -> u64 {
    17
}

const fn default_budget() -> u64 {
    128
}

impl StudyRequest {
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

/// What a study was computed from: a request with its instance and strategy
/// decided. The same key always gives the same study.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct StudyKey {
    pub problem: ProblemPath,
    pub instance: InstanceProfile,
    pub strategy: StrategyPath,
    pub seed: u64,
    pub budget: u64,
}

impl fmt::Display for StudyKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} · {} · {} · seed {} · budget {}",
            self.problem, self.instance, self.strategy, self.seed, self.budget
        )
    }
}

/// A finished study: the replay-derived documents of its alternatives, the
/// one its strategy selects, and the proposals assessed along the way.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Study {
    pub key: StudyKey,
    pub problem: ProblemDescriptor,
    pub instance: InstanceDescriptor,
    /// The document of the requested strategy.
    pub selected_document: DocumentPath,
    pub documents: Vec<ViewDocument>,
    #[serde(default)]
    pub assessed_proposals: Vec<ProposalView>,
}

impl Study {
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

pub trait StudyObserver {
    fn progress(&mut self, progress: ServiceProgress);

    fn observation(&mut self, _observation: SearchObservationView) {}

    fn frame(&mut self, _document: &DocumentPath, _frame: ExchangeFrame) {}

    /// A document's trajectory: the economy it starts from and the trace its
    /// frames replay, written as paths (D-044), so an observer can record it
    /// or explore it as an ordinary economy. Reported once per document,
    /// after its frames are derived.
    fn trajectory(
        &mut self,
        _document: &DocumentPath,
        _initial: &axionomy::paths::Economy,
        _trace: &axionomy::paths::Trace,
    ) {
    }
}

impl<F> StudyObserver for F
where
    F: FnMut(ServiceProgress),
{
    fn progress(&mut self, progress: ServiceProgress) {
        self(progress);
    }
}

/// Names a study a Studio engine started, written `study-` and a number. Each
/// Studio engine numbers its own studies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StudioStudyId(u64);

impl StudioStudyId {
    pub const fn new(number: u64) -> Self {
        Self(number)
    }
}

impl fmt::Display for StudioStudyId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "study-{}", self.0)
    }
}

/// Text that is not a Studio study ID.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("`{0}` is not a Studio study ID such as `study-7`")]
pub struct StudioStudyIdError(String);

impl FromStr for StudioStudyId {
    type Err = StudioStudyIdError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        text.strip_prefix("study-")
            .filter(|digits| {
                !digits.is_empty()
                    && digits.bytes().all(|byte| byte.is_ascii_digit())
                    && (*digits == "0" || !digits.starts_with('0'))
            })
            .and_then(|digits| digits.parse().ok())
            .map(Self)
            .ok_or_else(|| StudioStudyIdError(text.to_owned()))
    }
}

impl Serialize for StudioStudyId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for StudioStudyId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Cow::<str>::deserialize(deserializer)?
            .parse()
            .map_err(serde::de::Error::custom)
    }
}

impl JsonSchema for StudioStudyId {
    fn schema_name() -> Cow<'static, str> {
        "StudioStudyId".into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "description": "A Studio study ID: `study-` and a number, such as `study-7`.",
            "type": "string",
            "pattern": "^study-(0|[1-9][0-9]*)$"
        })
    }
}

/// Lifecycle events emitted by a Studio study. `sequence` orders transport
/// events; it is not economic time and has no effect on replay.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum StudioEvent {
    StudyStarted {
        study_id: StudioStudyId,
        sequence: u64,
        problem: ProblemPath,
        strategy: StrategyPath,
    },
    Progress {
        study_id: StudioStudyId,
        sequence: u64,
        /// The phase of work the progress is in, such as `monte-carlo`.
        phase: PhasePath,
        completed: u64,
        #[serde(skip_serializing_if = "Option::is_none")]
        total: Option<u64>,
        message: String,
    },
    /// One step of the thinking behind a study: what its search considered
    /// before choosing.
    Thinking {
        study_id: StudioStudyId,
        sequence: u64,
        observation: SearchObservationView,
    },
    FrameAppended {
        study_id: StudioStudyId,
        sequence: u64,
        document: DocumentPath,
        frame_index: u64,
        frame: Box<ExchangeFrame>,
    },
    /// The study can be fetched.
    StudyPublished {
        study_id: StudioStudyId,
        sequence: u64,
        key: StudyKey,
        documents: u64,
    },
    StudyPaused {
        study_id: StudioStudyId,
        sequence: u64,
    },
    StudyResumed {
        study_id: StudioStudyId,
        sequence: u64,
    },
    StudyCompleted {
        study_id: StudioStudyId,
        sequence: u64,
        /// The study's selected document.
        document: DocumentPath,
    },
    StudyCancelled {
        study_id: StudioStudyId,
        sequence: u64,
    },
    StudyFailed {
        study_id: StudioStudyId,
        sequence: u64,
        message: String,
    },
}

impl StudioEvent {
    pub const fn sequence(&self) -> u64 {
        match self {
            Self::StudyStarted { sequence, .. }
            | Self::Progress { sequence, .. }
            | Self::Thinking { sequence, .. }
            | Self::FrameAppended { sequence, .. }
            | Self::StudyPublished { sequence, .. }
            | Self::StudyPaused { sequence, .. }
            | Self::StudyResumed { sequence, .. }
            | Self::StudyCompleted { sequence, .. }
            | Self::StudyCancelled { sequence, .. }
            | Self::StudyFailed { sequence, .. } => *sequence,
        }
    }

    pub const fn study_id(&self) -> StudioStudyId {
        match self {
            Self::StudyStarted { study_id, .. }
            | Self::Progress { study_id, .. }
            | Self::Thinking { study_id, .. }
            | Self::FrameAppended { study_id, .. }
            | Self::StudyPublished { study_id, .. }
            | Self::StudyPaused { study_id, .. }
            | Self::StudyResumed { study_id, .. }
            | Self::StudyCompleted { study_id, .. }
            | Self::StudyCancelled { study_id, .. }
            | Self::StudyFailed { study_id, .. } => *study_id,
        }
    }
}

#[derive(Debug, Default)]
pub struct StudyControl {
    cancelled: AtomicBool,
    paused: AtomicBool,
}

impl StudyControl {
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
    /// or cancels the study. Search algorithms with finer-grained sessions can
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
    #[error("study was cancelled")]
    Cancelled,
    #[error("study is paused")]
    Paused,
    #[error("problem `{problem}` could not be studied: {message}")]
    Problem {
        problem: ProblemPath,
        message: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use axionomy::{
        Account, Economy, EconomyBuilder, Exchange, Goal, Identifier, Ontology, Quantity, Rate,
        Trace, basket, path, paths,
    };
    use axionomy_view::{Scene, SearchObservationKindView, TelemetrySubjectView};
    use std::sync::{Arc, mpsc};

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    enum Bakery {}
    #[derive(Debug, Clone, PartialEq, Eq, Hash, Identifier)]
    enum Holder {
        Baker,
    }
    #[derive(Debug, Clone, PartialEq, Eq, Hash, Identifier)]
    enum Good {
        Flour,
        Bread,
    }
    #[derive(Debug, Clone, PartialEq, Eq, Hash, Identifier)]
    enum Law {
        Bake,
    }
    #[derive(Debug, Clone, PartialEq, Eq, Hash, Identifier)]
    enum Part {
        Oven,
    }

    impl Ontology for Bakery {
        const NAME: &'static str = "bakery";
        type Account = Holder;
        type Asset = Good;
        type Rate = Law;
        type Role = Part;
    }

    axionomy_view::debug_labels!(Holder, Good, Law, Part);
    axionomy_view::counted!(Good);

    /// A problem small enough to read whole: bake loaves from flour after
    /// `budget` batches of reported work. Its strategies are how many loaves
    /// to bake; `none` names no document, which a study must refuse.
    struct Baking {
        descriptor: ProblemDescriptor,
    }

    impl Default for Baking {
        fn default() -> Self {
            let strategy = |key: StrategyPath| StrategyDescriptor {
                key,
                label: "Bake".into(),
                description: "Bake loaves".into(),
                algorithm: "baking".into(),
            };
            Self {
                descriptor: ProblemDescriptor {
                    key: path!("baking"),
                    title: "Baking".into(),
                    summary: "Bake bread from flour.".into(),
                    family: ProblemFamily::Production,
                    default_instance: InstanceProfile::Micro,
                    instances: vec![InstanceDescriptor {
                        profile: InstanceProfile::Micro,
                        label: "Micro".into(),
                        description: "Two units of flour".into(),
                    }],
                    default_strategy: path!("one"),
                    strategies: vec![
                        strategy(path!("one")),
                        strategy(path!("two")),
                        strategy(path!("none")),
                    ],
                    capabilities: Vec::new(),
                },
            }
        }
    }

    fn no_scene(_: u64, _: &Economy<Bakery>) -> Option<Scene> {
        None
    }

    impl Problem for Baking {
        fn descriptor(&self) -> &ProblemDescriptor {
            &self.descriptor
        }

        fn study(
            &self,
            key: &StudyKey,
            progress: &mut StudyProgress<'_>,
        ) -> Result<Vec<ViewDocument>, ServiceError> {
            let work = Work::new(
                path!("baking"),
                path!("baking"),
                SearchObservationKindView::Rollout,
            );
            for batch in 1..=key.budget {
                let _ = progress.emit(&work, batch, key.budget, format!("Baked batch {batch}"));
                progress.checkpoint()?;
            }
            let initial = EconomyBuilder::<Bakery>::new()
                .account(Holder::Baker, Account::from(basket([(Good::Flour, 2)])))
                .rate(
                    Law::Bake,
                    Rate::new()
                        .consume(Part::Oven, basket([(Good::Flour, 1)]))
                        .produce(Part::Oven, basket([(Good::Bread, 1)])),
                )
                .build()
                .map_err(|error| problem_error(&key.problem, error))?;
            [
                (path!("one"), "One loaf", 1),
                (path!("two"), "Two loaves", 2),
            ]
            .into_iter()
            .map(|(id, title, loaves)| {
                let mut trace = Trace::new();
                for _ in 0..loaves {
                    trace.push(
                        Exchange::new(Law::Bake, Quantity::new(1)).bind(Part::Oven, Holder::Baker),
                    );
                }
                document(
                    progress,
                    DocumentSpec {
                        problem: &key.problem,
                        document: id,
                        title,
                        description: "Loaves baked one at a time",
                        source_label: "Bakery",
                    },
                    &initial,
                    &Goal::new(),
                    &trace,
                    Vec::new(),
                    no_scene,
                )
                .map_err(|error| problem_error(&key.problem, error))
            })
            .collect()
        }
    }

    fn bakery() -> Registry {
        let mut registry = Registry::new();
        registry.register(Baking::default()).expect("one problem");
        registry
    }

    fn baking(strategy: StrategyPath, budget: u64) -> StudyRequest {
        let mut request = StudyRequest::new(path!("baking")).with_strategy(strategy);
        request.budget = budget;
        request
    }

    #[test]
    fn a_registry_holds_one_problem_under_each_key() {
        let mut registry = bakery();
        assert_eq!(
            registry.register(Baking::default()).err(),
            Some(DuplicateProblem(path!("baking")))
        );
        assert_eq!(registry.catalog().len(), 1);
        assert_eq!(
            registry
                .problem(&path!("baking"))
                .map(|problem| problem.title.as_str()),
            Some("Baking")
        );
    }

    #[test]
    fn a_request_resolves_to_what_its_problem_offers() {
        let registry = bakery();
        let key = registry
            .resolve(&StudyRequest::new(path!("baking")))
            .unwrap();
        assert_eq!(
            (key.instance, key.strategy),
            (InstanceProfile::Micro, path!("one"))
        );
        assert_eq!(
            registry.resolve(&StudyRequest::new(path!("bread"))),
            Err(ServiceError::UnknownProblem(path!("bread")))
        );
        assert!(matches!(
            registry.resolve(&StudyRequest::new(path!("baking")).with_strategy(path!("three"))),
            Err(ServiceError::UnknownStrategy { .. })
        ));
        assert!(matches!(
            registry.resolve(
                &StudyRequest::new(path!("baking")).with_instance(InstanceProfile::Stress)
            ),
            Err(ServiceError::UnknownInstance { .. })
        ));
    }

    /// A study holds its problem's documents, checked and assembled: the
    /// selected one streamed frame by frame, each with its model's size and
    /// the thinking reported while it was studied, and each document's
    /// trajectory reported as the paths its frames replay.
    #[test]
    fn a_study_assembles_the_documents_its_problem_derives() {
        #[derive(Default)]
        struct Observed {
            frames: Vec<DocumentPath>,
            trajectories: Vec<(DocumentPath, paths::Economy, paths::Trace)>,
        }

        impl StudyObserver for Observed {
            fn progress(&mut self, _: ServiceProgress) {}

            fn frame(&mut self, document: &DocumentPath, _: ExchangeFrame) {
                self.frames.push(document.clone());
            }

            fn trajectory(
                &mut self,
                document: &DocumentPath,
                initial: &paths::Economy,
                trace: &paths::Trace,
            ) {
                self.trajectories
                    .push((document.clone(), initial.clone(), trace.clone()));
            }
        }

        let mut observed = Observed::default();
        let study = bakery()
            .study_with(
                &baking(path!("two"), 3),
                &StudyControl::default(),
                &mut observed,
            )
            .unwrap();
        let selected = study.selected().unwrap();
        assert_eq!(selected.id, path!("two"));
        assert_eq!(selected.frames.len(), 2);
        assert_eq!(observed.frames, [path!("two"), path!("two")]);
        assert_eq!(
            observed
                .trajectories
                .iter()
                .map(|(document, _, trace)| (document.clone(), trace.exchanges().len()))
                .collect::<Vec<_>>(),
            [(path!("one"), 1), (path!("two"), 2)]
        );
        let (_, initial, trace) = &observed.trajectories[1];
        let baked = initial.replayed(trace).unwrap();
        assert_eq!(
            baked.balance(&path!("baker"), &path!("bread")),
            Quantity::new(2)
        );
        assert!(study.documents.iter().all(|document| {
            document
                .telemetry
                .iter()
                .any(|series| series.subject == TelemetrySubjectView::Model)
                && document
                    .thinking
                    .iter()
                    .any(|observation| observation.label == "Baked batch 3")
        }));
    }

    #[test]
    fn a_study_refuses_a_strategy_without_its_document() {
        assert!(matches!(
            bakery().study(baking(path!("none"), 1)),
            Err(ServiceError::Problem { message, .. }) if message.contains("produced no document")
        ));
    }

    #[test]
    fn a_paused_study_waits_and_resumes_without_restarting_its_command() {
        let control = Arc::new(StudyControl::default());
        control.pause();
        let worker_control = Arc::clone(&control);
        let worker = std::thread::spawn(move || {
            bakery().study_with(&baking(path!("one"), 2), &worker_control, &mut |_| {})
        });
        std::thread::sleep(std::time::Duration::from_millis(25));
        assert!(!worker.is_finished());
        control.resume();
        assert!(worker.join().unwrap().is_ok());
    }

    #[test]
    fn a_cancelled_study_stops_at_its_next_report() {
        let control = StudyControl::default();
        let mut batches = Vec::new();
        let result = bakery().study_with(
            &baking(path!("one"), 8),
            &control,
            &mut |progress: ServiceProgress| {
                if progress.phase == path!("baking") {
                    if progress.completed == 3 {
                        control.cancel();
                    }
                    batches.push(progress.completed);
                }
            },
        );
        assert_eq!(result, Err(ServiceError::Cancelled));
        assert_eq!(batches, [1, 2, 3]);
    }

    #[test]
    fn a_pause_mid_study_blocks_the_next_batch_until_resume() {
        let control = Arc::new(StudyControl::default());
        let resume_control = Arc::clone(&control);
        let (paused_tx, paused_rx) = mpsc::sync_channel(1);
        let resumer = std::thread::spawn(move || {
            paused_rx.recv().unwrap();
            std::thread::sleep(std::time::Duration::from_millis(30));
            assert!(resume_control.is_paused());
            resume_control.resume();
        });
        let mut paused_once = false;
        let result = bakery().study_with(
            &baking(path!("one"), 4),
            &control,
            &mut |progress: ServiceProgress| {
                if !paused_once && progress.phase == path!("baking") {
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

    #[test]
    fn studio_events_are_discriminated_unions_of_minted_studies() {
        let event = StudioEvent::Progress {
            study_id: StudioStudyId::new(1),
            sequence: 2,
            phase: path!("documents"),
            completed: 3,
            total: Some(5),
            message: "replaying".into(),
        };
        let value = serde_json::to_value(&event).unwrap();
        assert_eq!(value["kind"], "progress");
        assert_eq!(value["study_id"], "study-1");
        let decoded: StudioEvent = serde_json::from_value(value).unwrap();
        assert_eq!(decoded, event);
        assert_eq!(decoded.study_id(), StudioStudyId::new(1));
        for forged in [
            "study-",
            "study-01",
            "study--1",
            "Study-1",
            "study-1x",
            "run-1",
            "browser-1",
        ] {
            assert!(forged.parse::<StudioStudyId>().is_err(), "{forged}");
        }
    }
}
