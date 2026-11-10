use crate::{
    Capability, InstanceDescriptor, InstanceProfile, ProblemDescriptor, ProblemFamily,
    ServiceError, ServiceProgress, StrategyDescriptor, Study, StudyControl, StudyKey,
    StudyObserver,
};
use axionomy::{Economy, Goal, Ontology, Quantity, Trace, paths};
use axionomy_search::{GraphSearchProgress, rollout::RolloutResult};
use axionomy_view::{
    DocumentPath, ExchangeFrame, ItemPath, LeaderboardView, MeasurePath, ObjectiveView,
    PathOntology, PhasePath, PlaybackError, ProposalView, QuantityFormat, Scene, SceneAnchorView,
    SceneEntityRoleView, SceneEntityView, SceneEvidenceRefView, SceneGlyphView, SceneMetricView,
    SceneToneView, SearchObservationKindView, SearchObservationView, SearchTelemetryView,
    TelemetryKindView, TelemetryPointView, ViewDocument, ViewDocumentMetadata, ViewId,
    ViewOntology, ViewSource, WrittenOntology, derive_document, derive_document_with_frames,
    derive_model, derive_proposal, slug,
};
use serde::{Serialize, de::DeserializeOwned};
use std::{collections::BTreeSet, ops::ControlFlow};

pub(super) fn visual_entity(
    key: ItemPath,
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
    key: MeasurePath,
    label: impl Into<String>,
    value: impl ToString,
    unit: Option<&str>,
) -> SceneMetricView {
    SceneMetricView {
        key,
        label: label.into(),
        value: value.to_string(),
        unit: unit.map(str::to_owned),
        previous: None,
    }
}

