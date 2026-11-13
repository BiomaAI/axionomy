//! Points (D-046): a point's history drawn with its view, the exchanges a
//! candidate set offers there, each assessed, an exchange assessed and
//! applied, and a strategy run from it.

use crate::{Natural, PointId, Refusal};
use axionomy::{AccountPath, paths};
use axionomy_engine::{CandidatesPath, Effort, Engine, GoalPath, Outcome, Ran, StrategyPath};
use axionomy_search::GraphSearchProgress;
use axionomy_view::{ProposalView, ViewDocument};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::ops::ControlFlow;

/// The point at an earlier step of a point's history.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AncestorQuery {
    /// The step, from the history's first state, which is step 0.
    pub step: u64,
}

/// A point of a history.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Ancestor {
    pub point: PointId,
}

/// Which candidate set to ask.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CandidatesQuery {
    pub set: CandidatesPath,
}

/// The exchanges a candidate set offers at a point, each assessed there.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Candidates {
    pub point: PointId,
    pub set: CandidatesPath,
    pub candidates: Vec<Candidate>,
}

/// An exchange at a point: as it is applied, and as people read it with
/// the point's view, assessed against the economy there.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Candidate {
    pub exchange: paths::Exchange,
    pub proposal: ProposalView,
}

/// An exchange to assess at a point; nothing is recorded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AssessRequest {
    pub exchange: paths::Exchange,
}

/// Exchanges to apply at a point, all of them or none.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ApplyRequest {
    pub exchanges: Vec<paths::Exchange>,
}

/// A point's accounts, each with what it holds there.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Balances {
    pub point: PointId,
    /// Every account at the point, in the order of their paths.
    pub accounts: Vec<AccountBalances>,
}

/// An account and what it holds: each asset with its exact quantity, in
/// the order of the assets' paths, as an exchange's receipt writes a basket.
/// An asset it holds none of is not listed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AccountBalances {
    pub account: AccountPath,
    pub balances: paths::Basket,
}

/// Exchanges applied: the point they started from, which never changes,
/// and the new point they reach, a step on for each.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Applied {
    pub from: PointId,
    pub point: PointId,
}

/// A strategy to run from a point toward a goal, with what it may spend:
/// the seed of its randomness, a budget of expansions for a search, and the
/// most turns a policy or a game plays. The strategy is how to choose; the
/// point, the goal and the effort are the question it is asked, and it
/// reads only the limit it spends.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StrategyRequest {
    pub strategy: StrategyPath,
    /// A goal the store keeps, such as one of the package's goals.
    pub goal: GoalPath,
    pub seed: Natural,
    /// The most states a search expands.
    pub budget: Natural,
    /// The most turns a policy or a game plays.
    pub turns: Natural,
}

/// A strategy's run from a point: where it ended, how, and what its search
/// explored on the way.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct RanView {
    pub strategy: StrategyPath,
    /// The goal it worked toward.
    pub goal: GoalPath,
    pub start: PointId,
    /// Where the run ended: a new point a run forked from `start` reaches,
    /// or `start` itself when it applied nothing.
    pub end: PointId,
    pub outcome: OutcomeView,
    /// What the search expanded and generated; none for a policy or a plan.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thinking: Option<GraphSearchProgress>,
    /// The total cost of a cheapest search's exchanges.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost: Option<Natural>,
}

/// How a strategy's run ended.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum OutcomeView {
    /// The goal holds where the run ends.
    Reached,
    /// The policy proposed nothing, or a plan ended short of the goal.
    Stopped,
    /// The policy or the game played every turn it was given.
    Horizon,
    /// The policy proposed an exchange that does not apply, on turn `turn`
    /// counting from zero; the run ends before it.
    Refused {
        turn: u64,
        assessment: Box<paths::Assessment>,
    },
    /// The search explored everything its candidates reach without the goal.
    Exhausted,
    /// The search spent its budget first.
    Spent,
    /// The run was stopped before it ended.
    Interrupted,
}

impl From<Outcome> for OutcomeView {
    fn from(outcome: Outcome) -> Self {
        match outcome {
            Outcome::Reached => Self::Reached,
            Outcome::Stopped => Self::Stopped,
            Outcome::Horizon => Self::Horizon,
            Outcome::Refused { turn, assessment } => Self::Refused { turn, assessment },
            Outcome::Exhausted => Self::Exhausted,
            Outcome::Spent => Self::Spent,
            Outcome::Interrupted => Self::Interrupted,
        }
    }
}

