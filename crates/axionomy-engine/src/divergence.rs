//! Where two histories part and how they differ after. Two points' histories
//! share every step up to the last point on both, and none after it: from
//! there each side applies its own exchanges, and the states they reach are
//! compared step by step, by their trees, so a step costs what changed and
//! not the size of the economy.

use crate::{Engine, EngineError, ObjectivePath, State, run_at};
use axionomy::{
    AccountPath, AssetPath, Quantity,
    paths::{self, Paths},
};
use axionomy_store::{Point, RunId, StoreError, state};
use std::collections::HashMap;

/// One of the two histories compared.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
}

/// A balance that differs between the two sides' states after a step: what
/// the account holds of the asset on each, exactly, zero where it holds none.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BalanceDifference {
    pub account: AccountPath,
    pub asset: AssetPath,
    pub left: Quantity,
    pub right: Quantity,
}

/// An account that exists after a step on one side only: `on` is the side
/// whose state has it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountDifference {
    pub account: AccountPath,
    pub on: Side,
}

/// One step after the shared point, on both histories: the exchange each
/// side applied, none once that side's history has ended, and how the states
/// the two sides are in after it differ. A side whose history has ended stays
/// in its last state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Aligned {
    pub step: u64,
    pub left: Option<paths::Exchange>,
    pub right: Option<paths::Exchange>,
    /// Every balance that differs, in the order of the accounts' paths, then
    /// of the assets'.
    pub balances: Vec<BalanceDifference>,
    /// Every account one side has and the other does not, in path order.
    pub accounts: Vec<AccountDifference>,
}

/// The end of one of the histories compared, and the objectives there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tip {
    pub point: Point,
    /// The objectives asked for, measured at `point`, in the order asked.
    pub values: Vec<Quantity>,
}

/// How two histories compare: where they part, and the steps after it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Divergence {
    /// The last point on both histories: every step up to it is the same on
    /// both, and it is the state both sides start from.
    pub shared: Point,
    pub left: Tip,
    pub right: Tip,
    /// The steps after `shared` in the window asked for, earliest first.
    pub steps: Vec<Aligned>,
    /// The first step after `shared` after which the two sides are in the
    /// same state again, wherever it is on the histories, the window or not.
    pub converged: Option<u64>,
}

impl Divergence {
    /// The last step either history reaches: steps after the last one listed
    /// follow, up to this one.
    pub fn last(&self) -> u64 {
        self.left.point.step.max(self.right.point.step)
    }
}

impl Engine {
    /// How the histories of `left` and `right` compare: the last point on
    /// both; for each step after it and after `after`, at most `limit` of
    /// them, the exchange each side applied and the balances and accounts
    /// that differ between the states the two sides are in after it; the
    /// first step after which those states are equal again; and
    /// `objectives` measured at both points. Refused with `Unrelated` when
    /// no point is on both histories, as for two economies authored apart.
    pub fn divergence(
        &self,
        left: Point,
        right: Point,
        after: Option<u64>,
        limit: usize,
        objectives: &[ObjectivePath],
    ) -> Result<Divergence, EngineError> {
        let shared = self.shared(left, right)?;
        let tip = |point: Point| -> Result<Tip, EngineError> {
            let values = objectives
                .iter()
                .map(|objective| self.measure(point, objective))
                .collect::<Result<_, _>>()?;
            Ok(Tip { point, values })
        };
        let (left_tip, right_tip) = (tip(left)?, tip(right)?);
        let first = after.map_or(shared.step, |after| after.max(shared.step));
        let mut lefts = self.exchanges_after(left, shared.step)?.into_iter();
        let mut rights = self.exchanges_after(right, shared.step)?.into_iter();
        let start = self.reach(shared)?.state.clone();
        let mut sides = (start.clone(), start);
        let mut steps = Vec::new();
        let mut converged = None;
        for step in shared.step + 1..=left.step.max(right.step) {
            let exchanges = (lefts.next(), rights.next());
            for (state, exchange) in [(&mut sides.0, &exchanges.0), (&mut sides.1, &exchanges.1)] {
                if let Some(exchange) = exchange {
                    state.apply(exchange.clone()).map_err(|_| {
                        EngineError::Store(StoreError::Malformed("a history that does not replay"))
                    })?;
                }
            }
            if converged.is_none() && sides.0.root() == sides.1.root() {
                converged = Some(step);
            }
            if step > first && steps.len() < limit {
                let (balances, accounts) = differences(&sides.0, &sides.1)?;
                steps.push(Aligned {
                    step,
                    left: exchanges.0,
                    right: exchanges.1,
                    balances,
                    accounts,
                });
            } else if converged.is_some() && step > first {
                // The window is full, and where the sides meet again is known.
                break;
            }
        }
        Ok(Divergence {
            shared,
            left: left_tip,
            right: right_tip,
            steps,
            converged,
        })
    }