/// A scene metric showing a quantity of `asset`, written in the asset's
/// format, so a scene and the inspectors read it in the same unit.
pub(super) fn asset_metric<A: StudioFormat>(
    key: MeasurePath,
    label: impl Into<String>,
    asset: &A,
    quantity: impl Into<Quantity>,
) -> SceneMetricView {
    let format = asset.studio_format();
    SceneMetricView {
        key,
        label: label.into(),
        value: format.number(quantity.into()),
        unit: format.unit,
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

/// The key every view of `O` gives `account`, so a scene names the account
/// a frame shows.
pub(super) fn account_key<O: WrittenOntology>(account: &O::Account) -> ItemPath {
    PathOntology::<O>::default().account(account).key
}

/// The key every view of `O` gives `asset`.
pub(super) fn asset_key<O: WrittenOntology>(asset: &O::Asset) -> ItemPath {
    PathOntology::<O>::default().asset(asset).key
}

/// Shows `entity` as `account`, citing the account as its evidence.
pub(super) fn link_account<O: WrittenOntology>(
    mut entity: SceneEntityView,
    account: &O::Account,
) -> SceneEntityView {
    let account = account_key::<O>(account);
    entity.account = Some(account.clone());
    entity
        .evidence
        .push(SceneEvidenceRefView::Account { account });
    entity
}

/// Shows `entity` as `account`, citing its balance of `asset` as evidence.
pub(super) fn link_balance<O: WrittenOntology>(
    mut entity: SceneEntityView,
    account: &O::Account,
    asset: &O::Asset,
) -> SceneEntityView {
    let account = account_key::<O>(account);
    entity.account = Some(account.clone());
    entity.evidence.push(SceneEvidenceRefView::Balance {
        account,
        asset: asset_key::<O>(asset),
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

use labels::{StudioFormat, StudioLabel, counted, debug_labels, words};

type StudioScene<O> = fn(u64, &Economy<O>) -> Option<Scene>;
type StudioLeaderboards<O> = fn(u64, &Economy<O>) -> Vec<LeaderboardView>;
type StudioProjections<O> = (StudioScene<O>, StudioLeaderboards<O>);

/// Studio's view of an ontology: keys from [`PathOntology`], under the
/// ontology's name; labels from [`StudioLabel`]; and the problem's scene and
/// leaderboards.
struct StudioOntology<O: Ontology> {
    fallback: PathOntology<O>,
    scene: Option<StudioScene<O>>,
    leaderboards: Option<StudioLeaderboards<O>>,
}

impl<O: Ontology> StudioOntology<O> {
    fn new(scene: Option<StudioScene<O>>) -> Self {
        Self {
            fallback: PathOntology::default(),
            scene,
            leaderboards: None,
        }
    }

    fn with_leaderboards(mut self, leaderboards: StudioLeaderboards<O>) -> Self {
        self.leaderboards = Some(leaderboards);
        self
    }

    fn relabel<T: StudioLabel>(mut id: ViewId, value: &T) -> ViewId {
        id.label = value.studio_label();
        id
    }
}

impl<O: WrittenOntology> ViewOntology<O> for StudioOntology<O>
where
    O::Account: StudioLabel,
    O::Asset: StudioLabel + StudioFormat,
    O::Rate: StudioLabel,
    O::Role: StudioLabel,
{
    fn account(&self, id: &O::Account) -> ViewId {
        Self::relabel(self.fallback.account(id), id)
    }

    fn asset(&self, id: &O::Asset) -> ViewId {
        Self::relabel(self.fallback.asset(id), id)
    }

    fn rate(&self, id: &O::Rate) -> ViewId {
        Self::relabel(self.fallback.rate(id), id)
    }

    fn role(&self, id: &O::Role) -> ViewId {
        Self::relabel(self.fallback.role(id), id)
    }

    fn format(&self, asset: &O::Asset) -> QuantityFormat {
        asset.studio_format()
    }

    fn scene(&self, index: u64, economy: &Economy<O>) -> Option<Scene> {
        self.scene.and_then(|scene| scene(index, economy))
    }

    fn leaderboards(&self, index: u64, economy: &Economy<O>) -> Vec<LeaderboardView> {
        self.leaderboards
            .map_or_else(Vec::new, |leaderboards| leaderboards(index, economy))
    }
}

type Build =
    fn(&StudyKey, &ProblemDescriptor, &mut ProgressSink<'_>) -> Result<Study, ServiceError>;

pub(crate) fn catalog() -> Vec<ProblemDescriptor> {
    problems()
        .into_iter()
        .map(|(descriptor, _)| descriptor)
        .collect()
}

fn problems() -> Vec<(ProblemDescriptor, Build)> {
    vec![
        problem(
            ProblemCopy {
                key: "maze",
                build: maze::build,
                title: "Key-door maze",
                summary: "Reach the exit under limited energy and time. The key-and-gate route saves energy, direct routes save time, and the exact frontier preserves four tradeoffs that one score would hide.",
                instances: [
                    "The smallest maze with a key, a locked gate, and a real route choice",
                    "15 rooms and 25 passages with shared power, protection, persistent injury delays and recovery",
                    "24 rooms and 42 passages with more hazards and a second recovery route",
                ],
            },
            ProblemFamily::Pathfinding,
            "a-star",
            &[
                (
                    "breadth-first",
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
                    "a-star",
                    "Least energy",
                    "A*, guided by an admissible distance estimate stored in the economy itself.",
                    "A*",
                ),
                (
                    "pareto-energy",
                    "Pareto: least energy",
                    "The lowest-energy point on the exact Pareto frontier, where no route is better on both energy and time.",
                    "exact Pareto search",
                ),
                (
                    "pareto-time",
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
                build: sokoban::build,
                title: "Sokoban",
                summary: "Push crates onto their goal squares. Every push rewrites three cells at once, and some positions can never be recovered.",
                instances: [
                    "A bounded one-crate teaching puzzle",
                    "Two crates, a pressure plate, a gated chamber, and a remote latch",
                    "Three crates and two coupled door circuits across three chambers",
                ],
            },
            ProblemFamily::Pathfinding,
            "a-star",
            &[
                (
                    "a-star",
                    "Goal-directed A*",
                    "A* prioritises states using an admissible crate-to-goal distance bound.",
                    "A*",
                ),
                (
                    "breadth-first",
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
                key: "exact-cover",
                build: exact_cover::build,
                title: "Exact cover",
                summary: "Choose subsets that cover every element exactly once. A plain search and Algorithm X reach the same answer by different routes.",
                instances: [
                    "Four elements and four competing subsets",
                    "Eight coupled elements, fourteen irregular subsets, legal dead ends and covers of different sizes",
                    "Twelve globally coupled elements and twenty-five pairs, triples and quadruples",
                ],
            },
            ProblemFamily::Constraint,
            "algorithm-x",
            &[
                (
                    "breadth-first",
                    "Plain search",
                    "Breadth-first search with no knowledge of cover structure; it just tries selections.",
                    "breadth-first search",
                ),
                (
                    "algorithm-x",
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
                build: workshop::build,
                title: "Workshop",
                summary: "Choose funded contracts, prepare components, manage tool wear and deliver products. Exact tradeoffs expose faster work, lower waste and higher revenue.",
                instances: [
                    "Two chairs made from raw stock",
                    "Choose two of three chair/table contracts with components, tool wear, repair, upgrades and salvage",
                    "Choose at least three of five contracts under shared wood, time, labor and repair constraints",
                ],
            },
            ProblemFamily::Production,
            "minimum-waste",
            &[
                (
                    "breadth-first",
                    "Fewest steps",
                    "Breadth-first search: the fewest recipe firings, whatever they waste.",
                    "breadth-first search",
                ),
                (
                    "minimum-waste",
                    "Least waste",
                    "Best-first search guided by scrap accumulated in the workshop account.",
                    "best-first search",
                ),
                (
                    "pareto-waste",
                    "Pareto: least waste",
                    "The lowest-scrap point on the exact Pareto frontier.",
                    "exact Pareto search",
                ),
                (
                    "pareto-time",
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
                build: scheduling::build,
                title: "Job-shop scheduling",
                summary: "Route arriving jobs through queues and actual execution. Alternative machines, setup changes, shared crews and a recoverable machine failure change which schedule finishes first.",
                instances: [
                    "Two jobs on the smallest horizon that still forces a machine conflict",
                    "Six operations, three alternative machines, two crews and a recoverable disruption at tick three",
                    "One shared crew, later job arrival and a longer repair create a different execution bottleneck",
                ],
            },
            ProblemFamily::Scheduling,
            "bounded-optimizer",
            &[
                (
                    "best-first",
                    "Shortest makespan",
                    "Best-first search on the finish time recorded in the schedule.",
                    "best-first search",
                ),
                (
                    "bounded-optimizer",
                    "Branch optimizer",
                    "Depth-first branch-and-bound abandons schedules it can already prove are worse.",
                    "branch-and-bound",
                ),
                (
                    "pareto-job-one",
                    "Pareto: Job One first",
                    "The frontier schedule that finishes Job One earliest.",
                    "exact Pareto search",
                ),
                (
                    "pareto-job-two",
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
                build: rescue::build,
                title: "Uncertain rescue",
                summary: "Search for several hidden casualties while their condition worsens. Trade treatment time, limited medicine, noisy hints and transport fuel against one evacuation seat.",
                instances: [
                    "Two possible sites and eight hidden location/sensor outcomes",
                    "Three casualties across four sites, continuing deterioration, two sensor charges, stabilization and finite resupply",
                    "Four casualties across five sites, tighter fuel and crew energy, one evacuation seat and varied deadlines",
                ],
            },
            ProblemFamily::PartialObservation,
            "observe-then-follow",
            &[
                (
                    "observe-then-follow",
                    "Observe and triage",
                    "Micro evaluates the single search gamble; larger incidents repeatedly scan, search, triage, stabilize and evacuate from public observations.",
                    "Responsive triage; Micro Monte Carlo",
                ),
                (
                    "direct-north",
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
                build: bridge::build,
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
                    "breadth-first",
                    "Plain search",
                    "Breadth-first routing search with immediate ready completions; timed search excludes bidding and deliberate lease expiry.",
                    "breadth-first search",
                ),
                (
                    "first-come-a",
                    "First come: A",
                    "Wait for Agent A, give A first priority, then serve B.",
                    "first-come allocation",
                ),
                (
                    "first-come-b",
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
                    "pareto-a",
                    "Pareto: favors A",
                    "Micro: exact frontier. Timed traffic: A's preference among the evaluated routing and auction policies.",
                    "exact Pareto search",
                ),
                (
                    "pareto-b",
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
                build: amm::build,
                title: "The Living Market",
                summary: "Watch repeated production and trade reshape a closed economy. Daily needs and obligations, cloudy periods, limited storage, delayed capacity investment, and responsive liquidity interact through an exact constant-product AMM. Missed needs and defaults remain visible.",
                instances: [
                    "A compact closed market that exposes every reserve calculation and refusal",
                    "Eight market days with recurring needs, finite credit, production investment, and responsive trading",
                    "Twelve market days at four times the production scale with the same seeded sunlight across counterfactuals",
                ],
            },
            ProblemFamily::Market,
            "market-day",
            &[
                (
                    "market-day",
                    "Responsive market",
                    "Actors revisit production, trade, investment, and liquidity decisions throughout the market season.",
                    "verified autonomous market replay",
                ),
                (
                    "no-whale",
                    "Counterfactual: no whale",
                    "Replay the same economy without its largest buyer and inspect the causal price difference.",
                    "counterfactual replay",
                ),
                (
                    "thin-liquidity",
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
                build: lending::build,
                title: "Lending market",
                summary: "Finance a bill that falls due before revenue arrives. Collateral caps the loan, interest follows pool utilization along a kinked curve, a reserve cut stays with the market, and a scheduled price drop can push an over-borrowed loan into liquidation.",
                instances: [
                    "One pooled market and a two-period gap between the bill and revenue, with a price drop while the loan is open",
                    "A thin pool near its kink, a bill due early, revenue in two installments and a 40% price drop while the loan is open",
                    "The Showcase loan at four times the scale over twelve periods, with revenue in three installments",
                ],
            },
            ProblemFamily::Market,
            "least-interest",
            &[
                (
                    "least-interest",
                    "Least interest",
                    "Dijkstra search charging each closed period's interest to the plan.",
                    "Dijkstra search",
                ),
                (
                    "fewest-steps",
                    "Fewest steps",
                    "Breadth-first search: the fewest exchanges, whatever the loan costs.",
                    "breadth-first search",
                ),
                (
                    "over-borrowed",
                    "Over-borrowed",
                    "Borrow to the limit at once and watch interest and the price drop end in liquidation.",
                    "scripted scenario",
                ),
                (
                    "first-depositor-attack",
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
                build: marketplace::build,
                title: "Multi-party marketplace",
                summary: "Negotiate funded contracts, reserve scarce shipping capacity and fulfill deliveries over time. Route delays, price concessions, late rebates and refundable reservations couple buyer and seller outcomes.",
                instances: [
                    "Two orders across three buyers, three sellers, and one active carrier",
                    "Four arriving orders with escrow, two occupied carriers, delivery deadlines and price concessions",
                    "Six arriving orders with scarce stock, persistent budgets, route delays and reusable carrier capacity",
                ],
            },
            ProblemFamily::Market,
            "market-clearing",
            &[
                (
                    "market-clearing",
                    "Clear the market",
                    "Pick orders that can all settle together without overdrawing any account.",
                    "market clearing",
                ),
                (
                    "pareto-buyers",
                    "Pareto: favors buyers",
                    "Micro: exact clearing frontier. Larger markets: bounded fulfillment search favoring buyer benefit.",
                    "exact Pareto clearing",
                ),
                (
                    "pareto-sellers",
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
                build: logistics::build,
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
                key: "connect-four",
                build: connect_four::build,
                title: "Connect Four",
                summary: "Tactical Connect Four with forced blocks, double threats, positional search, and distinct opponents; gravity and terminal outcomes are core rules.",
                instances: [
                    "A compact board with a short game and few rollouts",
                    "The full 7×6 board, all 69 lines, tactical self-play and two distinct opponents",
                    "The full board with four times the MCTS work per move",
                ],
            },
            ProblemFamily::AdversarialGame,
            "mcts-game",
            &[
                (
                    "mcts-game",
                    "Tactical MCTS self-play",
                    "Both players recognize wins, forced blocks and double threats before exploring positional continuations.",
                    "tactical Monte Carlo tree search",
                ),
                (
                    "center-opponent",
                    "Center-first opponent",
                    "Red uses tactical MCTS against a center-first Yellow baseline on the standard board.",
                    "tactical MCTS versus positional baseline",
                ),
                (
                    "search-opponent",
                    "Depth-five opponent",
                    "Red uses tactical MCTS against depth-five alpha-beta Yellow on the standard board.",
                    "tactical MCTS versus alpha-beta",
                ),
            ],
            &[Capability::Mcts, Capability::MultiAccountExchange],
        ),
        problem(
            ProblemCopy {
                key: "work-league",
                build: work_league::build,
                title: "Autonomous Work League",
                summary: "Four workers share one clock: jobs overlap, repair and charging bays fill, claims expire, and deadlines can be missed. Earned credit funds recovery; recycling restores material; a worker can pay to take over another's contract. Outcomes compare the same job-attempt disruptions across policy fields.",
                instances: [
                    "Two agents contest four jobs in a short but complete match",
                    "Four policies contest 12 arriving contracts with deadlines, dependencies, occupied facilities, and paid handoffs",
                    "24 contracts over six arrival waves with shared finite supplies and equipment wear",
                ],
            },
            ProblemFamily::MultiAgentCompetition,
            "mixed-field",
            &[
                (
                    "mixed-field",
                    "Mixed policy field",
                    "A sprinter, steward, value hunter, and resilient worker reveal genuine cross-objective tradeoffs.",
                    "deterministic policy league with seeded chance",
                ),
                (
                    "throughput-field",
                    "Throughput field",
                    "Most workers prioritize short jobs and fast execution; one still chases contract value.",
                    "throughput-biased policy league with seeded chance",
                ),
                (
                    "sustainable-field",
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
                build: mission::build,
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
                    "direct-north",
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
                build: perishables::build,
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
                    "pareto-inventory",
                    "Pareto: save stock",
                    "Micro: exact storage frontier. Larger cold chains: a processing-first policy on the same outage schedule.",
                    "exact Pareto search",
                ),
                (
                    "pareto-energy",
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
    build: Build,
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
) -> (ProblemDescriptor, Build) {
    let descriptor = ProblemDescriptor {
        key: slug(copy.key),
        title: copy.title.into(),
        summary: copy.summary.into(),
        family,
        default_instance: InstanceProfile::Showcase,
        instances: [
            InstanceProfile::Micro,
            InstanceProfile::Showcase,
            InstanceProfile::Stress,
        ]
        .into_iter()
        .zip(["Micro", "Showcase", "Stress"])
        .zip(copy.instances)
        .map(|((profile, label), description)| InstanceDescriptor {
            profile,
            label: label.into(),
            description: description.into(),
        })
        .collect(),
        default_strategy: slug(default_strategy),
        strategies: strategies
            .iter()
            .map(|(key, label, description, algorithm)| StrategyDescriptor {
                key: slug(key),
                label: (*label).into(),
                description: (*description).into(),
                algorithm: (*algorithm).into(),
            })
            .collect(),
        capabilities: capabilities.to_vec(),
    };
    (descriptor, copy.build)
}

pub(crate) fn study(
    key: &StudyKey,
    control: &StudyControl,
    observer: &mut dyn StudyObserver,
) -> Result<Study, ServiceError> {
    let (descriptor, build) = problems()
        .into_iter()
        .find(|(descriptor, _)| descriptor.key == key.problem)
        .ok_or_else(|| ServiceError::UnknownProblem(key.problem.clone()))?;
    let mut progress = ProgressSink::new(control, observer);
    let _ = progress.emit(
        slug("prepare"),
        0,
        1,
        format!("preparing {} · {}", key.problem, key.instance),
    );
    progress.ensure()?;
    let mut study = build(key, &descriptor, &mut progress)?;
    progress.ensure()?;
    if let Some(document) = study.selected()
        && !progress.has_frames(&document.id)
    {
        for frame in &document.frames {
            progress.frame(&document.id, frame);
        }
    }
    for (offset, document) in study.documents.iter().enumerate() {
        let _ = progress.emit(
            slug("artifact"),
            offset as u64 + 1,
            study.documents.len() as u64,
            format!("derived {}", document.title),
        );
        progress.ensure()?;
    }
    study.assessed_proposals = study
        .documents
        .iter()
        .flat_map(|document| document.proposals.iter().cloned())
        .collect();
    let thinking = progress.observations().to_vec();
    for document in &mut study.documents {
        // Problem adapters may retain richer, strategy-specific evidence. The
        // live stream is the transport fallback, not permission to relabel one
        // algorithm's observations as every alternative in the study.
        if document.thinking.is_empty() {
            document.thinking = thinking.clone();
        }
        for frame in &mut document.frames {
            if frame.observations.is_empty() {
                frame.observations = document.observations.clone();
            }
        }
    }
    Ok(study)
}

pub(super) struct ProgressSink<'a> {
    control: &'a StudyControl,
    observer: &'a mut dyn StudyObserver,
    sequence: u64,
    error: Option<ServiceError>,
    observations: Vec<SearchObservationView>,
    framed_documents: BTreeSet<DocumentPath>,
}

impl<'a> ProgressSink<'a> {
    fn new(control: &'a StudyControl, observer: &'a mut dyn StudyObserver) -> Self {
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
        phase: PhasePath,
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
            algorithm: slug(
                phase
                    .as_str()
                    .trim_end_matches("-frontier")
                    .trim_end_matches("-solution"),
            ),
            kind: observation_kind(&phase),
            phase,
            label: message,
            completed,
            total,
            subjects: Vec::new(),
            metrics: vec![
                visual_metric(slug("completed"), "Work completed", completed, None),
                visual_metric(
                    slug("remaining"),
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
        phase: PhasePath,
        progress: GraphSearchProgress,
        message: impl Into<String>,
    ) -> ControlFlow<()> {
        self.graph_with_subjects(phase, progress, message, std::iter::empty())
    }

    /// Publish exact graph-search counters together with the Rust-owned scene
    /// subjects expanded during this checkpoint.
    pub fn graph_with_subjects(
        &mut self,
        phase: PhasePath,
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
            algorithm: slug(phase.as_str()),
            kind: observation_kind(&phase),
            phase,
            label: message,
            completed,
            total: 0,
            subjects: subjects.into_iter().collect(),
            metrics: vec![
                visual_metric(
                    slug("expanded"),
                    "States expanded",
                    progress.expanded(),
                    None,
                ),
                visual_metric(
                    slug("generated"),
                    "States generated",
                    progress.generated(),
                    None,
                ),
                visual_metric(slug("frontier"), "Frontier", progress.frontier(), None),
                visual_metric(slug("visited"), "States visited", progress.visited(), None),
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

    pub fn frame(&mut self, document: &DocumentPath, frame: &ExchangeFrame) {
        if self.error.is_none() {
            self.framed_documents.insert(document.clone());
            self.observer.frame(document, frame.clone());
        }
    }

    fn has_frames(&self, document: &DocumentPath) -> bool {
        self.framed_documents.contains(document)
    }

    /// Reports the trajectory a document's frames replay, written as paths.
    /// A typed economy and trace serialize as `paths` ones (D-044); one that
    /// does not fails the study, at its next check.
    fn trajectory<O: WrittenOntology>(
        &mut self,
        problem: &str,
        document: &DocumentPath,
        initial: &Economy<O>,
        trace: &Trace<O>,
    ) {
        if self.error.is_some() {
            return;
        }
        match written::<_, paths::Economy>(initial)
            .and_then(|initial| Ok((initial, written::<_, paths::Trace>(trace)?)))
        {
            Ok((initial, trace)) => self.observer.trajectory(document, &initial, &trace),
            Err(error) => {
                self.error = Some(problem_error(
                    problem,
                    format!("document `{document}` is not written as paths: {error}"),
                ));
            }
        }
    }
}

/// `value` written down and read back as `T`.
fn written<V: Serialize, T: DeserializeOwned>(value: &V) -> Result<T, serde_json::Error> {
    serde_json::from_value(serde_json::to_value(value)?)
}

fn observation_kind(phase: &PhasePath) -> SearchObservationKindView {
    let phase = phase.as_str();
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

/// The document of a trace replayed from `initial`. Its trajectory is
/// reported to the study's observer.
pub(super) fn document<O: WrittenOntology>(
    progress: &mut ProgressSink<'_>,
    spec: DocumentSpec<'_>,
    initial: &Economy<O>,
    goal: &Goal<O>,
    trace: &Trace<O>,
    objectives: Vec<ObjectiveView>,
    scene: StudioScene<O>,
) -> Result<ViewDocument, PlaybackError>
where
    O::Account: StudioLabel,
    O::Asset: StudioLabel + StudioFormat,
    O::Rate: StudioLabel,
    O::Role: StudioLabel,
{
    let ontology = StudioOntology::<O>::new(Some(scene));
    let mut document = derive_document(spec.metadata(), initial, trace, &ontology, objectives)?;
    document.model = Some(derive_model(initial, goal, &ontology));
    progress.trajectory(spec.problem, &document.id, initial, trace);
    Ok(document)
}

/// The document of a trace replayed from `initial`, with leaderboards; its
/// frames stream to the study's observer as they are derived when `streamed`.
/// Its trajectory is reported to the observer.
#[allow(clippy::too_many_arguments)]
pub(super) fn document_with_leaderboards_observed<O: WrittenOntology>(
    progress: &mut ProgressSink<'_>,
    spec: DocumentSpec<'_>,
    initial: &Economy<O>,
    goal: &Goal<O>,
    trace: &Trace<O>,
    objectives: Vec<ObjectiveView>,
    projections: StudioProjections<O>,
    streamed: bool,
) -> Result<ViewDocument, PlaybackError>
where
    O::Account: StudioLabel,
    O::Asset: StudioLabel + StudioFormat,
    O::Rate: StudioLabel,
    O::Role: StudioLabel,
{
    let (scene, leaderboards) = projections;
    let ontology = StudioOntology::<O>::new(Some(scene)).with_leaderboards(leaderboards);
    let metadata = spec.metadata();
    let id = metadata.id.clone();
    let mut document = derive_document_with_frames(
        metadata,
        initial,
        trace,
        &ontology,
        objectives,
        |frame: &ExchangeFrame| {
            if streamed {
                progress.frame(&id, frame);
            }
        },
    )?;
    document.model = Some(derive_model(initial, goal, &ontology));
    progress.trajectory(spec.problem, &id, initial, trace);
    Ok(document)
}

impl DocumentSpec<'_> {
    fn metadata(&self) -> ViewDocumentMetadata {
        ViewDocumentMetadata {
            id: slug(self.strategy),
            title: self.title.into(),
            description: self.description.into(),
            source: ViewSource {
                key: slug(self.problem),
                label: self.source_label.into(),
            },
        }
    }
}

pub(super) struct ProposalSpec<'a> {
    pub id: &'a str,
    pub label: &'a str,
    pub description: &'a str,
}

/// A proposal assessed against the document's initial state.
pub(super) fn proposal<O: WrittenOntology>(
    spec: ProposalSpec<'_>,
    economy: &Economy<O>,
    exchange: &axionomy::Exchange<O>,
) -> ProposalView
where
    O::Account: StudioLabel,
    O::Asset: StudioLabel + StudioFormat,
    O::Rate: StudioLabel,
    O::Role: StudioLabel,
{
    proposal_at(spec, 0, economy, exchange)
}

/// A proposal assessed against `economy`, the document's snapshot
/// `snapshot_index`: zero is the initial state, and frame `i` leaves
/// snapshot `i + 1`.
pub(super) fn proposal_at<O: WrittenOntology>(
    spec: ProposalSpec<'_>,
    snapshot_index: u64,
    economy: &Economy<O>,
    exchange: &axionomy::Exchange<O>,
) -> ProposalView
where
    O::Account: StudioLabel,
    O::Asset: StudioLabel + StudioFormat,
    O::Rate: StudioLabel,
    O::Role: StudioLabel,
{
    let ontology = StudioOntology::<O>::new(None);
    derive_proposal(
        slug(spec.id),
        spec.label,
        spec.description,
        snapshot_index,
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

pub(super) fn assemble(
    study: &StudyKey,
    descriptor: &ProblemDescriptor,
    mut documents: Vec<ViewDocument>,
) -> Result<Study, ServiceError> {
    let instance = descriptor
        .instances
        .iter()
        .find(|instance| instance.profile == study.instance)
        .ok_or_else(|| ServiceError::UnknownInstance {
            problem: study.problem.clone(),
            instance: study.instance,
        })?
        .clone();
    let selected_document: DocumentPath = slug(study.strategy.as_str());
    let alternatives = documents.len() as u64;
    for document in &mut documents {
        for snapshot in std::iter::once(&document.initial)
            .chain(document.frames.iter().map(|frame| &frame.after))
        {
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
        .any(|document| document.id == selected_document)
    {
        return Err(ServiceError::Problem {
            problem: descriptor.key.clone(),
            message: format!("strategy `{}` produced no document", study.strategy),
        });
    }
    Ok(Study {
        key: study.clone(),
        problem: descriptor.clone(),
        instance,
        selected_document,
        documents,
        assessed_proposals: Vec::new(),
    })
}

/// Runs a simulation from a fork of `initial` and gives the trace it left and
/// the state it ended in, or why it did not reach its goal.
pub(super) fn simulate<O: Ontology>(
    problem: &str,
    initial: &Economy<O>,
    run: impl FnOnce(&mut Economy<O>) -> RolloutResult<O>,
) -> Result<(Trace<O>, Economy<O>), ServiceError> {
    let mut world = initial.clone();
    let result = run(&mut world);
    if !result.termination().is_terminal() {
        return Err(problem_error(
            problem,
            format!("did not reach its goal: {:?}", result.termination()),
        ));
    }
    let trace = result
        .trace()
        .cloned()
        .ok_or_else(|| problem_error(problem, "the rollout kept no trace"))?;
    Ok((trace, world))
}

pub(super) fn problem_error(problem: &str, error: impl ToString) -> ServiceError {
    ServiceError::Problem {
        problem: slug(problem),
        message: error.to_string(),
    }
}