/// What a strategy reports while it runs, then where it ended or why it
/// could not.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum StrategyEvent {
    /// How far the run is: its search's progress, or its turns counted as a
    /// search counts expansions.
    Progress { progress: GraphSearchProgress },
    /// The run ended.
    Ran { ran: RanView },
    /// The strategy could not run, as the status says.
    Failed { status: u16, error: String },
}

impl From<Result<RanView, Refusal>> for StrategyEvent {
    fn from(ran: Result<RanView, Refusal>) -> Self {
        match ran {
            Ok(ran) => Self::Ran { ran },
            Err(refusal) => Self::Failed {
                status: refusal.status(),
                error: refusal.message().to_owned(),
            },
        }
    }
}

/// A point's history drawn as a document with the view of its run's
/// ontology.
pub fn draw(engine: &Engine, point: PointId) -> Result<ViewDocument, Refusal> {
    Ok(engine.draw(point.point())?)
}

/// The point at step `query.step` of `point`'s history.
pub fn ancestor(
    engine: &Engine,
    point: PointId,
    query: AncestorQuery,
) -> Result<Ancestor, Refusal> {
    Ok(Ancestor {
        point: engine.ancestor(point.point(), query.step)?.into(),
    })
}

/// What the candidate set `query.set` offers at `point`, each exchange
/// assessed there.
pub fn candidates(
    engine: &Engine,
    point: PointId,
    query: CandidatesQuery,
) -> Result<Candidates, Refusal> {
    let exchanges = engine.candidates(point.point(), &query.set)?;
    Ok(Candidates {
        point,
        candidates: presented(engine, point, exchanges)?,
        set: query.set,
    })
}

/// Every account at `point` and what it holds there.
pub fn balances(engine: &Engine, point: PointId) -> Result<Balances, Refusal> {
    let reached = engine.reach(point.point())?;
    let accounts = reached
        .state
        .economy
        .accounts()
        .map(|(account, held)| AccountBalances {
            account: account.clone(),
            balances: held
                .balances()
                .iter_sorted()
                .map(|(asset, quantity)| (asset.clone(), *quantity))
                .collect(),
        })
        .collect();
    Ok(Balances { point, accounts })
}

/// An exchange assessed at `point`: what it would move, or what it lacks.
pub fn assess(
    engine: &Engine,
    point: PointId,
    request: AssessRequest,
) -> Result<Candidate, Refusal> {
    let mut presented = presented(engine, point, vec![request.exchange])?;
    Ok(presented.remove(0))
}

/// Exchanges applied at `point`, all of them or none, recorded as a run
/// forked from it.
pub fn apply(engine: &Engine, point: PointId, request: ApplyRequest) -> Result<Applied, Refusal> {
    if request.exchanges.is_empty() {
        return Err(Refusal::BadRequest(
            "apply at least one exchange; applying none reaches the point itself".into(),
        ));
    }
    let reached = engine.apply(point.point(), &request.exchanges)?;
    Ok(Applied {
        from: point,
        point: reached.into(),
    })
}

/// Checks that `point` and the strategy and goal `request` names exist, so
/// a transport can refuse at once before it runs anything.
pub fn check_strategy(
    engine: &Engine,
    point: PointId,
    request: &StrategyRequest,
) -> Result<(), Refusal> {
    engine.reach(point.point())?;
    engine.strategy(&request.strategy)?;
    engine.goal(&request.goal)?;
    Ok(())
}

/// Runs the strategy `request` names from `point` toward its goal; `watch`
/// sees how far it is and may stop it, whatever its kind.
pub fn run_strategy(
    engine: &Engine,
    point: PointId,
    request: StrategyRequest,
    watch: &mut impl FnMut(&GraphSearchProgress) -> ControlFlow<()>,
) -> Result<RanView, Refusal> {
    let strategy = engine.strategy(&request.strategy)?;
    let effort = Effort {
        seed: request.seed.0,
        budget: request.budget.0,
        turns: request.turns.0,
    };
    let Ran {
        start,
        end,
        outcome,
        thinking,
        cost,
    } = engine.run_strategy(point.point(), &strategy, &request.goal, effort, watch)?;
    Ok(RanView {
        strategy: request.strategy,
        goal: request.goal,
        start: start.into(),
        end: end.into(),
        outcome: outcome.into(),
        thinking,
        cost: cost.map(Natural),
    })
}

fn presented(
    engine: &Engine,
    point: PointId,
    exchanges: Vec<paths::Exchange>,
) -> Result<Vec<Candidate>, Refusal> {
    let proposals = engine.present(point.point(), &exchanges)?;
    Ok(exchanges
        .into_iter()
        .zip(proposals)
        .map(|(exchange, proposal)| Candidate { exchange, proposal })
        .collect())
}
