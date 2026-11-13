//! Reading one consistent committed view of the store.

use crate::{
    DefinitionPath, GroupId, Member, MetricPath, Point, RefPath, RunId, StoreError, TagPath,
    ontology::Recordable,
    records::{
        DEFINITIONS, GROUPS, GroupRecord, MEMBERS, NODES, PINS, REFS, RUN_METRICS, RUNS, Run,
        STATES, STEP_METRICS, STEPS, Step, TAGGED, TAGS, Target,
    },
    state,
    transaction::{decode, receipt_digest},
};
use axionomy::{ApplyError, Economy, Exchange, OntologyPath, Receipt, paths::Paths};
use axionomy_tree::{Hash, Tree};
use redb::{ReadTransaction, ReadableTableMetadata};
use std::collections::BTreeMap;

/// A state the store holds or rebuilt: the kernel's economy, which decides
/// what exchanges do, and its canonical tree, whose hash is the state root.
pub struct State<O: Recordable> {
    pub economy: Economy<O>,
    pub tree: Tree,
}

impl<O: Recordable> Clone for State<O> {
    fn clone(&self) -> Self {
        Self {
            economy: self.economy.clone(),
            tree: self.tree.clone(),
        }
    }
}

impl<O: Recordable> std::fmt::Debug for State<O> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("State")
            .field("ontology", &O::NAME)
            .field("root", &self.root())
            .finish()
    }
}

impl<O: Recordable> State<O> {
    pub fn new(economy: Economy<O>) -> Self {
        let tree = state::tree::<O>(&economy);
        Self { economy, tree }
    }

    /// Applies an exchange to both, keeping them the same state.
    pub fn apply(&mut self, exchange: Exchange<O>) -> Result<Receipt<O>, ApplyError<O>> {
        let receipt = self.economy.apply(exchange)?;
        state::advance::<O>(&mut self.tree, &self.economy, &receipt);
        Ok(receipt)
    }

    pub fn root(&self) -> Hash {
        self.tree.hash()
    }
}

/// One step of a run as something consuming it step by step keeps it, such as
/// a replay buffer, a dataset or an audit: the exchange, the digest of its
/// receipt, and the roots of the states before and after it.
pub struct Transition<O: Recordable> {
    /// The step the exchange reached.
    pub step: u64,
    pub before: Hash,
    pub exchange: Exchange<O>,
    pub receipt: Hash,
    pub after: Hash,
}

impl<O: Recordable> std::fmt::Debug for Transition<O> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Transition")
            .field("step", &self.step)
            .field("before", &self.before)
            .field("exchange", &self.exchange)
            .field("receipt", &self.receipt)
            .field("after", &self.after)
            .finish()
    }
}

/// A read transaction: everything it returns is from one committed state of
/// the store, whatever is written meanwhile.
pub struct Reader {
    txn: ReadTransaction,
}

impl Reader {
    pub(crate) fn new(txn: ReadTransaction) -> Self {
        Self { txn }
    }

    /// The redb transaction, for tables an application keeps beside the
    /// store's own.
    pub fn redb(&self) -> &ReadTransaction {
        &self.txn
    }

    pub fn run(&self, run: RunId) -> Result<Option<Run>, StoreError> {
        self.txn
            .open_table(RUNS)?
            .get(run.key())?
            .map(|record| decode(record.value()))
            .transpose()
    }

    /// Runs in the order their IDs sort, after `after`, at most `limit`.
    pub fn runs(
        &self,
        after: Option<RunId>,
        limit: usize,
    ) -> Result<Vec<(RunId, Run)>, StoreError> {
        let table = self.txn.open_table(RUNS)?;
        let range = match after {
            Some(after) => table.range((after.key() + 1)..)?,
            None => table.range(0..)?,
        };
        range
            .take(limit)
            .map(|entry| {
                let (key, record) = entry?;
                Ok((RunId::from_key(key.value()), decode(record.value())?))
            })
            .collect()
    }

    /// The step of the run's last recorded state.
    pub fn last_step(&self, run: RunId) -> Result<Option<u64>, StoreError> {
        let Some(record) = self.run(run)? else {
            return Ok(None);
        };
        let last = self
            .txn
            .open_table(STEPS)?
            .range((run.key(), 0)..=(run.key(), u64::MAX))?
            .next_back()
            .transpose()?
            .map_or(record.first, |(key, _)| key.value().1);
        Ok(Some(last))
    }

