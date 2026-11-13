//! Live refs (P10, D-046): a ref names the point an application is at, and
//! moving it forward is what makes a step count. A ref moves only from where
//! the mover expects it, so of two proposers moving it from one point the
//! second looks again; only the steps it moves over are delivered, so a step
//! that was only tried out never reaches whoever follows the ref.

use crate::{Engine, EngineError, GoalPath, ObjectivePath};
use axionomy::{Quantity, paths};
use axionomy_store::{Durability, Point, RefPath, StoreError, Target};
use std::{
    sync::{Condvar, Mutex},
    time::{Duration, Instant},
};

/// One step as delivered to whoever follows a ref: the point it led to,
/// its exchange and receipt, and what the follower measures there.
#[derive(Debug, Clone)]
pub struct Frame {
    /// The point the step led to, by its one name: the point its mover
    /// advanced the ref to, or over, however late the follower asks.
    pub point: Point,
    pub exchange: paths::Exchange,
    pub receipt: paths::Receipt,
    /// The follower's objectives at `point`, in its order.
    pub values: Vec<Quantity>,
    /// Whether the follower's goal holds at `point`, when it follows one.
    pub holds: Option<bool>,
}

/// What a follower measures at each step delivered to it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Following {
    pub objectives: Vec<ObjectivePath>,
    pub goal: Option<GoalPath>,
}

/// How many times refs have moved, for followers waiting on one.
#[derive(Debug, Default)]
pub(crate) struct Moves {
    count: Mutex<u64>,
    moved: Condvar,
}

impl Moves {
    fn tell(&self) {
        *self
            .count
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) += 1;
        self.moved.notify_all();
    }
}

impl Engine {
    /// Names `at` with the ref `name`, which nothing may name yet.
    pub fn refer(&self, name: &RefPath, at: Point) -> Result<(), EngineError> {
        self.reach(at)?;
        self.move_ref(name, None, at)
    }

    /// The point the ref `name` names.
    pub fn referred(&self, name: &RefPath) -> Result<Point, EngineError> {
        match self.store.read()?.reference(name)? {
            Some(Target::Step(point)) => Ok(point),
            _ => Err(EngineError::UnknownRef(name.clone())),
        }
    }

    /// Moves the ref `name` from `expected` forward to `to`, whose history
    /// passes through `expected`: the steps between are what the move
    /// delivers. Refused with where the ref is when it is at neither
    /// `expected` nor `to`.
    ///
    /// A move already made succeeds again: when the ref is at `to`, the move
    /// asked for is the move made, since a ref only moves forward and so
    /// cannot be at `to` by another route. A mover that never heard whether
    /// its move counted asks again.
    pub fn advance(&self, name: &RefPath, expected: Point, to: Point) -> Result<(), EngineError> {
        self.reach(to)?;
        self.reach(expected)?;
        if !self.passes_through(to, expected)? {
            return Err(EngineError::Backward {
                name: name.clone(),
                to,
            });
        }
        match self.move_ref(name, Some(expected), to) {
            Err(EngineError::Moved {
                current: Some(current),
                ..
            }) if current == to => Ok(()),
            moved => moved,
        }
    }

    /// The steps the ref `name` was moved over after `after`, the point a
    /// follower last handled, earliest first and at most `limit` of them,
    /// each measured as `following` asks. `after` must be on the history of
    /// where the ref is. Each step is named as its mover named it, so a
    /// follower that falls behind receives the points the ref was advanced
    /// to.
    pub fn frames(
        &self,
        name: &RefPath,
        after: Point,
        following: &Following,
        limit: usize,
    ) -> Result<Vec<Frame>, EngineError> {
        let current = self.referred(name)?;
        let mut economy = self.reach(after)?.state.economy.clone();
        if !self.passes_through(current, after)? {
            return Err(EngineError::NotOnHistory {
                at: current,
                step: after.step,
            });
        }
        let goal = following
            .goal
            .as_ref()
            .map(|goal| self.goal(goal))
            .transpose()?;
        let (_, exchanges) = self.history(current)?;
        let mut frames = Vec::new();
        for step in after.step + 1..=current.step {
            if frames.len() == limit {
                break;
            }
            let exchange = exchanges[usize::try_from(step - 1).unwrap_or(usize::MAX)].clone();
            let receipt = economy.apply(exchange.clone()).map_err(|_| {
                EngineError::Store(StoreError::Malformed("a history that does not replay"))
            })?;
            let values = following
                .objectives
                .iter()
                .map(|objective| self.measured(&economy, objective, current, step))
                .collect::<Result<_, _>>()?;
            frames.push(Frame {
                point: self.on_history(current, step)?,
                exchange,
                receipt,
                values,
                holds: goal.as_ref().map(|goal| economy.matches(goal)),
            });
        }
        Ok(frames)
    }

    /// How many times this engine has moved a ref, making one included. A
    /// follower that cannot block a thread on [`wait`](Self::wait), as in a
    /// browser's Worker, looks again for its ref whenever this changes.
    pub fn moves(&self) -> u64 {
        *self
            .moved
            .count
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Where the ref `name` is once it is anywhere but `after`, waiting at
    /// most `timeout` for a move; where it is when the time is up.
    pub fn wait(
        &self,
        name: &RefPath,
        after: Point,
        timeout: Duration,
    ) -> Result<Point, EngineError> {
        let until = Instant::now() + timeout;
        let mut count = self
            .moved
            .count
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        loop {
            let current = self.referred(name)?;
            let left = until.saturating_duration_since(Instant::now());
            if current != after || left.is_zero() {
                return Ok(current);
            }
            count = self
                .moved
                .moved
                .wait_timeout(count, left)
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .0;
        }
    }

    /// Moves `name` from `expected`, no ref when `None`, to `to`.
    fn move_ref(
        &self,
        name: &RefPath,
        expected: Option<Point>,
        to: Point,
    ) -> Result<(), EngineError> {
        let named = name.clone();
        // A move refused is what this write gives, not a failed write: the
        // writer commits other callers' writes in the same transaction, and
        // one that fails fails them all, a rival's move among them.
        let moved = self
            .store
            .transact(Durability::Immediate, move |txn| {
                match txn.compare_and_set_ref(
                    &named,
                    expected.map(Target::Step),
                    Some(Target::Step(to)),
                ) {
                    Ok(()) => Ok(None),
                    Err(StoreError::Moved { current, .. }) => Ok(Some(current)),
                    Err(error) => Err(error),
                }
            })
            .wait();
        match moved {
            Ok(None) => {
                self.moved.tell();
                Ok(())
            }
            Ok(Some(current)) => Err(EngineError::Moved {
                name: name.clone(),
                current: match current {
                    Some(Target::Step(point)) => Some(point),
                    _ => None,
                },
            }),
            Err(error) => Err(error.into()),
        }
    }

    /// An objective at a delivered step: of its state, or summed over the
    /// history up to it.
    fn measured(
        &self,
        economy: &paths::Economy,
        objective: &ObjectivePath,
        current: Point,
        step: u64,
    ) -> Result<Quantity, EngineError> {
        match self.measure_economy(objective, economy) {
            Err(EngineError::NeedsHistory(_)) => {
                self.measure(self.on_history(current, step)?, objective)
            }
            measured => measured,
        }
    }

    /// Whether `at`'s history passes through `point`: the point at its step
    /// of that history has its name.
    fn passes_through(&self, at: Point, point: Point) -> Result<bool, EngineError> {
        Ok(point.step <= at.step && self.on_history(at, point.step)? == point)
    }
}
