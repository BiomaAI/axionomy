//! Two points' histories compared: where they part, the exchange each side
//! applied at every step after, the balances and accounts the two states
//! differ in, where they meet again, and objectives at both ends.

use crate::{ObjectiveList, ObjectiveView, PointId, Refusal, packages::objectives};
use axionomy::{AccountPath, AssetPath, Quantity, paths};
use axionomy_engine::{
    AccountDifference, Aligned, BalanceDifference, Divergence, Engine, Side, Tip,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Steps one request returns at most.
pub const MAX_STEPS: u64 = 1_000;

/// Two points to compare, the window of steps after the point both
/// histories share, and the objectives to measure at both.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DivergenceQuery {
    pub left: PointId,
    pub right: PointId,
    /// The step the window starts after: the last step a reader handled,
    /// such as the last one listed. None, or a step before the shared
    /// point, starts right after the shared point.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<u64>,
    /// The most steps to return, from 1 to 1000.
    #[serde(default = "default_limit")]
    #[schemars(range(min = 1, max = 1_000))]
    pub limit: u64,
    /// Objectives the store keeps, measured at both points in this order.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub objectives: Option<ObjectiveList>,
}

const fn default_limit() -> u64 {
    100
}

/// How two histories compare: the last point on both, the steps after it in
/// the window asked for, where the two states are equal again, and each
/// end's objectives.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DivergenceView {
    /// The last point on both histories: every step up to it is the same on
    /// both, and both sides start from its state.
    pub shared: PointId,
    pub left: TipView,
    pub right: TipView,
    /// The objectives each end is measured on, in the order asked.
    pub objectives: Vec<ObjectiveView>,
    /// The steps after `shared` in the window, earliest first.
    pub steps: Vec<AlignedView>,
    /// The first step after `shared` after which the two states are equal
    /// again, wherever it is on the histories, in the window or not.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub converged: Option<u64>,
    /// The `after` that asks for the steps following these, while more
    /// follow; none once the window reached the last step either history
    /// takes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next: Option<u64>,
}

/// One end of the histories compared, with the objectives measured there.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TipView {
    pub point: PointId,
    /// The objectives, exactly, in the order asked.
    pub values: Vec<Quantity>,
}

/// One step after the shared point on both histories: the exchange each
/// side applied, absent once that side's history has ended and its state
/// stays where it ended, and how the two states differ after it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AlignedView {
    /// The step, counted from the histories' first state, which is step 0.
    pub step: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub left: Option<paths::Exchange>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub right: Option<paths::Exchange>,
    /// Every balance that differs, in the order of the accounts' paths,
    /// then of the assets'.
    pub balances: Vec<BalanceDifferenceView>,
    /// Every account one state has and the other does not, in path order.
    pub accounts: Vec<AccountDifferenceView>,
}

/// A balance that differs between the two states: what the account holds
/// of the asset on each side, exactly, zero where it holds none.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct BalanceDifferenceView {
    pub account: AccountPath,
    pub asset: AssetPath,
    pub left: Quantity,
    pub right: Quantity,
}

/// An account one state has and the other does not.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AccountDifferenceView {
    pub account: AccountPath,
    /// The side whose state has the account.
    pub on: SideView,
}

/// One of the two histories compared.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SideView {
    Left,
    Right,
}

impl From<Side> for SideView {
    fn from(side: Side) -> Self {
        match side {
            Side::Left => Self::Left,
            Side::Right => Self::Right,
        }
    }
}

impl From<Tip> for TipView {
    fn from(tip: Tip) -> Self {
        Self {
            point: tip.point.into(),
            values: tip.values,
        }
    }
}

impl From<Aligned> for AlignedView {
    fn from(aligned: Aligned) -> Self {
        Self {
            step: aligned.step,
            left: aligned.left,
            right: aligned.right,
            balances: aligned
                .balances
                .into_iter()
                .map(
                    |BalanceDifference {
                         account,
                         asset,
                         left,
                         right,
                     }| BalanceDifferenceView {
                        account,
                        asset,
                        left,
                        right,
                    },
                )
                .collect(),
            accounts: aligned
                .accounts
                .into_iter()
                .map(|AccountDifference { account, on }| AccountDifferenceView {
                    account,
                    on: on.into(),
                })
                .collect(),
        }
    }
}

/// How the histories of `query.left` and `query.right` compare, over the
/// window of steps it asks for.
pub fn divergence(engine: &Engine, query: DivergenceQuery) -> Result<DivergenceView, Refusal> {
    if !(1..=MAX_STEPS).contains(&query.limit) {
        return Err(Refusal::BadRequest(format!(
            "step limit must be between 1 and {MAX_STEPS}; page on from `next`"
        )));
    }
    let asked = query.objectives.unwrap_or_default().0;
    let limit = usize::try_from(query.limit).unwrap_or(usize::MAX);
    let compared = engine.divergence(
        query.left.point(),
        query.right.point(),
        query.after,
        limit,
        &asked,
    )?;
    let last = compared.last();
    let Divergence {
        shared,
        left,
        right,
        steps,
        converged,
    } = compared;
    let next = steps
        .last()
        .map(|step| step.step)
        .filter(|&listed| listed < last);
    Ok(DivergenceView {
        shared: shared.into(),
        left: left.into(),
        right: right.into(),
        objectives: objectives(engine, &asked)?,
        steps: steps.into_iter().map(AlignedView::from).collect(),
        converged,
        next,
    })
}
