use crate::{
    Capability, InstanceDescriptor, InstanceProfile, ProblemDescriptor, ProblemFamily,
    ReferenceService, RunArtifact, RunControl, RunObserver, RunRequest, ServiceError,
    ServiceProgress, StrategyDescriptor,
};
use axionomy::{Economy, Goal, QuantityScalar, Trace};
use axionomy_view::{
    DebugOntology, ObjectiveView, PlaybackError, ProposalView, Scene, SceneAnchorView,
    SceneEntityView, SceneGlyphView, SceneMetricView, SceneToneView, SearchObservationKindView,
    SearchObservationView, SearchTelemetryView, TelemetryKindView, TelemetryPointView,
    ViewDocument, ViewDocumentMetadata, ViewId, ViewOntology, ViewSource, derive_document,
    derive_model, derive_proposal,
};
use std::{fmt::Debug, hash::Hash, ops::ControlFlow};

pub(super) fn visual_entity(
    key: impl Into<String>,
    label: impl Into<String>,
    glyph: SceneGlyphView,
    anchor: SceneAnchorView,
    tone: SceneToneView,
    status: Option<String>,
) -> SceneEntityView {
    SceneEntityView {
        id: ViewId::new(key, label),
        glyph,
        anchor,
        tone,
        status,
        account: None,
        metrics: Vec::new(),
    }
}

pub(super) fn visual_metric(
    key: impl Into<String>,
    label: impl Into<String>,
    value: impl ToString,
    unit: Option<&str>,
) -> SceneMetricView {
    SceneMetricView {
        key: key.into(),
        label: label.into(),
        value: value.to_string(),
        unit: unit.map(str::to_owned),
        previous: None,
    }
}

mod bridge;
mod connect_four;
mod exact_cover;
mod labels;
mod logistics;
mod marketplace;
mod maze;
mod mission;
mod perishables;
mod rescue;
mod scheduling;
mod sokoban;
mod workshop;

use labels::StudioLabel;

type StudioScene<AccountId, A, RateId, Role, N> =
    fn(u64, &Economy<AccountId, A, RateId, Role, N>) -> Option<Scene>;

struct StudioOntology<AccountId, A, RateId, Role, N = u64> {
    fallback: DebugOntology<AccountId, A, RateId, Role, N>,
    scene: Option<StudioScene<AccountId, A, RateId, Role, N>>,
}

impl<AccountId, A, RateId, Role, N> StudioOntology<AccountId, A, RateId, Role, N> {
    fn new(
        namespace: impl Into<String>,
        scene: Option<StudioScene<AccountId, A, RateId, Role, N>>,
    ) -> Self {
        Self {
            fallback: DebugOntology::new(namespace),
            scene,
        }
    }

    fn relabel<T: StudioLabel>(mut id: ViewId, value: &T) -> ViewId {
        id.label = value.studio_label();
        id
    }
}

impl<AccountId, A, RateId, Role, N> ViewOntology<AccountId, A, RateId, Role, N>
    for StudioOntology<AccountId, A, RateId, Role, N>
where
    AccountId: Debug + StudioLabel,
    A: Debug + StudioLabel,
    RateId: Debug + StudioLabel,
    Role: Debug + StudioLabel,
{
    fn account(&self, id: &AccountId) -> ViewId {
        Self::relabel(self.fallback.account(id), id)
    }

    fn asset(&self, id: &A) -> ViewId {
        Self::relabel(self.fallback.asset(id), id)
    }

    fn rate(&self, id: &RateId) -> ViewId {
        Self::relabel(self.fallback.rate(id), id)
    }

    fn role(&self, id: &Role) -> ViewId {
        Self::relabel(self.fallback.role(id), id)
    }

    fn scene(&self, index: u64, economy: &Economy<AccountId, A, RateId, Role, N>) -> Option<Scene> {
        self.scene.and_then(|scene| scene(index, economy))
    }
}

