//! The history tree, and what changes it. Every run is a segment of some
//! history: a root run starts from an economy of its own, and a fork, a
//! branch, continues a history from the point it forked at.
//! [`Engine::histories`] lists the segments growing from the roots of some
//! points' histories, so a front end can draw them as one tree;
//! [`Engine::subscribe`] tells it of every run recorded, ref moved and
//! definition kept afterwards, so the tree it drew stays the store's.

use crate::{Engine, EngineError};
use axionomy_store::{Point, RefPath, RunId};
use std::{
    collections::{BTreeSet, VecDeque},
    sync::{Arc, Mutex},
};

/// The steps one run took: from the point it forked at, or from its own
/// first state when `from` is none, up to `tip`, the run's last point. The
/// run is `tip.run`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Segment {
    pub from: Option<Point>,
    pub tip: Point,
}

/// Something the store holds now that it did not before.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    /// A run was recorded: a new segment of some history.
    Run(Segment),
    /// The ref `name` was made, moved or deleted: it names `point` now, or
    /// nothing.
    Ref { name: RefPath, point: Option<Point> },
    /// Definitions were kept: a package loaded, a script, an experiment's
    /// results. What reads them reads them again.
    Definitions,
}

/// Told of every [`Change`], after the store holds it, on the thread that
/// made it. It is told while that work waits, so it hands the change on
/// (to a channel, a message) rather than calling back into the engine.
pub type Subscriber = Arc<dyn Fn(&Change) + Send + Sync>;

/// The subscribers an engine tells.
#[derive(Default)]
pub(crate) struct Changes {
    subscribers: Mutex<Vec<Subscriber>>,
}

impl Changes {
    pub(crate) fn tell(&self, change: &Change) {
        // Told outside the lock, so a subscriber may subscribe another.
        let subscribers = self
            .subscribers
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone();
        for subscriber in subscribers {
            subscriber(change);
        }
    }
}

impl Engine {
    /// Tells `subscriber` of every change this engine makes from now on.
    pub fn subscribe(&self, subscriber: Subscriber) {
        self.changes
            .subscribers
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(subscriber);
    }

    /// The history tree of `points`: every run growing from the roots of
    /// their histories, each root, then every run forked from a run already
    /// listed, parents before children and forks in the order of the step
    /// they forked at.
    pub fn histories(&self, points: &[Point]) -> Result<Vec<Segment>, EngineError> {
        let reader = self.store().read()?;
        let mut roots = BTreeSet::new();
        for point in points {
            let mut run = point.run;
            loop {
                let record = reader.run(run)?.ok_or(EngineError::UnknownPoint(*point))?;
                match record.from {
                    Some(from) => run = from.run,
                    None => break,
                }
            }
            roots.insert(run);
        }
        let mut segments = Vec::new();
        let mut queue: VecDeque<(Option<Point>, RunId)> =
            roots.into_iter().map(|run| (None, run)).collect();
        while let Some((from, run)) = queue.pop_front() {
            let last = reader
                .last_step(run)?
                .ok_or(EngineError::UnknownPoint(Point { run, step: 0 }))?;
            segments.push(Segment {
                from,
                tip: Point { run, step: last },
            });
            queue.extend(
                reader
                    .branches(run)?
                    .into_iter()
                    .map(|(step, child)| (Some(Point { run, step }), child)),
            );
        }
        Ok(segments)
    }
}