    /// A run of `O`'s economies; another ontology's run is refused, except
    /// that any run reads as [`Paths`], since every identifier is written as
    /// its path (D-044).
    pub fn run_of<O: Recordable>(&self, run: RunId) -> Result<Option<Run>, StoreError> {
        let Some(record) = self.run(run)? else {
            return Ok(None);
        };
        let read = OntologyPath::of::<O>();
        if record.ontology != read && read != OntologyPath::of::<Paths>() {
            return Err(StoreError::OtherOntology {
                run,
                recorded: record.ontology,
                read,
            });
        }
        Ok(Some(record))
    }

    /// The exchange that took a run of `O`'s economies to `point`.
    pub fn step<O: Recordable>(&self, point: Point) -> Result<Option<Step<O>>, StoreError> {
        self.run_of::<O>(point.run)?;
        self.txn
            .open_table(STEPS)?
            .get(point.key())?
            .map(|record| decode(record.value()))
            .transpose()
    }

    /// A run's steps from `from` on, at most `limit`.
    pub fn steps<O: Recordable>(
        &self,
        run: RunId,
        from: u64,
        limit: usize,
    ) -> Result<Vec<(u64, Step<O>)>, StoreError> {
        self.run_of::<O>(run)?;
        self.txn
            .open_table(STEPS)?
            .range((run.key(), from)..=(run.key(), u64::MAX))?
            .take(limit)
            .map(|entry| {
                let (key, record) = entry?;
                Ok((key.value().1, decode(record.value())?))
            })
            .collect()
    }

    /// The root of the state at `point`, if it is stored.
    pub fn root(&self, point: Point) -> Result<Option<Hash>, StoreError> {
        Ok(self
            .txn
            .open_table(STATES)?
            .get(point.key())?
            .map(|root| Hash::from_bytes(*root.value())))
    }

    /// Loads a stored tree, checking every node.
    pub fn tree(&self, root: &Hash) -> Result<Tree, StoreError> {
        let nodes = self.txn.open_table(NODES)?;
        Tree::load(root, |hash| {
            nodes
                .get(hash.as_bytes())
                .map(|node| node.map(|node| node.value().to_vec()))
        })
        .map_err(|error| match error {
            axionomy_tree::LoadError::Fetch(error) => StoreError::from(error),
            axionomy_tree::LoadError::Missing(hash) => StoreError::MissingNode(hash),
            axionomy_tree::LoadError::Malformed { node, .. } => StoreError::MissingNode(node),
        })
    }

    /// The state at `point`: the nearest stored state at or before it, plus
    /// the run's exchanges in between, each checked against its receipt.
    pub fn state<O: Recordable>(&self, point: Point) -> Result<Option<State<O>>, StoreError> {
        let Some(record) = self.run_of::<O>(point.run)? else {
            return Ok(None);
        };
        if point.step < record.first || self.last_step(point.run)? < Some(point.step) {
            return Ok(None);
        }
        let (stored, root) = self
            .txn
            .open_table(STATES)?
            .range((point.run.key(), record.first)..=point.key())?
            .next_back()
            .transpose()?
            .map(|(key, root)| (key.value().1, Hash::from_bytes(*root.value())))
            .ok_or(StoreError::Malformed("a run without its first state"))?;
        let tree = self.tree(&root)?;
        let economy = state::economy::<O>(&tree)?;
        let mut state = State { economy, tree };
        self.replay(&mut state, point.run, stored, point.step)?;
        Ok(Some(state))
    }

    /// Applies a run's exchanges after step `after` through step `through` to
    /// `state`, the state at `after`, checking each against its recorded
    /// receipt, and returns their receipts.
    pub fn replay<O: Recordable>(
        &self,
        state: &mut State<O>,
        run: RunId,
        after: u64,
        through: u64,
    ) -> Result<Vec<Receipt<O>>, StoreError> {
        self.run_of::<O>(run)?;
        let steps = self.txn.open_table(STEPS)?;
        let mut receipts = Vec::new();
        for step in after.saturating_add(1)..=through {
            let point = Point { run, step };
            let record: Step<O> = decode(
                steps
                    .get(point.key())?
                    .ok_or(StoreError::Diverged(point))?
                    .value(),
            )?;
            let receipt = state
                .apply(record.exchange)
                .map_err(|_| StoreError::Diverged(point))?;
            if receipt_digest::<O>(&receipt) != record.receipt {
                return Err(StoreError::Diverged(point));
            }
            receipts.push(receipt);
        }
        Ok(receipts)
    }