pub(crate) fn catalog() -> Vec<ProblemDescriptor> {
    vec![
        problem(
            "maze",
            "Key-door maze",
            "Reach the exit on limited energy and time. The key route costs less energy, the detour takes fewer moves, and no single weighting picks between them.",
            [
                "The smallest maze with a key, a locked door, and a real route choice",
                "14 rooms, 16 one-way passages, and four competing route families",
                "22 rooms, 29 passages, five route families, and cross-route choices",
            ],
            ProblemFamily::Pathfinding,
            "a_star",
            &[
                (
                    "breadth_first",
                    "Fewest moves",
                    "Breadth-first search: the shortest sequence, whatever it costs.",
                    "breadth-first search",
                ),
                (
                    "a_star",
                    "Least energy",
                    "A*, guided by a distance estimate stored in the economy itself.",
                    "A*",
                ),
                (
                    "pareto_energy",
                    "Pareto: least energy",
                    "The lowest-energy point on the exact Pareto frontier, where no route is better on both energy and time.",
                    "exact Pareto search",
                ),
                (
                    "pareto_time",
                    "Pareto: least time",
                    "The fastest point on that exact frontier.",
                    "exact Pareto search",
                ),
            ],
            &[
                Capability::DeterministicSearch,
                Capability::WeightedSearch,
                Capability::ExactPareto,
                Capability::FeasibilityAssessment,
            ],
        ),
        problem(
            "sokoban",
            "Sokoban",
            "Push crates onto their goal squares. Every push rewrites three cells at once, and some positions can never be recovered.",
            [
                "Five cells and two pushes",
                "A 7×5 board with 35 cells and a ten-step solution",
                "An 8×6 board with a longer route and a larger cell economy",
            ],
            ProblemFamily::Pathfinding,
            "breadth_first",
            &[(
                "breadth_first",
                "Solve puzzle",
                "Breadth-first search over single moves and crate pushes.",
                "breadth-first search",
            )],
            &[
                Capability::DeterministicSearch,
                Capability::MultiAccountExchange,
                Capability::FeasibilityAssessment,
            ],
        ),
        problem(
            "exact_cover",
            "Exact cover",
            "Choose subsets that cover every element exactly once. A plain search and Algorithm X reach the same answer by different routes.",
            [
                "Four elements and four competing subsets",
                "Eight elements and twelve subsets arranged as two interacting blocks",
                "Twelve elements and eighteen subsets with 27 competing exact covers",
            ],
            ProblemFamily::Constraint,
            "algorithm_x",
            &[
                (
                    "breadth_first",
                    "Plain search",
                    "Breadth-first search with no knowledge of cover structure; it just tries selections.",
                    "breadth-first search",
                ),
                (
                    "algorithm_x",
                    "Algorithm X",
                    "Knuth's Algorithm X chooses the moves; the economy still validates every one.",
                    "Algorithm X",
                ),
            ],
            &[
                Capability::DeterministicSearch,
                Capability::SpecializedAlgorithm,
                Capability::FeasibilityAssessment,
            ],
        ),
        problem(
            "workshop",
            "Workshop",
            "Build the order from raw stock without creating or destroying material. The fastest plan and the least wasteful plan are not the same plan.",
            [
                "One chair made from raw stock",
                "A six-chair order with competing fast and low-waste recipes",
                "A ten-chair order with more material, labor, and production choices",
            ],
            ProblemFamily::Production,
            "minimum_waste",
            &[
                (
                    "breadth_first",
                    "Fewest steps",
                    "Breadth-first search: the fewest recipe firings, whatever they waste.",
                    "breadth-first search",
                ),
                (
                    "minimum_waste",
                    "Least waste",
                    "Best-first search guided by scrap accumulated in the workshop account.",
                    "best-first search",
                ),
                (
                    "pareto_waste",
                    "Pareto: least waste",
                    "The lowest-scrap point on the exact Pareto frontier.",
                    "exact Pareto search",
                ),
                (
                    "pareto_time",
                    "Pareto: least time",
                    "The fastest point on that exact frontier.",
                    "exact Pareto search",
                ),
            ],
            &[
                Capability::DeterministicSearch,
                Capability::WeightedSearch,
                Capability::ExactPareto,
                Capability::FeasibilityAssessment,
            ],
        ),
        problem(
            "scheduling",
            "Job-shop scheduling",
            "Fit every operation onto a machine without double-booking one or running a job out of order. Finishing one job early delays the other.",
            [
                "Two jobs on the smallest horizon that still forces a machine conflict",
                "Six ordered operations across three machines and 18 time slots",
                "The same operations across a longer eight-slot decision horizon",
            ],
            ProblemFamily::Scheduling,
            "bounded_optimizer",
            &[
                (
                    "best_first",
                    "Shortest makespan",
                    "Best-first search on the finish time recorded in the schedule.",
                    "best-first search",
                ),
                (
                    "bounded_optimizer",
                    "Branch optimizer",
                    "Depth-first branch-and-bound abandons schedules it can already prove are worse.",
                    "branch-and-bound",
                ),
                (
                    "pareto_job_one",
                    "Pareto: Job One first",
                    "The frontier schedule that finishes Job One earliest.",
                    "exact Pareto search",
                ),
                (
                    "pareto_job_two",
                    "Pareto: Job Two first",
                    "The frontier schedule that finishes Job Two earliest.",
                    "exact Pareto search",
                ),
            ],
            &[
                Capability::WeightedSearch,
                Capability::BranchOptimization,
                Capability::ExactPareto,
                Capability::FeasibilityAssessment,
            ],
        ),
        problem(
            "rescue",
            "Uncertain rescue",
            "The survivor's location is unknown. Spend the sensor first, or commit immediately and risk searching the wrong site.",
            [
                "Two possible sites and eight hidden location/sensor outcomes",
                "Four possible sites, 32 outcomes, an unreliable sensor, and an evacuation leg",
                "Six possible sites, 72 outcomes, and at least 256 policy samples",
            ],
            ProblemFamily::PartialObservation,
            "observe_then_follow",
            &[
                (
                    "observe_then_follow",
                    "Sense, then move",
                    "Spend the sensor, read what Nature reports, then head for the reported site.",
                    "Monte Carlo policy evaluation",
                ),
                (
                    "direct_north",
                    "Go north immediately",
                    "Skip the sensor and commit; the drawn scenario decides whether that pays off.",
                    "Monte Carlo policy evaluation",
                ),
            ],
            &[
                Capability::PartialObservation,
                Capability::Chance,
                Capability::MonteCarlo,
                Capability::ApproximatePareto,
            ],
        ),
        problem(
            "bridge",
            "Bridge allocation",
            "One lane, two agents, both need it. Compare first-come priority against an auction that settles bids and access in indivisible steps.",
            [
                "One crossing round for two agents and one lane",
                "Two allocation rounds with escrow, recharge, and retained credit",
                "Three bounded rounds where priority and spending affect later choices",
            ],
            ProblemFamily::Allocation,
            "auction",
            &[
                (
                    "breadth_first",
                    "Plain search",
                    "Breadth-first search for any valid crossing order.",
                    "breadth-first search",
                ),
                (
                    "first_come_a",
                    "First come: A",
                    "Agent A gets the first crossing right by arrival order, not by bid.",
                    "first-come allocation",
                ),
                (
                    "first_come_b",
                    "First come: B",
                    "Agent B gets the first crossing right by arrival order, not by bid.",
                    "first-come allocation",
                ),
                (
                    "auction",
                    "Auction",
                    "Bids, winner, escrow, refunds, and crossing rights settle through indivisible changes.",
                    "auction mechanism",
                ),
                (
                    "pareto_a",
                    "Pareto: favors A",
                    "The point on the exact frontier that is best for Agent A.",
                    "exact Pareto search",
                ),
                (
                    "pareto_b",
                    "Pareto: favors B",
                    "The point on the exact frontier that is best for Agent B.",
                    "exact Pareto search",
                ),
            ],
            &[
                Capability::DeterministicSearch,
                Capability::AtomicSettlement,
                Capability::ExactPareto,
                Capability::MultiAccountExchange,
            ],
        ),
        problem(
            "marketplace",
            "Multi-party marketplace",
            "Settle orders across buyers, sellers, carriers, the platform, and tax in one indivisible step. When it fails, you see exactly which account fell short and by how much.",
            [
                "Two orders across three buyers, three sellers, and one active carrier",
                "Four linked orders across 14 accounts sharing budget, stock, shipping capacity, and tax",
                "Six linked orders across 16 accounts with exactly enough stock and shipping capacity",
            ],
            ProblemFamily::Market,
            "market_clearing",
            &[
                (
                    "market_clearing",
                    "Clear the market",
                    "Pick orders that can all settle together without overdrawing any account.",
                    "market clearing",
                ),
                (
                    "pareto_buyers",
                    "Pareto: favors buyers",
                    "The clearing on the exact frontier with the highest total buyer benefit.",
                    "exact Pareto clearing",
                ),
                (
                    "pareto_sellers",
                    "Pareto: favors sellers",
                    "The clearing on the exact frontier with the highest total seller benefit.",
                    "exact Pareto clearing",
                ),
            ],
            &[
                Capability::AtomicSettlement,
                Capability::MultiAccountExchange,
                Capability::FeasibilityAssessment,
                Capability::ExactPareto,
            ],
        ),
        problem(
            "logistics",
            "Stochastic logistics",
            "Deliver four orders through weather and breakdowns. The short route wins when nothing goes wrong; the long one finishes more often when things do.",
            [
                "Two deliveries with a compact route and a small sample budget",
                "Four deliveries with recurring weather and breakdowns",
                "The four-delivery network evaluated with at least 256 stochastic rollouts",
            ],
            ProblemFamily::StochasticPlanning,
            "reliable",
            &[
                (
                    "direct",
                    "Direct route",
                    "The shortest route, fastest when weather and breakdowns cooperate.",
                    "Monte Carlo policy evaluation",
                ),
                (
                    "reliable",
                    "Reliable route",
                    "A longer route that completes more often when they do not.",
                    "Monte Carlo policy evaluation",
                ),
                (
                    "mcts",
                    "MCTS",
                    "Monte Carlo tree search plans the next move by sampling how chance could unfold.",
                    "Monte Carlo tree search",
                ),
            ],
            &[
                Capability::Chance,
                Capability::MonteCarlo,
                Capability::Mcts,
                Capability::ApproximatePareto,
            ],
        ),
        problem(
            "connect_four",
            "Connect Four",
            "A full game of Connect Four played by MCTS on both sides, with gravity and every winning line written as rules.",
            [
                "A compact board with a short game and few rollouts",
                "The full 7×6 board and all 69 winning lines",
                "The full board with four times the MCTS work per move",
            ],
            ProblemFamily::AdversarialGame,
            "mcts_game",
            &[(
                "mcts_game",
                "MCTS self-play",
                "Monte Carlo tree search plays both colours until someone wins or the board fills.",
                "Monte Carlo tree search",
            )],
            &[Capability::Mcts, Capability::MultiAccountExchange],
        ),
        problem(
            "mission",
            "Hidden-information mission",
            "Two agents each see only part of the map. Compare sharing what they see before acting against both moving straight in.",
            [
                "Two hidden scenarios and one private sighting",
                "Sixteen hidden scenarios with private sightings, belief updates, and hazards",
                "The same mission evaluated with at least 256 belief-conditioned simulations",
            ],
            ProblemFamily::PartialObservation,
            "coordinated",
            &[
                (
                    "coordinated",
                    "Scout, share, then move",
                    "One agent looks, tells the other, and both act on the updated picture.",
                    "information-set policy evaluation",
                ),
                (
                    "direct_north",
                    "Both go north",
                    "Neither agent looks or shares; replay keeps whatever goes wrong.",
                    "Monte Carlo policy evaluation",
                ),
            ],
            &[
                Capability::InformationSetSearch,
                Capability::PartialObservation,
                Capability::Chance,
                Capability::MonteCarlo,
                Capability::ApproximatePareto,
                Capability::RlProjection,
            ],
        ),
        problem(
            "perishables",
            "Perishable inventory",
            "Ten thousand units spoil by batch, not one at a time. Cooling costs energy, and a power outage forces the tradeoff.",
            [
                "One hundred units split between two batches",
                "Ten thousand units with cooling, deadlines, and a power outage",
                "One million units using the same two batch-level condition facts",
            ],
            ProblemFamily::TemporalSimulation,
            "outage",
            &[
                (
                    "outage",
                    "Power outage",
                    "Refrigeration fails, and every batch past its deadline spoils at once.",
                    "indexed temporal effects",
                ),
                (
                    "pareto_inventory",
                    "Pareto: save stock",
                    "The storage plan on the exact frontier that keeps the most usable units.",
                    "exact Pareto search",
                ),
                (
                    "pareto_energy",
                    "Pareto: save energy",
                    "The storage plan that spends the least cooling energy.",
                    "exact Pareto search",
                ),
            ],
            &[
                Capability::TemporalEffects,
                Capability::FungibleCohorts,
                Capability::NonFungibleFacts,
                Capability::ExactPareto,
            ],
        ),
    ]
}

