use crate::{
    Capability, ProblemDescriptor, ProblemFamily, ReferenceService, RunArtifact, RunControl,
    RunObserver, RunRequest, ServiceError, ServiceProgress, StrategyDescriptor,
};
use axionomy::{Economy, Goal, QuantityScalar, Trace};
use axionomy_view::{
    DebugOntology, ObjectiveView, PlaybackError, ProposalView, Scene, SearchTelemetryView,
    TelemetryKindView, TelemetryPointView, ViewDocument, ViewDocumentMetadata, ViewSource,
    derive_document, derive_model, derive_proposal,
};
use std::{fmt::Debug, hash::Hash};

mod bridge;
mod connect_four;
mod exact_cover;
mod logistics;
mod marketplace;
mod maze;
mod mission;
mod perishables;
mod rescue;
mod scheduling;
mod sokoban;
mod workshop;

pub(crate) fn catalog() -> Vec<ProblemDescriptor> {
    vec![
        problem(
            "maze",
            "Key-door maze",
            "Weighted paths, keys, doors, rejected moves, and an exact energy/time frontier.",
            ProblemFamily::Pathfinding,
            "a_star",
            &[
                (
                    "breadth_first",
                    "Fewest exchanges",
                    "Breadth-first graph search",
                ),
                (
                    "a_star",
                    "Least energy",
                    "A* with an encoded admissible heuristic",
                ),
                (
                    "pareto_energy",
                    "Pareto: least energy",
                    "Select the energy-first exact frontier member",
                ),
                (
                    "pareto_time",
                    "Pareto: least time",
                    "Select the time-first exact frontier member",
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
            "Spatial pushes, multi-account rewrites, deadlocks, and replay-verified puzzle solving.",
            ProblemFamily::Pathfinding,
            "breadth_first",
            &[(
                "breadth_first",
                "Solve puzzle",
                "Breadth-first search over atomic pushes",
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
            "One encoded constraint system proposed by generic search and Algorithm X.",
            ProblemFamily::Constraint,
            "algorithm_x",
            &[
                ("breadth_first", "Generic BFS", "Generic graph search"),
                (
                    "algorithm_x",
                    "Algorithm X",
                    "Traditional Algorithm X proposes core-valid exchanges",
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
            "Stoichiometric production with conserved tools, labor, waste, time, and exact tradeoffs.",
            ProblemFamily::Production,
            "minimum_waste",
            &[
                (
                    "breadth_first",
                    "Fewest recipes",
                    "Breadth-first production search",
                ),
                (
                    "minimum_waste",
                    "Minimum waste",
                    "Best-first production search",
                ),
                (
                    "pareto_waste",
                    "Pareto: least waste",
                    "Waste-first exact frontier member",
                ),
                (
                    "pareto_time",
                    "Pareto: least time",
                    "Time-first exact frontier member",
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
            "Machine capacity, precedence, alternate allocations, and bounded optimization.",
            ProblemFamily::Scheduling,
            "bounded_optimizer",
            &[
                (
                    "best_first",
                    "Best-first",
                    "Generic best-first schedule search",
                ),
                (
                    "bounded_optimizer",
                    "Bounded optimizer",
                    "Caller-owned branch optimizer",
                ),
                (
                    "pareto_job_one",
                    "Pareto: Job One",
                    "Earliest Job One frontier member",
                ),
                (
                    "pareto_job_two",
                    "Pareto: Job Two",
                    "Earliest Job Two frontier member",
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
            "Partially observed rescue policies compared across encoded Nature scenarios.",
            ProblemFamily::PartialObservation,
            "observe_then_follow",
            &[
                (
                    "observe_then_follow",
                    "Observe then follow",
                    "Sense before committing",
                ),
                ("direct_north", "Direct north", "Commit without observing"),
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
            "Capacity allocation through search, first-come policy, auctions, and exact welfare tradeoffs.",
            ProblemFamily::Allocation,
            "auction",
            &[
                ("breadth_first", "Generic BFS", "Generic allocation search"),
                (
                    "first_come_a",
                    "First come: A",
                    "Give Agent A first priority",
                ),
                (
                    "first_come_b",
                    "First come: B",
                    "Give Agent B first priority",
                ),
                (
                    "auction",
                    "Atomic auction",
                    "Resolve bids and allocation atomically",
                ),
                (
                    "pareto_a",
                    "Pareto: Agent A",
                    "Agent A-first frontier member",
                ),
                (
                    "pareto_b",
                    "Pareto: Agent B",
                    "Agent B-first frontier member",
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
            "Buyer, seller, carrier, platform, and tax accounts cleared with explanatory shortfalls.",
            ProblemFamily::Market,
            "market_clearing",
            &[
                (
                    "market_clearing",
                    "Clear market",
                    "Select compatible exact settlements",
                ),
                (
                    "pareto_buyers",
                    "Pareto: buyers",
                    "Buyer-utility frontier member",
                ),
                (
                    "pareto_sellers",
                    "Pareto: sellers",
                    "Seller-utility frontier member",
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
            "Long-horizon routes with weather, breakdowns, risk criteria, Monte Carlo, and MCTS.",
            ProblemFamily::StochasticPlanning,
            "reliable",
            &[
                ("direct", "Direct policy", "Shortest stochastic route"),
                ("reliable", "Reliable policy", "Safer long-horizon route"),
                ("mcts", "MCTS policy", "Plan actions from encoded chance"),
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
            "A complete adversarial game played by vector-valued MCTS over encoded gravity and wins.",
            ProblemFamily::AdversarialGame,
            "mcts_game",
            &[(
                "mcts_game",
                "MCTS game",
                "Play both sides to a terminal state",
            )],
            &[Capability::Mcts, Capability::MultiAccountExchange],
        ),
        problem(
            "mission",
            "Hidden-information mission",
            "Two-agent coordination with actor views, beliefs, information sets, Nature, and RL projections.",
            ProblemFamily::PartialObservation,
            "coordinated",
            &[
                (
                    "coordinated",
                    "Share and coordinate",
                    "Observe, share, and act on updated beliefs",
                ),
                (
                    "direct_north",
                    "Direct north",
                    "Commit both agents without information",
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
            "Fungible cohorts, unique condition facts, time, cooling, power loss, and event-driven decay.",
            ProblemFamily::TemporalSimulation,
            "outage",
            &[
                (
                    "outage",
                    "Power outage",
                    "Replay indexed temporal effects after refrigeration fails",
                ),
                (
                    "pareto_inventory",
                    "Pareto: preserve inventory",
                    "Inventory-first storage frontier member",
                ),
                (
                    "pareto_energy",
                    "Pareto: save energy",
                    "Cooling-energy-first storage frontier member",
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
    family: ProblemFamily,
    default_strategy: &str,
    strategies: &[(&str, &str, &str)],
    capabilities: &[Capability],
) -> ProblemDescriptor {
    ProblemDescriptor {
        key: key.into(),
        title: title.into(),
        summary: summary.into(),
        family,
        default_strategy: default_strategy.into(),
        strategies: strategies
            .iter()
            .map(|(key, label, description)| StrategyDescriptor {
                key: (*key).into(),
                label: (*label).into(),
                description: (*description).into(),
                algorithm: description.to_lowercase(),
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
    if control.is_cancelled() {
        return Err(ServiceError::Cancelled);
    }
    if control.is_paused() {
        return Err(ServiceError::Paused);
    }
    observer.progress(ServiceProgress {
        sequence: 0,
        phase: "prepare".into(),
        completed: 0,
        total: 1,
        message: format!("preparing {}", request.problem),
    });
    let mut artifact = match request.problem.as_str() {
        "maze" => maze::build(request, &descriptor),
        "sokoban" => sokoban::build(request, &descriptor),
        "exact_cover" => exact_cover::build(request, &descriptor),
        "workshop" => workshop::build(request, &descriptor),
        "scheduling" => scheduling::build(request, &descriptor),
        "rescue" => rescue::build(request, &descriptor),
        "bridge" => bridge::build(request, &descriptor),
        "marketplace" => marketplace::build(request, &descriptor),
        "logistics" => logistics::build(request, &descriptor),
        "connect_four" => connect_four::build(request, &descriptor),
        "mission" => mission::build(request, &descriptor),
        "perishables" => perishables::build(request, &descriptor),
        _ => unreachable!("catalog and dispatch must agree"),
    }?;
    if control.is_cancelled() {
        return Err(ServiceError::Cancelled);
    }
    if control.is_paused() {
        return Err(ServiceError::Paused);
    }
    for (offset, document) in artifact.documents.iter().enumerate() {
        observer.progress(ServiceProgress {
            sequence: offset as u64 + 1,
            phase: "artifact".into(),
            completed: offset as u64 + 1,
            total: artifact.documents.len() as u64,
            message: format!("derived {}", document.title),
        });
    }
    artifact.assessed_proposals = artifact
        .documents
        .iter()
        .flat_map(|document| document.proposals.iter().cloned())
        .collect();
    Ok(artifact)
}

pub(super) struct DocumentSpec<'a> {
    pub problem: &'a str,
    pub strategy: &'a str,
    pub title: &'a str,
    pub description: &'a str,
    pub source_label: &'a str,
}

pub(super) fn document<AccountId, A, RateId, Role, N, SceneFn>(
    spec: DocumentSpec<'_>,
    initial: &Economy<AccountId, A, RateId, Role, N>,
    goal: &Goal<AccountId, A, N>,
    trace: &Trace<RateId, Role, AccountId, N>,
    objectives: Vec<ObjectiveView>,
    scene: SceneFn,
) -> Result<ViewDocument, PlaybackError>
where
    AccountId: Clone + Debug + Eq + Hash + Ord,
    A: Clone + Debug + Eq + Hash + Ord,
    RateId: Clone + Debug + Eq + Hash + Ord,
    Role: Clone + Debug + Ord,
    N: QuantityScalar,
    SceneFn: Fn(u64, &Economy<AccountId, A, RateId, Role, N>) -> Option<Scene>,
{
    let ontology =
        DebugOntology::<AccountId, A, RateId, Role, N>::new(spec.problem).with_scene(scene);
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
    AccountId: Clone + Debug + Eq + Hash + Ord,
    A: Clone + Debug + Eq + Hash + Ord,
    RateId: Clone + Debug + Eq + Hash + Ord,
    Role: Clone + Debug + Ord,
    N: QuantityScalar,
{
    let ontology = DebugOntology::<AccountId, A, RateId, Role, N>::new(namespace);
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
    documents: Vec<ViewDocument>,
) -> Result<RunArtifact, ServiceError> {
    let selected_document_id = format!("{}:{}", descriptor.key, selected_strategy);
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
            "{}:{}:{}:{}",
            descriptor.key, selected_strategy, request.seed, request.budget
        ),
        problem: descriptor.clone(),
        request: request.clone(),
        selected_document_id,
        documents,
        assessed_proposals: Vec::new(),
    })
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
