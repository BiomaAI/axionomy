//! Writing records: runs, steps, pins, tags, metrics, groups and refs; and
//! deleting, collecting and merging.

use crate::{
    GroupId, Member, MetricPath, Point, RefPath, RunId, StoreError, TagPath, Transaction,
    ontology::Recordable,
    records::{
        Checkpoints, GROUPS, Group, GroupRecord, MEMBERS, NODES, PINS, REFS, RUN_METRICS, RUNS,
        Run, STATES, STEP_METRICS, STEPS, Step, Summary, TAGGED, TAGS, Target,
    },
};
use axionomy::{OntologyPath, Receipt};
use axionomy_tree::{Hash, Tree};
use redb::{ReadableTable, TableDefinition};
use std::collections::HashSet;

impl Transaction<'_> {
    /// Records a new run of `O`'s economies whose first state, at step
    /// `first`, is `state`. A run forked from another records the same
    /// ontology.
    pub fn create_run<O: Recordable>(
        &mut self,
        run: RunId,
        from: Option<Point>,
        first: u64,
        state: &Tree,
        checkpoints: Checkpoints,
    ) -> Result<(), StoreError> {
        let ontology = OntologyPath::of::<O>();
        if let Some(from) = from {
            let source: Run = decode(
                self.redb()
                    .open_table(RUNS)?
                    .get(from.run.key())?
                    .ok_or(StoreError::UnknownRun(from.run))?
                    .value(),
            )?;
            if source.ontology != ontology {
                return Err(StoreError::OtherOntology {
                    run: from.run,
                    recorded: source.ontology,
                    read: O::NAME,
                });
            }
        }
        let root = self.put_tree(state)?;
        let record = Run {
            ontology,
            from,
            first,
            root,
            checkpoints,
            finished: false,
        };
        self.redb()
            .open_table(RUNS)?
            .insert(run.key(), json(&record).as_slice())?;
        self.redb()
            .open_table(STATES)?
            .insert((run.key(), first), root.as_bytes())?;
        Ok(())
    }

    /// Records the exchange that took a run to `point`, with its receipt, and
    /// the state it reached when the run's checkpoints keep it.
    pub fn put_step<O: Recordable>(
        &mut self,
        point: Point,
        receipt: &Receipt<O>,
        state: Option<&Tree>,
    ) -> Result<(), StoreError> {
        let step = Step {
            exchange: receipt.exchange().clone(),
            receipt: receipt_digest::<O>(receipt),
        };
        self.redb()
            .open_table(STEPS)?
            .insert(point.key(), json(&step).as_slice())?;
        if let Some(state) = state {
            self.put_state(point, state)?;
        }
        Ok(())
    }

    /// Stores the state a point reached, so reading it needs no replay.
    pub fn put_state(&mut self, point: Point, state: &Tree) -> Result<(), StoreError> {
        let root = self.put_tree(state)?;
        self.redb()
            .open_table(STATES)?
            .insert(point.key(), root.as_bytes())?;
        Ok(())
    }

    /// Keeps a point, and everything needed to read it, from garbage
    /// collection. Its state is stored too when given.
    pub fn pin(&mut self, point: Point, state: Option<&Tree>) -> Result<(), StoreError> {
        if let Some(state) = state {
            self.put_state(point, state)?;
        }
        self.redb().open_table(PINS)?.insert(point.key(), ())?;
        Ok(())
    }

    pub fn unpin(&mut self, point: Point) -> Result<(), StoreError> {
        self.redb().open_table(PINS)?.remove(point.key())?;
        Ok(())
    }

    /// Marks a run finished, storing its last state when given.
    pub fn finish(&mut self, last: Point, state: Option<&Tree>) -> Result<(), StoreError> {
        if let Some(state) = state {
            self.put_state(last, state)?;
        }
        let mut runs = self.redb().open_table(RUNS)?;
        let record = runs
            .get(last.run.key())?
            .map(|record| record.value().to_vec());
        if let Some(record) = record {
            let mut run: Run = decode(&record)?;
            run.finished = true;
            runs.insert(last.run.key(), json(&run).as_slice())?;
        }
        Ok(())
    }

    /// Sets a tag on a run, or removes it with `None`.
    pub fn tag(
        &mut self,
        run: RunId,
        key: &TagPath,
        value: Option<&str>,
    ) -> Result<(), StoreError> {
        let mut tags = self.redb().open_table(TAGS)?;
        let mut tagged = self.redb().open_table(TAGGED)?;
        let old = tags
            .get((run.key(), key.as_str()))?
            .map(|old| old.value().to_owned());
        if let Some(old) = old {
            tagged.remove((key.as_str(), old.as_str(), run.key()))?;
            tags.remove((run.key(), key.as_str()))?;
        }
        if let Some(value) = value {
            tags.insert((run.key(), key.as_str()), value)?;
            tagged.insert((key.as_str(), value, run.key()), ())?;
        }
        Ok(())
    }

    /// Records a metric of a run, or of one of its steps.
    pub fn metric(
        &mut self,
        run: RunId,
        step: Option<u64>,
        name: &MetricPath,
        value: f64,
    ) -> Result<(), StoreError> {
        match step {
            Some(step) => {
                self.redb()
                    .open_table(STEP_METRICS)?
                    .insert((run.key(), step, name.as_str()), value)?;
            }
            None => {
                self.redb()
                    .open_table(RUN_METRICS)?
                    .insert((run.key(), name.as_str()), value)?;
            }
        }
        Ok(())
    }

    pub fn create_group(&mut self, id: GroupId, group: Group) -> Result<(), StoreError> {
        let record = GroupRecord {
            group,
            summaries: Default::default(),
        };
        self.redb()
            .open_table(GROUPS)?
            .insert(id.key(), json(&record).as_slice())?;
        Ok(())
    }

    /// Adds sample `sample` to a group and folds its metrics into the group's
    /// summaries. A sample is added once: adding it again changes nothing and
    /// returns false, as does adding to a group that does not exist.
    pub fn add_member(
        &mut self,
        group: GroupId,
        sample: u64,
        member: &Member,
    ) -> Result<bool, StoreError> {
        let mut members = self.redb().open_table(MEMBERS)?;
        let mut groups = self.redb().open_table(GROUPS)?;
        if members.get((group.key(), sample))?.is_some() {
            return Ok(false);
        }
        let Some(record) = groups
            .get(group.key())?
            .map(|record| record.value().to_vec())
        else {
            return Ok(false);
        };
        let mut record: GroupRecord = decode(&record)?;
        for (name, value) in &member.metrics {
            if !value.is_finite() {
                return Err(StoreError::NotFinite(name.clone()));
            }
            record
                .summaries
                .entry(name.clone())
                .and_modify(|summary| summary.add(*value))
                .or_insert_with(|| Summary::of(*value));
        }
        members.insert((group.key(), sample), json(member).as_slice())?;
        groups.insert(group.key(), json(&record).as_slice())?;
        Ok(true)
    }

    /// Points a ref at a target, or removes it with `None`.
    pub fn set_ref(&mut self, name: &RefPath, target: Option<Target>) -> Result<(), StoreError> {
        let mut refs = self.redb().open_table(REFS)?;
        match target {
            Some(target) => refs.insert(name.as_str(), json(&target).as_slice())?,
            None => refs.remove(name.as_str())?,
        };
        Ok(())
    }

    /// Deletes a run with its steps, states, tags and metrics, unless one of
    /// its points is pinned or a ref names it. Returns whether it was deleted.
    /// Its nodes stay until [`collect`](Self::collect) finds them unreachable.
    pub fn delete_run(&mut self, run: RunId) -> Result<bool, StoreError> {
        let points = (run.key(), 0)..=(run.key(), u64::MAX);
        if self
            .redb()
            .open_table(PINS)?
            .range(points.clone())?
            .next()
            .is_some()
        {
            return Ok(false);
        }
        for entry in self.redb().open_table(REFS)?.iter()? {
            let target: Target = decode(entry?.1.value())?;
            if matches!(target, Target::Run(named) | Target::Step(Point { run: named, .. }) if named == run)
            {
                return Ok(false);
            }
        }
        if self.redb().open_table(RUNS)?.remove(run.key())?.is_none() {
            return Ok(false);
        }
        let (this, next) = (run.key(), run.key() + 1);
        self.redb()
            .open_table(STEPS)?
            .retain_in(points.clone(), |_, _| false)?;
        self.redb()
            .open_table(STATES)?
            .retain_in(points, |_, _| false)?;
        let mut tags = self.redb().open_table(TAGS)?;
        let mut tagged = self.redb().open_table(TAGGED)?;
        for entry in tags.extract_from_if((this, "")..(next, ""), |_, _| true)? {
            let (key, value) = entry?;
            tagged.remove((key.value().1, value.value(), this))?;
        }
        self.redb()
            .open_table(RUN_METRICS)?
            .retain_in((this, "")..(next, ""), |_, _| false)?;
        self.redb()
            .open_table(STEP_METRICS)?
            .retain_in((this, 0, "")..(next, 0, ""), |_, _| false)?;
        Ok(true)
    }

    /// Deletes every node no stored state reaches, and returns how many.
    pub fn collect(&mut self) -> Result<usize, StoreError> {
        let mut reachable = HashSet::new();
        let nodes = self.redb().open_table(NODES)?;
        let mut stack: Vec<Hash> = self
            .redb()
            .open_table(STATES)?
            .iter()?
            .map(|entry| entry.map(|(_, root)| Hash::from_bytes(*root.value())))
            .collect::<Result<_, _>>()?;
        while let Some(hash) = stack.pop() {
            if !reachable.insert(hash) {
                continue;
            }
            let encoding = nodes
                .get(hash.as_bytes())?
                .ok_or(StoreError::MissingNode(hash))?;
            stack.extend(
                axionomy_tree::children(encoding.value())
                    .map_err(|_| StoreError::MissingNode(hash))?,
            );
        }
        drop(nodes);
        let mut nodes = self.redb().open_table(NODES)?;
        let unreachable: Vec<[u8; 32]> = nodes
            .iter()?
            .filter_map(|entry| match entry {
                Ok((hash, _)) => {
                    let hash = *hash.value();
                    (!reachable.contains(&Hash::from_bytes(hash))).then_some(Ok(hash))
                }
                Err(error) => Some(Err(error)),
            })
            .collect::<Result<_, _>>()?;
        for hash in &unreachable {
            nodes.remove(hash)?;
        }
        drop(nodes);
        self.forget_nodes();
        Ok(unreachable.len())
    }
}