fn problem(
    key: &str,
    title: &str,
    summary: &str,
    instance_descriptions: [&str; 3],
    family: ProblemFamily,
    default_strategy: &str,
    strategies: &[(&str, &str, &str, &str)],
    capabilities: &[Capability],
) -> ProblemDescriptor {
    ProblemDescriptor {
        key: key.into(),
        title: title.into(),
        summary: summary.into(),
        family,
        default_instance: "showcase".into(),
        instances: vec![
            InstanceDescriptor {
                key: "micro".into(),
                label: "Micro".into(),
                description: instance_descriptions[0].into(),
                profile: InstanceProfile::Micro,
            },
            InstanceDescriptor {
                key: "showcase".into(),
                label: "Showcase".into(),
                description: instance_descriptions[1].into(),
                profile: InstanceProfile::Showcase,
            },
            InstanceDescriptor {
                key: "stress".into(),
                label: "Stress".into(),
                description: instance_descriptions[2].into(),
                profile: InstanceProfile::Stress,
            },
        ],
        default_strategy: default_strategy.into(),
        strategies: strategies
            .iter()
            .map(|(key, label, description, algorithm)| StrategyDescriptor {
                key: (*key).into(),
                label: (*label).into(),
                description: (*description).into(),
                algorithm: (*algorithm).into(),
            })
            .collect(),
        capabilities: capabilities.to_vec(),
    }
}

