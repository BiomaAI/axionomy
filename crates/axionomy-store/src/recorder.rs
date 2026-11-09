//! Recording a run as it happens.

use crate::{
    Durability, MetricPath, Pending, Point, RunId, State, Store, StoreError, TagPath,
    ontology::Recordable, records::Checkpoints,
};
use axionomy::{ApplyError, Exchange, Receipt};

/// Records a run step by step. Each step is a kernel `apply` and an update of
/// the state's tree in memory; steps reach the writer in batches and never
/// wait on disk. [`commit`](Self::commit) and [`finish`](Self::finish) wait
/// until everything recorded so far is durable.
pub struct Recorder<O: Recordable> {
    store: Store,
    run: RunId,
    step: u64,
    checkpoints: Checkpoints,
    state: State<O>,
    ops: Vec<Op<O>>,
    in_flight: Vec<Pending<()>>,
    /// The first write that failed, reported by the next commit.
    failed: Option<StoreError>,
}

enum Op<O: Recordable> {
    Run {
        from: Option<Point>,
        state: axionomy_tree::Tree,
    },
    Step {
        step: u64,
        receipt: Receipt<O>,
        state: Option<axionomy_tree::Tree>,
    },
    Pin {
        step: u64,
        state: axionomy_tree::Tree,
    },
    Tag {
        key: TagPath,
        value: Option<String>,
    },
    Metric {
        step: Option<u64>,
        name: MetricPath,
        value: f64,
    },
    Finish {
        step: u64,
        state: Option<axionomy_tree::Tree>,
    },
}

/// Steps recorded before they are sent to the writer.
const BATCH: usize = 256;

impl<O: Recordable> Recorder<O> {
    pub(crate) fn new(
        store: Store,
        from: Option<Point>,
        state: State<O>,
        checkpoints: Checkpoints,
    ) -> Self {
        let run = store.mint_run();
        let ops = vec![Op::Run {
            from,
            state: state.tree.clone(),
        }];
        Self {
            store,
            run,
            step: from.map_or(0, |from| from.step),
            checkpoints,
            state,
            ops,
            in_flight: Vec::new(),
            failed: None,
        }
    }

    pub fn run(&self) -> RunId {
        self.run
    }

    /// The point the run has reached.
    pub fn point(&self) -> Point {
        Point {
            run: self.run,
            step: self.step,
        }
    }

    pub fn state(&self) -> &State<O> {
        &self.state
    }

    /// Applies an exchange and records it. A failed exchange changes nothing
    /// and records nothing.
    pub fn apply(&mut self, exchange: Exchange<O>) -> Result<Receipt<O>, ApplyError<O>> {
        let receipt = self.state.apply(exchange)?;
        self.step += 1;
        self.ops.push(Op::Step {
            step: self.step,
            receipt: receipt.clone(),
            state: self
                .checkpoints
                .keeps(self.step)
                .then(|| self.state.tree.clone()),
        });
        if self.ops.len() >= BATCH {
            self.send(Durability::Eventual);
        }
        Ok(receipt)
    }

    /// Stores the current state and keeps it from garbage collection.
    pub fn pin(&mut self) {
        self.ops.push(Op::Pin {
            step: self.step,
            state: self.state.tree.clone(),
        });
    }

    /// Sets a tag on the run, or removes it with `None`.
    pub fn tag(&mut self, key: TagPath, value: Option<String>) {
        self.ops.push(Op::Tag { key, value });
    }

    /// Records a metric of the run, or with `at_step`, of its current step.
    pub fn metric(
        &mut self,
        name: MetricPath,
        value: f64,
        at_step: bool,
    ) -> Result<(), StoreError> {
        if !value.is_finite() {
            return Err(StoreError::NotFinite(name));
        }
        self.ops.push(Op::Metric {
            step: at_step.then_some(self.step),
            name,
            value,
        });
        Ok(())
    }

    /// Waits until everything recorded so far is durable.
    pub fn commit(&mut self) -> Result<Point, StoreError> {
        self.send(Durability::Immediate);
        for pending in self.in_flight.drain(..) {
            if let Err(error) = pending.wait() {
                self.failed.get_or_insert(error);
            }
        }
        match self.failed.take() {
            Some(error) => Err(error),
            None => Ok(self.point()),
        }
    }

    /// Ends the run, storing its last state as its checkpoints require, and
    /// waits until everything is durable.
    pub fn finish(mut self) -> Result<Point, StoreError> {
        let stored = self.checkpoints.keeps(self.step);
        self.ops.push(Op::Finish {
            step: self.step,
            state: (!stored).then(|| self.state.tree.clone()),
        });
        self.commit()
    }

    fn send(&mut self, durability: Durability) {
        let ops = std::mem::take(&mut self.ops);
        let run = self.run;
        let checkpoints = self.checkpoints;
        let pending = self.store.transact(durability, move |txn| {
            for op in ops {
                match op {
                    Op::Run { from, state } => {
                        let first = from.map_or(0, |from| from.step);
                        txn.create_run::<O>(run, from, first, &state, checkpoints)?;
                    }
                    Op::Step {
                        step,
                        receipt,
                        state,
                    } => txn.put_step::<O>(Point { run, step }, &receipt, state.as_ref())?,
                    Op::Pin { step, state } => txn.pin(Point { run, step }, Some(&state))?,
                    Op::Tag { key, value } => txn.tag(run, &key, value.as_deref())?,
                    Op::Metric { step, name, value } => txn.metric(run, step, &name, value)?,
                    Op::Finish { step, state } => {
                        txn.finish(Point { run, step }, state.as_ref())?
                    }
                }
            }
            Ok(())
        });
        // Keep only writes whose outcome is still unknown; a commit reports
        // the first failure.
        let failed = &mut self.failed;
        self.in_flight.retain(|pending| match pending.try_take() {
            None => true,
            Some(Ok(())) => false,
            Some(Err(error)) => {
                failed.get_or_insert(error);
                false
            }
        });
        self.in_flight.push(pending);
    }
}

impl<O: Recordable> Drop for Recorder<O> {
    /// Sends what was recorded but not yet sent; it becomes durable with the
    /// writer's next durable commit.
    fn drop(&mut self) {
        if !self.ops.is_empty() {
            self.send(Durability::Eventual);
        }
    }
}