/// Copies what `other` holds and this store lacks. Identifiers are unique
/// across stores, so nothing conflicts but refs, where this store's wins.
/// Returns the refs left as they were.
pub(crate) fn merge(
    txn: &mut Transaction<'_>,
    other: &redb::ReadTransaction,
) -> Result<Vec<RefPath>, StoreError> {
    copy(txn, other, NODES)?;
    copy(txn, other, RUNS)?;
    copy(txn, other, STEPS)?;
    copy(txn, other, STATES)?;
    copy(txn, other, PINS)?;
    copy(txn, other, TAGS)?;
    copy(txn, other, TAGGED)?;
    copy(txn, other, RUN_METRICS)?;
    copy(txn, other, STEP_METRICS)?;
    copy(txn, other, GROUPS)?;
    copy(txn, other, MEMBERS)?;
    let mut kept = Vec::new();
    let mut refs = txn.redb().open_table(REFS)?;
    for entry in other.open_table(REFS)?.iter()? {
        let (name, target) = entry?;
        let ours = refs.get(name.value())?.map(|ours| ours.value().to_vec());
        match ours {
            Some(ours) if ours != target.value() => {
                kept.push(
                    name.value()
                        .parse()
                        .map_err(|_| StoreError::Malformed("a ref name"))?,
                );
            }
            Some(_) => {}
            None => {
                refs.insert(name.value(), target.value())?;
            }
        }
    }
    Ok(kept)
}

fn copy<K: redb::Key + 'static, V: redb::Value + 'static>(
    txn: &mut Transaction<'_>,
    other: &redb::ReadTransaction,
    table: TableDefinition<K, V>,
) -> Result<(), StoreError> {
    let mut ours = txn.redb().open_table(table)?;
    for entry in other.open_table(table)?.iter()? {
        let (key, value) = entry?;
        if ours.get(key.value())?.is_none() {
            ours.insert(key.value(), value.value())?;
        }
    }
    Ok(())
}

/// The digest a step records for its receipt.
pub(crate) fn receipt_digest<O: Recordable>(receipt: &Receipt<O>) -> Hash {
    blake3::hash(&json(receipt))
}

pub(crate) fn json(value: &impl serde::Serialize) -> Vec<u8> {
    serde_json::to_vec(value).expect("records serialize")
}

pub(crate) fn decode<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Result<T, StoreError> {
    serde_json::from_slice(bytes).map_err(|_| StoreError::Malformed(std::any::type_name::<T>()))
}