    /// The last point on the histories of both `left` and `right`, where
    /// they part; refused with `Unrelated` when there is none. A history
    /// runs back through the runs it forked from, each to the step it forked
    /// at; two histories share a run's steps up to the earlier of the steps
    /// each takes in it, and the first run of `right`'s, counting back from
    /// its tip, that `left`'s history also runs through is the one they part
    /// in.
    pub fn shared(&self, left: Point, right: Point) -> Result<Point, EngineError> {
        let reader = self.store.read()?;
        run_at(&reader, left)?;
        run_at(&reader, right)?;
        let runs = |at: Point| -> Result<Vec<(RunId, u64)>, EngineError> {
            let mut runs = Vec::new();
            let mut on = at;
            loop {
                runs.push((on.run, on.step));
                let run = reader.run(on.run)?.ok_or(EngineError::UnknownPoint(on))?;
                match run.from {
                    Some(from) => on = from,
                    None => return Ok(runs),
                }
            }
        };
        let lefts: HashMap<RunId, u64> = runs(left)?.into_iter().collect();
        for (run, step) in runs(right)? {
            if let Some(other) = lefts.get(&run) {
                return Ok(Point {
                    run,
                    step: step.min(*other),
                });
            }
        }
        Err(EngineError::Unrelated { left, right })
    }

    /// The exchanges of `at`'s history after step `after`, in order: those
    /// of its own run, then back through the runs it forked from, no
    /// further than `after`.
    fn exchanges_after(&self, at: Point, after: u64) -> Result<Vec<paths::Exchange>, EngineError> {
        let reader = self.store.read()?;
        let mut segments = Vec::new();
        let mut on = at;
        while on.step > after {
            let run = reader
                .run_of::<Paths>(on.run)?
                .ok_or(EngineError::UnknownPoint(on))?;
            segments.push((on.run, run.first.max(after), on.step));
            match run.from {
                Some(from) => on = from,
                None => break,
            }
        }
        let mut exchanges = Vec::new();
        for (run, first, last) in segments.into_iter().rev() {
            let count = usize::try_from(last - first).unwrap_or(usize::MAX);
            for (_, step) in reader.steps::<Paths>(run, first + 1, count)? {
                exchanges.push(step.exchange);
            }
        }
        Ok(exchanges)
    }
}

/// How the state on the left differs from the state on the right: the keys
/// their trees differ in, read as balances and accounts, the subtrees they
/// share skipped. Two states of one history's descendants have the same
/// rates and invariants, which exchanges never change.
fn differences(
    left: &State,
    right: &State,
) -> Result<(Vec<BalanceDifference>, Vec<AccountDifference>), EngineError> {
    let mut balances = Vec::new();
    let mut accounts = Vec::new();
    for change in state::changes::<Paths>(&left.tree, &right.tree).map_err(StoreError::from)? {
        match change {
            state::Change::Balance {
                account,
                asset,
                before,
                after,
            } => balances.push(BalanceDifference {
                account,
                asset,
                left: before,
                right: after,
            }),
            state::Change::Opened(account) => accounts.push(AccountDifference {
                account,
                on: Side::Right,
            }),
            state::Change::Closed(account) => accounts.push(AccountDifference {
                account,
                on: Side::Left,
            }),
            state::Change::Rate(_) | state::Change::Invariants => {}
        }
    }
    Ok((balances, accounts))
}
