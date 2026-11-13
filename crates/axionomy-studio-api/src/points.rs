//! Points (D-046): a point's history drawn with its view, the exchanges a
//! candidate set offers there, each assessed, an exchange assessed and
//! applied, and a strategy run from it.

use crate::{Natural, PointId, Refusal};
use axionomy::paths;
use axionomy_engine::{CandidatesPath, Effort, Engine, Outcome, Ran, StrategyPath};
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

/// Exchanges applied: the point they started from, which never changes,
/// and the new point they reach, a step on for each.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Applied {
    pub from: PointId,
    pub point: PointId,
}

/// A strategy to run from a point, with the seed of its randomness and its
/// budget, in expansions for a search.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StrategyRequest {
    pub strategy: StrategyPath,
    pub seed: Natural,
    pub budget: Natural,
}

/// A strategy's run from a point: where it ended, how, and what its search
/// explored on the way.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct RanView {
    pub strategy: StrategyPath,
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
    /// The policy used its every turn.
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
    /// The search's progress so far.
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

/// Checks that `point` and the strategy `request` names exist, so a
/// transport can refuse at once before it runs anything.
pub fn check_strategy(
    engine: &Engine,
    point: PointId,
    request: &StrategyRequest,
) -> Result<(), Refusal> {
    engine.reach(point.point())?;
    engine.strategy(&request.strategy)?;
    Ok(())
}

/// Runs the strategy `request` names from `point`; `watch` sees its search's
/// progress and may stop it.
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
    };
    let Ran {
        start,
        end,
        outcome,
        thinking,
        cost,
    } = engine.run_strategy(point.point(), &strategy, effort, watch)?;
    Ok(RanView {
        strategy: request.strategy,
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