pub(crate) fn run(
    request: &RunRequest,
    control: &RunControl,
    observer: &mut dyn RunObserver,
) -> Result<RunArtifact, ServiceError> {
    let descriptor = ReferenceService
        .problem(&request.problem)
        .ok_or_else(|| ServiceError::UnknownProblem(request.problem.clone()))?;
    let strategy = request
        .strategy
        .as_deref()
        .unwrap_or(&descriptor.default_strategy);
    if !descriptor
        .strategies
        .iter()
        .any(|item| item.key == strategy)
    {
        return Err(ServiceError::UnknownStrategy {
            problem: descriptor.key,
            strategy: strategy.into(),
        });
    }
    let instance_key = request
        .instance
        .as_deref()
        .unwrap_or(&descriptor.default_instance);
    if !descriptor
        .instances
        .iter()
        .any(|item| item.key == instance_key)
    {
        return Err(ServiceError::UnknownInstance {
            problem: descriptor.key,
            instance: instance_key.into(),
        });
    }
    let mut progress = ProgressSink::new(control, observer);
    let _ = progress.emit(
        "prepare",
        0,
        1,
        format!("preparing {} · {instance_key}", request.problem),
    );
    progress.ensure()?;
    let mut artifact = match request.problem.as_str() {
        "maze" => maze::build(request, &descriptor),
        "sokoban" => sokoban::build(request, &descriptor),
        "exact_cover" => exact_cover::build(request, &descriptor),
        "workshop" => workshop::build(request, &descriptor),
        "scheduling" => scheduling::build(request, &descriptor),
        "rescue" => rescue::build(request, &descriptor),
        "bridge" => bridge::build(request, &descriptor),
        "marketplace" => marketplace::build(request, &descriptor),
        "logistics" => logistics::build(request, &descriptor, &mut progress),
        "connect_four" => connect_four::build(request, &descriptor, &mut progress),
        "mission" => mission::build(request, &descriptor, &mut progress),
        "perishables" => perishables::build(request, &descriptor),
        _ => unreachable!("catalog and dispatch must agree"),
    }?;
    progress.ensure()?;
    for (offset, document) in artifact.documents.iter().enumerate() {
        let _ = progress.emit(
            "artifact",
            offset as u64 + 1,
            artifact.documents.len() as u64,
            format!("derived {}", document.title),
        );
        progress.ensure()?;
    }
    artifact.assessed_proposals = artifact
        .documents
        .iter()
        .flat_map(|document| document.proposals.iter().cloned())
        .collect();
    let solve_observations = progress.observations().to_vec();
    for document in &mut artifact.documents {
        document.solve_observations = solve_observations.clone();
        for frame in &mut document.frames {
            if frame.observations.is_empty() {
                frame.observations = document.observations.clone();
            }
        }
    }
    Ok(artifact)
}

