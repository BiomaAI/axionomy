use crate::{
    Capability, InstanceDescriptor, InstanceProfile, ProblemDescriptor, ProblemFamily,
    ReferenceService, RunArtifact, RunControl, RunObserver, RunRequest, ServiceError,
    ServiceProgress, StrategyDescriptor,
};
use axionomy::{Economy, Goal, QuantityScalar, Trace};
use axionomy_search::GraphSearchProgress;
use axionomy_view::{
    DebugOntology, ExchangeFrame, LeaderboardView, ObjectiveView, PlaybackError, ProposalView,
    Scene, SceneAnchorView, SceneEntityRoleView, SceneEntityView, SceneEvidenceRefView,
    SceneGlyphView, SceneMetricView, SceneToneView, SearchObservationKindView,
    SearchObservationView, SearchTelemetryView, TelemetryKindView, TelemetryPointView,
    ViewDocument, ViewDocumentMetadata, ViewId, ViewOntology, ViewSource, derive_document,
    derive_document_with_frames, derive_model, derive_proposal,
};
use std::{collections::BTreeSet, fmt::Debug, hash::Hash, ops::ControlFlow};

pub(super) fn visual_entity(
    key: impl Into<String>,
    label: impl Into<String>,
    glyph: SceneGlyphView,
    anchor: SceneAnchorView,
    role: SceneEntityRoleView,
    tone: SceneToneView,
    status: Option<String>,
) -> SceneEntityView {
    SceneEntityView {
        id: ViewId::new(key, label),
        glyph,
        anchor,
        role,
        tone,
        status,
        account: None,
        evidence: Vec::new(),
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

/// Compare replayed integer outcomes without claiming an exhaustive frontier.
pub(super) fn evaluated_integer_front(
    documents: &[ViewDocument],
    selected: &ViewDocument,
    title: &str,
) -> axionomy_view::ParetoFrontView {
    use axionomy_view::{
        FrontierCompletenessView, ObjectiveAxisView, ObjectiveDirectionView, ParetoFrontView,
        ParetoPointView,
    };
    let rows: Vec<_> = documents
        .iter()
        .map(|document| {
            (
                document,
                document
                    .objectives
                    .iter()
                    .map(|o| o.value.parse::<u128>().expect("integer outcome"))
                    .collect::<Vec<_>>(),
            )
        })
        .collect();
    let dominates = |left: &[u128], right: &[u128]| {
        left.iter()
            .zip(right)
            .zip(&selected.objectives)
            .all(|((l, r), axis)| match axis.direction {
                ObjectiveDirectionView::Maximize => l >= r,
                ObjectiveDirectionView::Minimize => l <= r,
            })
            && left
                .iter()
                .zip(right)
                .zip(&selected.objectives)
                .any(|((l, r), axis)| match axis.direction {
                    ObjectiveDirectionView::Maximize => l > r,
                    ObjectiveDirectionView::Minimize => l < r,
                })
    };
    ParetoFrontView {
        title: title.into(),
        completeness: FrontierCompletenessView::Approximate,
        axes: selected
            .objectives
            .iter()
            .map(|o| ObjectiveAxisView {
                key: o.key.clone(),
                label: o.label.clone(),
                direction: o.direction,
            })
            .collect(),
        points: rows
            .iter()
            .filter(|(_, candidate)| !rows.iter().any(|(_, other)| dominates(other, candidate)))
            .map(|(document, values)| ParetoPointView {
                label: document.title.clone(),
                selected: document.id == selected.id,
                values: values.iter().map(ToString::to_string).collect(),
            })
            .collect(),
    }
}

pub(super) fn link_account(
    mut entity: SceneEntityView,
    account: impl Into<String>,
) -> SceneEntityView {
    let account = account.into();
    entity.account = Some(account.clone());
    entity
        .evidence
        .push(SceneEvidenceRefView::Account { account });
    entity
}

pub(super) fn link_balance(
    mut entity: SceneEntityView,
    account: impl Into<String>,
    asset: impl Into<String>,
) -> SceneEntityView {
    let account = account.into();
    entity.account = Some(account.clone());
    entity.evidence.push(SceneEvidenceRefView::Balance {
        account,
        asset: asset.into(),
    });
    entity
}

mod amm;
mod bridge;
mod connect_four;
mod exact_cover;
mod labels;
mod lending;
mod logistics;
mod marketplace;
mod maze;
mod mission;
mod perishables;
mod rescue;
mod scheduling;
mod sokoban;
mod work_league;
mod workshop;

use labels::StudioLabel;

type StudioScene<AccountId, A, RateId, Role, N> =
    fn(u64, &Economy<AccountId, A, RateId, Role, N>) -> Option<Scene>;
type StudioLeaderboards<AccountId, A, RateId, Role, N> =
    fn(u64, &Economy<AccountId, A, RateId, Role, N>) -> Vec<LeaderboardView>;
type StudioProjections<AccountId, A, RateId, Role, N> = (
    StudioScene<AccountId, A, RateId, Role, N>,
    StudioLeaderboards<AccountId, A, RateId, Role, N>,
);

struct StudioOntology<AccountId, A, RateId, Role, N = u64> {
    fallback: DebugOntology<AccountId, A, RateId, Role, N>,
    scene: Option<StudioScene<AccountId, A, RateId, Role, N>>,
    leaderboards: Option<StudioLeaderboards<AccountId, A, RateId, Role, N>>,
}

impl<AccountId, A, RateId, Role, N> StudioOntology<AccountId, A, RateId, Role, N> {
    fn new(
        namespace: impl Into<String>,
        scene: Option<StudioScene<AccountId, A, RateId, Role, N>>,
    ) -> Self {
        Self {
            fallback: DebugOntology::new(namespace),
            scene,
            leaderboards: None,
        }
    }

    fn with_leaderboards(
        mut self,
        leaderboards: StudioLeaderboards<AccountId, A, RateId, Role, N>,
    ) -> Self {
        self.leaderboards = Some(leaderboards);
        self
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

    fn leaderboards(
        &self,
        index: u64,
        economy: &Economy<AccountId, A, RateId, Role, N>,
    ) -> Vec<LeaderboardView> {
        self.leaderboards
            .map_or_else(Vec::new, |leaderboards| leaderboards(index, economy))
    }
}

pub(crate) fn catalog() -> Vec<ProblemDescriptor> {
    vec![
        problem(
            ProblemCopy {
                key: "maze",
                title: "Key-door maze",
                summary: "Reach the exit under limited energy and time. The key-and-gate route saves energy, direct routes save time, and the exact frontier preserves four tradeoffs that one score would hide.",
                instances: [
                    "The smallest maze with a key, a locked gate, and a real route choice",
                    "15 rooms and 25 passages with shared power, protection, persistent injury delays and recovery",
                    "24 rooms and 42 passages with more hazards and a second recovery route",
                ],
            },
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
                    "dijkstra",
                    "Least energy · uninformed",
                    "Dijkstra proves the lowest-energy route without a heuristic.",
                    "Dijkstra",
                ),
                (
                    "a_star",
                    "Least energy",
                    "A*, guided by an admissible distance estimate stored in the economy itself.",
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
            ProblemCopy {
                key: "sokoban",
                title: "Sokoban",
                summary: "Push crates onto their goal squares. Every push rewrites three cells at once, and some positions can never be recovered.",
                instances: [
                    "A bounded one-crate teaching puzzle",
                    "Two crates, a pressure plate, a gated chamber, and a remote latch",
                    "Three crates and two coupled door circuits across three chambers",
                ],
            },
            ProblemFamily::Pathfinding,
            "a_star",
            &[
                (
                    "a_star",
                    "Goal-directed A*",
                    "A* prioritises states using an admissible crate-to-goal distance bound.",
                    "A*",
                ),
                (
                    "breadth_first",
                    "Fewest exchanges",
                    "Breadth-first search explores every shorter replay before accepting a solution.",
                    "breadth-first search",
                ),
            ],
            &[
                Capability::DeterministicSearch,
                Capability::MultiAccountExchange,
                Capability::FeasibilityAssessment,
            ],
        ),
        problem(
            ProblemCopy {
                key: "exact_cover",
                title: "Exact cover",
                summary: "Choose subsets that cover every element exactly once. A plain search and Algorithm X reach the same answer by different routes.",
                instances: [
                    "Four elements and four competing subsets",
                    "Eight coupled elements, fourteen irregular subsets, legal dead ends and covers of different sizes",
                    "Twelve globally coupled elements and twenty-five pairs, triples and quadruples",
                ],
            },
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
            ProblemCopy {
                key: "workshop",
                title: "Workshop",
                summary: "Choose funded contracts, prepare components, manage tool wear and deliver products. Exact tradeoffs expose faster work, lower waste and higher revenue.",
                instances: [
                    "Two chairs made from raw stock",
                    "Choose two of three chair/table contracts with components, tool wear, repair, upgrades and salvage",
                    "Choose at least three of five contracts under shared wood, time, labor and repair constraints",
                ],
            },
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
            ProblemCopy {
                key: "scheduling",
                title: "Job-shop scheduling",
                summary: "Route arriving jobs through queues and actual execution. Alternative machines, setup changes, shared crews and a recoverable machine failure change which schedule finishes first.",
                instances: [
                    "Two jobs on the smallest horizon that still forces a machine conflict",
                    "Six operations, three alternative machines, two crews and a recoverable disruption at tick three",
                    "One shared crew, later job arrival and a longer repair create a different execution bottleneck",
                ],
            },
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
            ProblemCopy {
                key: "rescue",
                title: "Uncertain rescue",
                summary: "Search for several hidden casualties while their condition worsens. Trade treatment time, limited medicine, noisy hints and transport fuel against one evacuation seat.",
                instances: [
                    "Two possible sites and eight hidden location/sensor outcomes",
                    "Three casualties across four sites, continuing deterioration, two sensor charges, stabilization and finite resupply",
                    "Four casualties across five sites, tighter fuel and crew energy, one evacuation seat and varied deadlines",
                ],
            },
            ProblemFamily::PartialObservation,
            "observe_then_follow",
            &[
                (
                    "observe_then_follow",
                    "Observe and triage",
                    "Micro evaluates the single search gamble; larger incidents repeatedly scan, search, triage, stabilize and evacuate from public observations.",
                    "Responsive triage; Micro Monte Carlo",
                ),
                (
                    "direct_north",
                    "Rapid sector sweep",
                    "Start north without sensing, then revisit search and evacuation decisions as the incident evolves.",
                    "Responsive rapid sweep; Micro Monte Carlo",
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
            ProblemCopy {
                key: "bridge",
                title: "Bridge allocation",
                summary: "Timed traffic competes for one occupied lane. Compare fixed priority, urgency-based bids and a paid concurrent detour against arrivals, deadlines and expiring reservations.",
                instances: [
                    "One crossing round for two agents and one lane",
                    "Two traffic waves with staggered arrivals, 2–3 tick crossings, deadlines and a paid detour",
                    "Three traffic waves with persistent credit and changing deadline pressure",
                ],
            },
            ProblemFamily::Allocation,
            "auction",
            &[
                (
                    "breadth_first",
                    "Plain search",
                    "Breadth-first routing search with immediate ready completions; timed search excludes bidding and deliberate lease expiry.",
                    "breadth-first search",
                ),
                (
                    "first_come_a",
                    "First come: A",
                    "Wait for Agent A, give A first priority, then serve B.",
                    "first-come allocation",
                ),
                (
                    "first_come_b",
                    "First come: B",
                    "Wait for Agent B, give B first priority, then serve A.",
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
                    "Micro: exact frontier. Timed traffic: A's preference among the evaluated routing and auction policies.",
                    "exact Pareto search",
                ),
                (
                    "pareto_b",
                    "Pareto: favors B",
                    "Micro: exact frontier. Timed traffic: B's preference among the evaluated routing and auction policies.",
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
            ProblemCopy {
                key: "amm",
                title: "The Living Market",
                summary: "Watch repeated production and trade reshape a closed economy. Daily needs and obligations, cloudy periods, limited storage, delayed capacity investment, and responsive liquidity interact through an exact constant-product AMM. Missed needs and defaults remain visible.",
                instances: [
                    "A compact closed market that exposes every reserve calculation and refusal",
                    "Eight market days with recurring needs, finite credit, production investment, and responsive trading",
                    "Twelve market days at four times the production scale with the same seeded sunlight across counterfactuals",
                ],
            },
            ProblemFamily::Market,
            "market_day",
            &[
                (
                    "market_day",
                    "Responsive market",
                    "Actors revisit production, trade, investment, and liquidity decisions throughout the market season.",
                    "verified autonomous market replay",
                ),
                (
                    "no_whale",
                    "Counterfactual: no whale",
                    "Replay the same economy without its largest buyer and inspect the causal price difference.",
                    "counterfactual replay",
                ),
                (
                    "thin_liquidity",
                    "Counterfactual: thin liquidity",
                    "Withdraw half the founding liquidity and inspect how the same actors adapt to different depth and prices.",
                    "counterfactual replay",
                ),
            ],
            &[
                Capability::AtomicSettlement,
                Capability::MultiAccountExchange,
                Capability::FeasibilityAssessment,
                Capability::ReplayDerivedLeaderboards,
                Capability::NonFungibleFacts,
            ],
        ),
        problem(
            ProblemCopy {
                key: "lending",
                title: "Lending market",
                summary: "Finance a bill that falls due before revenue arrives. Collateral caps the loan, interest follows pool utilization along a kinked curve, a reserve cut stays with the market, and a scheduled price drop can push an over-borrowed loan into liquidation.",
                instances: [
                    "One pooled market and a two-period gap between the bill and revenue, with a price drop while the loan is open",
                    "A thin pool near its kink, a bill due early, revenue in two installments and a 40% price drop while the loan is open",
                    "The Showcase loan at four times the scale over twelve periods, with revenue in three installments",
                ],
            },
            ProblemFamily::Market,
            "least_interest",
            &[
                (
                    "least_interest",
                    "Least interest",
                    "Dijkstra search charging each closed period's interest to the plan.",
                    "Dijkstra search",
                ),
                (
                    "fewest_steps",
                    "Fewest steps",
                    "Breadth-first search: the fewest exchanges, whatever the loan costs.",
                    "breadth-first search",
                ),
                (
                    "over_borrowed",
                    "Over-borrowed",
                    "Borrow to the limit at once and watch interest and the price drop end in liquidation.",
                    "scripted scenario",
                ),
                (
                    "first_depositor_attack",
                    "First-depositor attack",
                    "Try to inflate a fresh market's share price against a later depositor.",
                    "scripted scenario",
                ),
            ],
            &[
                Capability::DeterministicSearch,
                Capability::WeightedSearch,
                Capability::AtomicSettlement,
                Capability::MultiAccountExchange,
                Capability::FeasibilityAssessment,
                Capability::TemporalEffects,
            ],
        ),
        problem(
            ProblemCopy {
                key: "marketplace",
                title: "Multi-party marketplace",
                summary: "Negotiate funded contracts, reserve scarce shipping capacity and fulfill deliveries over time. Route delays, price concessions, late rebates and refundable reservations couple buyer and seller outcomes.",
                instances: [
                    "Two orders across three buyers, three sellers, and one active carrier",
                    "Four arriving orders with escrow, two occupied carriers, delivery deadlines and price concessions",
                    "Six arriving orders with scarce stock, persistent budgets, route delays and reusable carrier capacity",
                ],
            },
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
                    "Micro: exact clearing frontier. Larger markets: bounded fulfillment search favoring buyer benefit.",
                    "exact Pareto clearing",
                ),
                (
                    "pareto_sellers",
                    "Pareto: favors sellers",
                    "Micro: exact clearing frontier. Larger markets: bounded fulfillment search favoring seller benefit.",
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
            ProblemCopy {
                key: "logistics",
                title: "Stochastic logistics",
                summary: "Coordinate refrigerated and heavy-freight vans across three customers. Arriving orders, delivery windows, a shared trunk road, finite fuel, occupied service bays and hub handoffs interact with route disruptions.",
                instances: [
                    "Two deliveries with a compact route and a small sample budget",
                    "Two vans, four heterogeneous orders, three customers, one trunk-road slot, two-unit relay hub and finite services",
                    "Six arriving orders, more route delays, a longer horizon, finite fuel and three repair parts",
                ],
            },
            ProblemFamily::StochasticPlanning,
            "reliable",
            &[
                (
                    "direct",
                    "Fastest routes",
                    "Use express routes and respond to realized breakdowns; Micro retains sampled single-vehicle evaluation.",
                    "Responsive fleet dispatch; Micro Monte Carlo",
                ),
                (
                    "reliable",
                    "Reliable routes",
                    "Avoid the express link and coordinate deliveries around shared road and service capacity.",
                    "Responsive fleet dispatch; Micro Monte Carlo",
                ),
                (
                    "mcts",
                    "Relay fleet / Micro MCTS",
                    "Larger fleets bundle compatible cargo and hand it off through the hub; Micro retains full receding-horizon Monte Carlo tree search.",
                    "Responsive hub relay; Micro Monte Carlo tree search",
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
            ProblemCopy {
                key: "connect_four",
                title: "Connect Four",
                summary: "Tactical Connect Four with forced blocks, double threats, positional search, and distinct opponents; gravity and terminal outcomes are core rules.",
                instances: [
                    "A compact board with a short game and few rollouts",
                    "The full 7×6 board, all 69 lines, tactical self-play and two distinct opponents",
                    "The full board with four times the MCTS work per move",
                ],
            },
            ProblemFamily::AdversarialGame,
            "mcts_game",
            &[
                (
                    "mcts_game",
                    "Tactical MCTS self-play",
                    "Both players recognize wins, forced blocks and double threats before exploring positional continuations.",
                    "tactical Monte Carlo tree search",
                ),
                (
                    "center_opponent",
                    "Center-first opponent",
                    "Red uses tactical MCTS against a center-first Yellow baseline on the standard board.",
                    "tactical MCTS versus positional baseline",
                ),
                (
                    "search_opponent",
                    "Depth-five opponent",
                    "Red uses tactical MCTS against depth-five alpha-beta Yellow on the standard board.",
                    "tactical MCTS versus alpha-beta",
                ),
            ],
            &[Capability::Mcts, Capability::MultiAccountExchange],
        ),
        problem(
            ProblemCopy {
                key: "work_league",
                title: "Autonomous Work League",
                summary: "Four workers share one clock: jobs overlap, repair and charging bays fill, claims expire, and deadlines can be missed. Earned credit funds recovery; recycling restores material; a worker can pay to take over another's contract. Outcomes compare the same job-attempt disruptions across policy fields.",
                instances: [
                    "Two agents contest four jobs in a short but complete match",
                    "Four policies contest 12 arriving contracts with deadlines, dependencies, occupied facilities, and paid handoffs",
                    "24 contracts over six arrival waves with shared finite supplies and equipment wear",
                ],
            },
            ProblemFamily::MultiAgentCompetition,
            "mixed_field",
            &[
                (
                    "mixed_field",
                    "Mixed policy field",
                    "A sprinter, steward, value hunter, and resilient worker reveal genuine cross-objective tradeoffs.",
                    "deterministic policy league with seeded chance",
                ),
                (
                    "throughput_field",
                    "Throughput field",
                    "Most workers prioritize short jobs and fast execution; one still chases contract value.",
                    "throughput-biased policy league with seeded chance",
                ),
                (
                    "sustainable_field",
                    "Sustainable field",
                    "Most workers minimize material loss and recycle, while one specializes in reliability.",
                    "resource-biased policy league with seeded chance",
                ),
            ],
            &[
                Capability::MultiAccountExchange,
                Capability::AtomicSettlement,
                Capability::Chance,
                Capability::ReplayDerivedLeaderboards,
                Capability::ApproximatePareto,
                Capability::RlProjection,
            ],
        ),
        problem(
            ProblemCopy {
                key: "mission",
                title: "Hidden-information mission",
                summary: "Coordinate independent Scout and Medic timelines. Share private discoveries, recover archives, stabilize casualties and rendezvous for extraction as hazards close sectors and resources run short.",
                instances: [
                    "Sixteen hidden scenarios with a small per-decision search budget",
                    "A hidden archive and casualty, independent travel, timed recovery, radio or local briefing, sector hazards and clinical decline",
                    "A second casualty, one radio charge, lower energy, finite resupply and one evacuation seat",
                ],
            },
            ProblemFamily::PartialObservation,
            "coordinated",
            &[
                (
                    "coordinated",
                    "Coordinate independent agents",
                    "Larger missions use separate observation-responsive policies with radio briefing; Micro retains belief-conditioned ISMCTS.",
                    "Private-view policies; Micro information-set MCTS",
                ),
                (
                    "direct_north",
                    "Search north first",
                    "Larger missions begin north, then coordinate through local briefings. Micro retains the blind joint-movement baseline.",
                    "North-first local coordination; Micro Monte Carlo",
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
            ProblemCopy {
                key: "perishables",
                title: "Perishable inventory",
                summary: "Manage scarce refrigerated space, thermal history and daily food obligations. Cooling and processing compete for electricity; backup generation and deliveries compete for fuel.",
                instances: [
                    "One hundred units split between two batches",
                    "Ten crates across six possible cohorts, six days, finite cold storage and a two-day outage",
                    "Twelve crates, tighter cold storage, less fuel, eight days and a three-day outage",
                ],
            },
            ProblemFamily::TemporalSimulation,
            "outage",
            &[
                (
                    "outage",
                    "Power outage",
                    "Prioritize refrigeration, then revisit shipment, processing and backup decisions each day.",
                    "indexed temporal effects",
                ),
                (
                    "pareto_inventory",
                    "Pareto: save stock",
                    "Micro: exact storage frontier. Larger cold chains: a processing-first policy on the same outage schedule.",
                    "exact Pareto search",
                ),
                (
                    "pareto_energy",
                    "Pareto: save energy",
                    "Micro: minimum-energy storage frontier. Larger cold chains: alternate-day shipping to conserve fuel, with missed demand kept visible.",
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

struct ProblemCopy<'a> {
    key: &'a str,
    title: &'a str,
    summary: &'a str,
    instances: [&'a str; 3],
}

fn problem(
    copy: ProblemCopy<'_>,
    family: ProblemFamily,
    default_strategy: &str,
    strategies: &[(&str, &str, &str, &str)],
    capabilities: &[Capability],
) -> ProblemDescriptor {
    ProblemDescriptor {
        key: copy.key.into(),
        title: copy.title.into(),
        summary: copy.summary.into(),
        family,
        default_instance: "showcase".into(),
        instances: vec![
            InstanceDescriptor {
                key: "micro".into(),
                label: "Micro".into(),
                description: copy.instances[0].into(),
                profile: InstanceProfile::Micro,
            },
            InstanceDescriptor {
                key: "showcase".into(),
                label: "Showcase".into(),
                description: copy.instances[1].into(),
                profile: InstanceProfile::Showcase,
            },
            InstanceDescriptor {
                key: "stress".into(),
                label: "Stress".into(),
                description: copy.instances[2].into(),
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
        "maze" => maze::build(request, &descriptor, &mut progress),
        "sokoban" => sokoban::build(request, &descriptor, &mut progress),
        "exact_cover" => exact_cover::build(request, &descriptor),
        "workshop" => workshop::build(request, &descriptor),
        "scheduling" => scheduling::build(request, &descriptor),
        "rescue" => rescue::build(request, &descriptor),
        "bridge" => bridge::build(request, &descriptor),
        "amm" => amm::build(request, &descriptor),
        "lending" => lending::build(request, &descriptor),
        "marketplace" => marketplace::build(request, &descriptor),
        "logistics" => logistics::build(request, &descriptor, &mut progress),
        "connect_four" => connect_four::build(request, &descriptor, &mut progress),
        "mission" => mission::build(request, &descriptor, &mut progress),
        "perishables" => perishables::build(request, &descriptor),
        "work_league" => work_league::build(request, &descriptor, &mut progress),
        _ => unreachable!("catalog and dispatch must agree"),
    }?;
    progress.ensure()?;
    if let Some(document) = artifact.selected_document()
        && !progress.has_frames(&document.id)
    {
        for frame in &document.frames {
            progress.frame(&document.id, frame);
        }
    }
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
        // Problem adapters may retain richer, strategy-specific evidence. The
        // live stream is the transport fallback, not permission to relabel one
        // algorithm's observations as every alternative in the artifact.
        if document.solve_observations.is_empty() {
            document.solve_observations = solve_observations.clone();
        }
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
    framed_documents: BTreeSet<String>,
}

impl<'a> ProgressSink<'a> {
    fn new(control: &'a RunControl, observer: &'a mut dyn RunObserver) -> Self {
        Self {
            control,
            observer,
            sequence: 0,
            error: None,
            observations: Vec::new(),
            framed_documents: BTreeSet::new(),
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
            algorithm: phase
                .trim_end_matches("_frontier")
                .trim_end_matches("_solution")
                .into(),
            kind: observation_kind(&phase),
            phase,
            label: message,
            completed,
            total,
            subjects: Vec::new(),
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

    /// Publish graph-search work without pretending that an implicit frontier
    /// has a knowable completion percentage.
    pub fn graph(
        &mut self,
        phase: impl Into<String>,
        progress: GraphSearchProgress,
        message: impl Into<String>,
    ) -> ControlFlow<()> {
        self.graph_with_subjects(phase, progress, message, std::iter::empty())
    }

    /// Publish exact graph-search counters together with the Rust-owned scene
    /// subjects expanded during this checkpoint.
    pub fn graph_with_subjects(
        &mut self,
        phase: impl Into<String>,
        progress: GraphSearchProgress,
        message: impl Into<String>,
        subjects: impl IntoIterator<Item = ViewId>,
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
        let completed = progress.expanded() as u64;
        self.observer.progress(ServiceProgress {
            sequence: self.sequence,
            phase: phase.clone(),
            completed,
            // Zero means the frontier has no honest fixed denominator. The UI
            // renders an indeterminate bar and the exact counters below.
            total: 0,
            message: message.clone(),
        });
        let observation = SearchObservationView {
            sequence: self.sequence,
            algorithm: phase.clone(),
            kind: observation_kind(&phase),
            phase,
            label: message,
            completed,
            total: 0,
            subjects: subjects.into_iter().collect(),
            metrics: vec![
                visual_metric("expanded", "States expanded", progress.expanded(), None),
                visual_metric("generated", "States generated", progress.generated(), None),
                visual_metric("frontier", "Frontier", progress.frontier(), None),
                visual_metric("visited", "States visited", progress.visited(), None),
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

    pub fn frame(&mut self, document_id: &str, frame: &ExchangeFrame) {
        if self.error.is_none() {
            self.framed_documents.insert(document_id.into());
            self.observer.frame(document_id, frame.clone());
        }
    }

    fn has_frames(&self, document_id: &str) -> bool {
        self.framed_documents.contains(document_id)
    }
}

fn observation_kind(phase: &str) -> SearchObservationKindView {
    if phase.contains("solution") || phase.contains("incumbent") {
        SearchObservationKindView::Incumbent
    } else if phase.contains("pareto") || phase.contains("frontier") {
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

pub(super) fn document_with_leaderboards_observed<AccountId, A, RateId, Role, N>(
    spec: DocumentSpec<'_>,
    initial: &Economy<AccountId, A, RateId, Role, N>,
    goal: &Goal<AccountId, A, N>,
    trace: &Trace<RateId, Role, AccountId, N>,
    objectives: Vec<ObjectiveView>,
    projections: StudioProjections<AccountId, A, RateId, Role, N>,
    mut on_frame: impl FnMut(&ExchangeFrame),
) -> Result<ViewDocument, PlaybackError>
where
    AccountId: Clone + Debug + Eq + Hash + Ord + StudioLabel,
    A: Clone + Debug + Eq + Hash + Ord + StudioLabel,
    RateId: Clone + Debug + Eq + Hash + Ord + StudioLabel,
    Role: Clone + Debug + Ord + StudioLabel,
    N: QuantityScalar,
{
    let (scene, leaderboards) = projections;
    let ontology = StudioOntology::<AccountId, A, RateId, Role, N>::new(spec.problem, Some(scene))
        .with_leaderboards(leaderboards);
    let mut document = derive_document_with_frames(
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
        &mut on_frame,
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
                scene
                    .validate_evidence(&snapshot.accounts)
                    .map_err(|error| ServiceError::Problem {
                        problem: descriptor.key.clone(),
                        message: format!(
                            "invalid scene evidence at snapshot {}: {error}",
                            snapshot.index
                        ),
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
