//! Reading one consistent committed view of the store.

use crate::{
    GroupId, Member, MetricPath, Point, RefPath, RunId, StoreError, TagPath,
    records::{
        GROUPS, GroupRecord, MEMBERS, NODES, PINS, REFS, RUN_METRICS, RUNS, Run, STATES,
        STEP_METRICS, STEPS, Step, TAGGED, TAGS, Target,
    },
    state,
    transaction::{decode, receipt_digest},
};
use axionomy::paths;
use axionomy_tree::{Hash, Tree};
use redb::{ReadTransaction, ReadableTableMetadata};
use std::collections::BTreeMap;

/// A state the store holds or rebuilt: the kernel's economy, which decides
/// what exchanges do, and its canonical tree, whose hash is the state root.
#[derive(Debug, Clone)]
pub struct State {
    pub economy: paths::Economy,
    pub tree: Tree,
}

impl State {
    pub fn new(economy: paths::Economy) -> Self {
        let tree = state::tree(&economy);
        Self { economy, tree }
    }

    /// Applies an exchange to both, keeping them the same state.
    pub fn apply(
        &mut self,
        exchange: paths::Exchange,
    ) -> Result<paths::Receipt, paths::ApplyError> {
        let receipt = self.economy.apply(exchange)?;
        state::advance(&mut self.tree, &self.economy, &receipt);
        Ok(receipt)
    }

    pub fn root(&self) -> Hash {
        self.tree.hash()
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

    /// The exchange that took a run to `point`.
    pub fn step(&self, point: Point) -> Result<Option<Step>, StoreError> {
        self.txn
            .open_table(STEPS)?
            .get(point.key())?
            .map(|record| decode(record.value()))
            .transpose()
    }

    /// A run's steps from `from` on, at most `limit`.
    pub fn steps(
        &self,
        run: RunId,
        from: u64,
        limit: usize,
    ) -> Result<Vec<(u64, Step)>, StoreError> {
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
    pub fn state(&self, point: Point) -> Result<Option<State>, StoreError> {
        let Some(record) = self.run(point.run)? else {
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
        let economy = state::economy(&tree)?;
        let mut state = State { economy, tree };
        self.replay(&mut state, point.run, stored, point.step)?;
        Ok(Some(state))
    }

    /// Applies a run's exchanges after step `after` through step `through` to
    /// `state`, the state at `after`, checking each against its recorded
    /// receipt, and returns their receipts.
    pub fn replay(
        &self,
        state: &mut State,
        run: RunId,
        after: u64,
        through: u64,
    ) -> Result<Vec<paths::Receipt>, StoreError> {
        let steps = self.txn.open_table(STEPS)?;
        let mut receipts = Vec::new();
        for step in after.saturating_add(1)..=through {
            let point = Point { run, step };
            let record: Step = decode(
                steps
                    .get(point.key())?
                    .ok_or(StoreError::Diverged(point))?
                    .value(),
            )?;
            let receipt = state
                .apply(record.exchange)
                .map_err(|_| StoreError::Diverged(point))?;
            if receipt_digest(&receipt) != record.receipt {
                return Err(StoreError::Diverged(point));
            }
            receipts.push(receipt);
        }
        Ok(receipts)
    }

    /// Replays a whole run from its first state, checking every receipt and
    /// every stored state against what replay produces.
    pub fn verify(&self, run: RunId) -> Result<(), StoreError> {
        let record = self.run(run)?.ok_or(StoreError::Malformed("no such run"))?;
        let last = self.last_step(run)?.unwrap_or(record.first);
        let tree = self.tree(&record.root)?;
        let mut state = State {
            economy: state::economy(&tree)?,
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