pub(super) struct ProgressSink<'a> {
    control: &'a RunControl,
    observer: &'a mut dyn RunObserver,
    sequence: u64,
    error: Option<ServiceError>,
    observations: Vec<SearchObservationView>,
}

impl<'a> ProgressSink<'a> {
    fn new(control: &'a RunControl, observer: &'a mut dyn RunObserver) -> Self {
        Self {
            control,
            observer,
            sequence: 0,
            error: None,
            observations: Vec::new(),
        }
    }

    pub fn emit(
        &mut self,
        phase: impl Into<String>,
        completed: u64,
        total: u64,
        message: impl Into<String>,
    ) -> ControlFlow<()> {
        if self.error.is_some() {
            return ControlFlow::Break(());
        }
        if let Err(error) = self.control.checkpoint() {
            self.error = Some(error);
            return ControlFlow::Break(());
        }
        let phase = phase.into();
        let message = message.into();
        self.observer.progress(ServiceProgress {
            sequence: self.sequence,
            phase: phase.clone(),
            completed,
            total,
            message: message.clone(),
        });
        let observation = SearchObservationView {
            sequence: self.sequence,
            algorithm: phase.clone(),
            kind: observation_kind(&phase),
            phase,
            label: message,
            completed,
            total,
            metrics: vec![
                visual_metric("completed", "Work completed", completed, None),
                visual_metric(
                    "remaining",
                    "Work remaining",
                    total.saturating_sub(completed),
                    None,
                ),
            ],
        };
        self.observer.observation(observation.clone());
        if self.observations.len() == 256 {
            self.observations.remove(0);
        }
        self.observations.push(observation);
        self.sequence += 1;
        ControlFlow::Continue(())
    }