    /// Replays a whole run from its first state, checking every receipt and
    /// every stored state against what replay produces.
    pub fn verify<O: Recordable>(&self, run: RunId) -> Result<(), StoreError> {
        let record = self.run_of::<O>(run)?.ok_or(StoreError::UnknownRun(run))?;
        let last = self.last_step(run)?.unwrap_or(record.first);
        let tree = self.tree(&record.root)?;
        let mut state = State::<O> {
            economy: state::economy::<O>(&tree)?,
            tree,
        };
        let stored: Vec<(u64, Hash)> = self
            .txn
            .open_table(STATES)?
            .range((run.key(), record.first)..=(run.key(), u64::MAX))?
            .map(|entry| entry.map(|(key, root)| (key.value().1, Hash::from_bytes(*root.value()))))
            .collect::<Result<_, _>>()?;
        let mut at = record.first;
        for (step, root) in stored {
            self.replay(&mut state, run, at, step)?;
            at = step;
            if state.root() != root {
                return Err(StoreError::Diverged(Point { run, step }));
            }
        }
        self.replay(&mut state, run, at, last)?;
        Ok(())
    }

    /// A run's steps after step `after` as transitions, at most `limit`: each
    /// exchange between the roots of the states before and after it, replayed
    /// and checked against its receipt.
    pub fn transitions<O: Recordable>(
        &self,
        run: RunId,
        after: u64,
        limit: usize,
    ) -> Result<Vec<Transition<O>>, StoreError> {
        let Some(mut state) = self.state::<O>(Point { run, step: after })? else {
            return Err(StoreError::UnknownRun(run));
        };
        let mut transitions = Vec::new();
        for (step, record) in self.steps::<O>(run, after.saturating_add(1), limit)? {
            let point = Point { run, step };
            let before = state.root();
            let receipt = state
                .apply(record.exchange.clone())
                .map_err(|_| StoreError::Diverged(point))?;
            if receipt_digest::<O>(&receipt) != record.receipt {
                return Err(StoreError::Diverged(point));
            }
            transitions.push(Transition {
                step,
                before,
                exchange: record.exchange,
                receipt: record.receipt,
                after: state.root(),
            });
        }
        Ok(transitions)
    }

    /// The first step from `from` through `through` of a run whose state
    /// satisfies `holds`, or none. `holds` must keep holding once it holds,
    /// such as "the clock has reached noon" or "the agent has delivered": the
    /// search halves the run's stored states, loading O(log n) of them, then
    /// replays from the last one where `holds` fails, at most the distance
    /// between two stored states.
    pub fn bisect<O: Recordable>(
        &self,
        run: RunId,
        from: u64,
        through: u64,
        mut holds: impl FnMut(&Economy<O>) -> bool,
    ) -> Result<Option<u64>, StoreError> {
        let mut base = self
            .state::<O>(Point { run, step: from })?
            .ok_or(StoreError::UnknownRun(run))?;
        if self.last_step(run)? < Some(through) {
            return Err(StoreError::UnknownRun(run));
        }
        if holds(&base.economy) {
            return Ok(Some(from));
        }
        let stored: Vec<(u64, Hash)> = self
            .txn
            .open_table(STATES)?
            .range((run.key(), from.saturating_add(1))..=(run.key(), through))?
            .map(|entry| entry.map(|(key, root)| (key.value().1, Hash::from_bytes(*root.value()))))
            .collect::<Result<_, _>>()?;
        // The stored states where `holds` fails come first; find the last.
        let (mut low, mut high) = (0, stored.len());
        let mut failing = None;
        while low < high {
            let middle = low + (high - low) / 2;
            let (step, root) = stored[middle];
            let tree = self.tree(&root)?;
            let economy = state::economy::<O>(&tree)?;
            if holds(&economy) {
                high = middle;
            } else {
                failing = Some((step, State { economy, tree }));
                low = middle + 1;
            }
        }
        let mut at = from;
        if let Some((step, state)) = failing {
            (at, base) = (step, state);
        }
        let end = stored.get(low).map_or(through, |&(step, _)| step);
        for step in at.saturating_add(1)..=end {
            self.replay(&mut base, run, step - 1, step)?;
            if holds(&base.economy) {
                return Ok(Some(step));
            }
        }
        Ok(None)
    }

    /// How many tree nodes the store holds.
    pub fn nodes(&self) -> Result<u64, StoreError> {
        Ok(self.txn.open_table(NODES)?.len()?)
    }

    pub fn pinned(&self, point: Point) -> Result<bool, StoreError> {
        Ok(self.txn.open_table(PINS)?.get(point.key())?.is_some())
    }

    pub fn tags(&self, run: RunId) -> Result<BTreeMap<TagPath, String>, StoreError> {
        self.txn
            .open_table(TAGS)?
            .range((run.key(), "")..(run.key() + 1, ""))?
            .map(|entry| {
                let (key, value) = entry?;
                Ok((path(key.value().1)?, value.value().to_owned()))
            })
            .collect()
    }