    pub fn ensure(&self) -> Result<(), ServiceError> {
        match &self.error {
            Some(error) => Err(error.clone()),
            None => Ok(()),
        }
    }

    pub fn observations(&self) -> &[SearchObservationView] {
        &self.observations
    }
}

fn observation_kind(phase: &str) -> SearchObservationKindView {
    if phase.contains("pareto") || phase.contains("frontier") {
        SearchObservationKindView::Frontier
    } else if phase.contains("monte") || phase.contains("rollout") {
        SearchObservationKindView::Rollout
    } else if phase.contains("mcts") || phase.contains("tree") {
        SearchObservationKindView::Tree
    } else if phase == "artifact" {
        SearchObservationKindView::Artifact
    } else {
        SearchObservationKindView::Phase
    }
}

pub(super) struct DocumentSpec<'a> {
    pub problem: &'a str,
    pub strategy: &'a str,
    pub title: &'a str,
    pub description: &'a str,
    pub source_label: &'a str,
}

pub(super) fn document<AccountId, A, RateId, Role, N>(
    spec: DocumentSpec<'_>,
    initial: &Economy<AccountId, A, RateId, Role, N>,
    goal: &Goal<AccountId, A, N>,
    trace: &Trace<RateId, Role, AccountId, N>,
    objectives: Vec<ObjectiveView>,
    scene: StudioScene<AccountId, A, RateId, Role, N>,
) -> Result<ViewDocument, PlaybackError>
where
    AccountId: Clone + Debug + Eq + Hash + Ord + StudioLabel,
    A: Clone + Debug + Eq + Hash + Ord + StudioLabel,
    RateId: Clone + Debug + Eq + Hash + Ord + StudioLabel,
    Role: Clone + Debug + Ord + StudioLabel,
    N: QuantityScalar,
{
    let ontology = StudioOntology::<AccountId, A, RateId, Role, N>::new(spec.problem, Some(scene));
    let mut document = derive_document(
        ViewDocumentMetadata {
            id: format!("{}:{}", spec.problem, spec.strategy),
            title: spec.title.into(),
            description: spec.description.into(),
            source: ViewSource {
                key: spec.problem.into(),
                label: spec.source_label.into(),
            },
        },
        initial,
        trace,
        &ontology,
        objectives,
    )?;
    document.model = Some(derive_model(initial, goal, &ontology));
    Ok(document)
}

pub(super) struct ProposalSpec<'a> {
    pub id: &'a str,
    pub label: &'a str,
    pub description: &'a str,
}

pub(super) fn proposal<AccountId, A, RateId, Role, N>(
    namespace: &str,
    spec: ProposalSpec<'_>,
    economy: &Economy<AccountId, A, RateId, Role, N>,
    exchange: &axionomy::Exchange<RateId, Role, AccountId, N>,
) -> ProposalView
where
    AccountId: Clone + Debug + Eq + Hash + Ord + StudioLabel,
    A: Clone + Debug + Eq + Hash + Ord + StudioLabel,
    RateId: Clone + Debug + Eq + Hash + Ord + StudioLabel,
    Role: Clone + Debug + Ord + StudioLabel,
    N: QuantityScalar,
{
    let ontology = StudioOntology::<AccountId, A, RateId, Role, N>::new(namespace, None);
    derive_proposal(
        spec.id,
        spec.label,
        spec.description,
        0,
        economy,
        exchange,
        &ontology,
    )
}

pub(super) fn telemetry(
    algorithm: impl Into<String>,
    exact: bool,
    values: impl IntoIterator<Item = (TelemetryKindView, u64, String)>,
) -> SearchTelemetryView {
    SearchTelemetryView {
        algorithm: algorithm.into(),
        exact,
        points: values
            .into_iter()
            .enumerate()
            .map(|(sequence, (kind, value, label))| TelemetryPointView {
                sequence: sequence as u64,
                kind,
                value: value.to_string(),
                label,
            })
            .collect(),
    }
}

pub(super) fn artifact(
    request: &RunRequest,
    descriptor: &ProblemDescriptor,
    selected_strategy: &str,
    mut documents: Vec<ViewDocument>,
) -> Result<RunArtifact, ServiceError> {
    let instance = selected_instance(request, descriptor)
        .expect("service validates instance identity before adapter dispatch")
        .clone();
    let selected_document_id = format!("{}:{}", descriptor.key, selected_strategy);
    let alternatives = documents.len() as u64;
    for document in &mut documents {
        for snapshot in std::iter::once(&document.initial).chain(
            document
                .frames
                .iter()
                .flat_map(|frame| [&frame.before, &frame.after]),
        ) {
            if let Some(scene) = &snapshot.scene {
                scene.validate().map_err(|error| ServiceError::Problem {
                    problem: descriptor.key.clone(),
                    message: format!("invalid scene at snapshot {}: {error}", snapshot.index),
                })?;
            }
        }
        if document.frames.iter().any(|frame| frame.cues.is_empty()) {
            return Err(ServiceError::Problem {
                problem: descriptor.key.clone(),
                message: "accepted transition omitted its explanatory frame cue".into(),
            });
        }
        let accounts = document.initial.accounts.len() as u64;
        let rates = document
            .model
            .as_ref()
            .map_or(0, |model| model.rates.len() as u64);
        let transitions = document.frames.len() as u64;
        let constraints = document.proposals.len() as u64;
        document.telemetry.push(telemetry(
            "Model size",
            true,
            [
                (TelemetryKindView::Accounts, accounts, "accounts".into()),
                (TelemetryKindView::Rates, rates, "rules".into()),
                (
                    TelemetryKindView::Transitions,
                    transitions,
                    "steps in this trace".into(),
                ),
                (
                    TelemetryKindView::Constraints,
                    constraints,
                    "rejection probes".into(),
                ),
                (
                    TelemetryKindView::Alternatives,
                    alternatives,
                    "alternatives compared".into(),
                ),
            ],
        ));
    }
    if !documents
        .iter()
        .any(|document| document.id == selected_document_id)
    {
        return Err(ServiceError::Problem {
            problem: descriptor.key.clone(),
            message: format!("strategy `{selected_strategy}` produced no document"),
        });
    }
    Ok(RunArtifact {
        id: format!(
            "{}:{}:{}:{}:{}",
            descriptor.key, instance.key, selected_strategy, request.seed, request.budget
        ),
        problem: descriptor.clone(),
        instance,
        request: request.clone(),
        selected_document_id,
        documents,
        assessed_proposals: Vec::new(),
    })
}

pub(super) fn selected_instance<'a>(
    request: &RunRequest,
    descriptor: &'a ProblemDescriptor,
) -> Option<&'a InstanceDescriptor> {
    let key = request
        .instance
        .as_deref()
        .unwrap_or(&descriptor.default_instance);
    descriptor
        .instances
        .iter()
        .find(|instance| instance.key == key)
}

pub(super) fn instance_profile(
    request: &RunRequest,
    descriptor: &ProblemDescriptor,
) -> InstanceProfile {
    selected_instance(request, descriptor)
        .expect("service validates instance identity before adapter dispatch")
        .profile
}

pub(super) fn selected_strategy<'a>(
    request: &'a RunRequest,
    descriptor: &'a ProblemDescriptor,
) -> &'a str {
    request
        .strategy
        .as_deref()
        .unwrap_or(&descriptor.default_strategy)
}

pub(super) fn problem_error(problem: &str, error: impl ToString) -> ServiceError {
    ServiceError::Problem {
        problem: problem.into(),
        message: error.to_string(),
    }
}