    /// Runs tagged `key` = `value`, in ID order, after `after`, at most `limit`.
    pub fn tagged(
        &self,
        key: &TagPath,
        value: &str,
        after: Option<RunId>,
        limit: usize,
    ) -> Result<Vec<RunId>, StoreError> {
        let start = after.map_or(0, |after| after.key() + 1);
        self.txn
            .open_table(TAGGED)?
            .range((key.as_str(), value, start)..=(key.as_str(), value, u128::MAX))?
            .take(limit)
            .map(|entry| Ok(RunId::from_key(entry?.0.value().2)))
            .collect()
    }

    pub fn run_metrics(&self, run: RunId) -> Result<BTreeMap<MetricPath, f64>, StoreError> {
        self.txn
            .open_table(RUN_METRICS)?
            .range((run.key(), "")..(run.key() + 1, ""))?
            .map(|entry| {
                let (key, value) = entry?;
                Ok((path(key.value().1)?, value.value()))
            })
            .collect()
    }

    pub fn step_metrics(&self, point: Point) -> Result<BTreeMap<MetricPath, f64>, StoreError> {
        let (run, step) = point.key();
        self.txn
            .open_table(STEP_METRICS)?
            .range((run, step, "")..(run, step + 1, ""))?
            .map(|entry| {
                let (key, value) = entry?;
                Ok((path(key.value().2)?, value.value()))
            })
            .collect()
    }

    /// A group with its running summaries.
    pub fn group(&self, group: GroupId) -> Result<Option<GroupRecord>, StoreError> {
        self.txn
            .open_table(GROUPS)?
            .get(group.key())?
            .map(|record| decode(record.value()))
            .transpose()
    }

    /// A group's members from sample `from` on, at most `limit`.
    pub fn members(
        &self,
        group: GroupId,
        from: u64,
        limit: usize,
    ) -> Result<Vec<(u64, Member)>, StoreError> {
        self.txn
            .open_table(MEMBERS)?
            .range((group.key(), from)..=(group.key(), u64::MAX))?
            .take(limit)
            .map(|entry| {
                let (key, record) = entry?;
                Ok((key.value().1, decode(record.value())?))
            })
            .collect()
    }

    /// The `q` quantile of a metric over a group's members, nearest rank.
    pub fn quantile(
        &self,
        group: GroupId,
        metric: &MetricPath,
        q: f64,
    ) -> Result<Option<f64>, StoreError> {
        let mut values: Vec<f64> = self
            .members(group, 0, usize::MAX)?
            .into_iter()
            .filter_map(|(_, member)| member.metrics.get(metric).copied())
            .collect();
        if values.is_empty() || !(0.0..=1.0).contains(&q) {
            return Ok(None);
        }
        values.sort_by(f64::total_cmp);
        let rank = (q * (values.len() - 1) as f64).round() as usize;
        Ok(Some(values[rank]))
    }

    pub fn reference(&self, name: &RefPath) -> Result<Option<Target>, StoreError> {
        self.txn
            .open_table(REFS)?
            .get(name.as_str())?
            .map(|target| decode(target.value()))
            .transpose()
    }

    /// What `name` defines, as its owner encoded it.
    pub fn definition(&self, name: &DefinitionPath) -> Result<Option<Vec<u8>>, StoreError> {
        Ok(self
            .txn
            .open_table(DEFINITIONS)?
            .get(name.as_str())?
            .map(|value| value.value().to_vec()))
    }

    /// Definitions whose names start with `prefix`, in name order, at most
    /// `limit`.
    pub fn definitions(
        &self,
        prefix: &str,
        limit: usize,
    ) -> Result<Vec<(DefinitionPath, Vec<u8>)>, StoreError> {
        self.txn
            .open_table(DEFINITIONS)?
            .range(prefix..)?
            .map_while(|entry| match entry {
                Ok((name, value)) => name
                    .value()
                    .starts_with(prefix)
                    .then(|| Ok((path(name.value())?, value.value().to_vec()))),
                Err(error) => Some(Err(error.into())),
            })
            .take(limit)
            .collect()
    }

    /// Refs whose names start with `prefix`, in name order, at most `limit`.
    pub fn refs(&self, prefix: &str, limit: usize) -> Result<Vec<(RefPath, Target)>, StoreError> {
        self.txn
            .open_table(REFS)?
            .range(prefix..)?
            .map_while(|entry| match entry {
                Ok((name, target)) => name
                    .value()
                    .starts_with(prefix)
                    .then(|| Ok((path(name.value())?, decode(target.value())?))),
                Err(error) => Some(Err(error.into())),
            })
            .take(limit)
            .collect()
    }
}

fn path<K>(text: &str) -> Result<axionomy::path::Path<K>, StoreError> {
    text.parse()
        .map_err(|_| StoreError::Malformed("a stored path"))
}
