//! What the store records about runs, steps, groups and refs, and the redb
//! tables that hold them.

use crate::{GroupId, GroupKindPath, MetricPath, Point, RunId, TagPath};
use axionomy::{Exchange, Ontology, OntologyPath};
use axionomy_tree::Hash;
use redb::TableDefinition;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{collections::BTreeMap, num::NonZeroU64};

/// The store's own 64-bit prefix for the identifiers it mints, and the format
/// its records and trees are written in.
pub(crate) const META: TableDefinition<&str, u64> = TableDefinition::new("meta");
pub(crate) const STORE_PREFIX: &str = "store-prefix";
pub(crate) const FORMAT: &str = "format";
/// The format this version writes and reads: records, tree values and receipt
/// digests in postcard, identifiers in keys as their paths.
/// The format this version writes and reads, which a package records.
pub const FORMAT_VERSION: u64 = 2;
/// Tree nodes by hash.
pub(crate) const NODES: TableDefinition<&[u8; 32], &[u8]> = TableDefinition::new("nodes");
pub(crate) const RUNS: TableDefinition<u128, &[u8]> = TableDefinition::new("runs");
pub(crate) const STEPS: TableDefinition<(u128, u64), &[u8]> = TableDefinition::new("steps");
/// The roots of stored states: every run's first state, and its checkpoints.
pub(crate) const STATES: TableDefinition<(u128, u64), &[u8; 32]> = TableDefinition::new("states");
pub(crate) const PINS: TableDefinition<(u128, u64), ()> = TableDefinition::new("pins");
pub(crate) const TAGS: TableDefinition<(u128, &str), &str> = TableDefinition::new("tags");
/// Tags by key and value, for finding runs.
pub(crate) const TAGGED: TableDefinition<(&str, &str, u128), ()> = TableDefinition::new("tagged");
pub(crate) const RUN_METRICS: TableDefinition<(u128, &str), f64> =
    TableDefinition::new("run-metrics");
pub(crate) const STEP_METRICS: TableDefinition<(u128, u64, &str), f64> =
    TableDefinition::new("step-metrics");
pub(crate) const GROUPS: TableDefinition<u128, &[u8]> = TableDefinition::new("groups");
pub(crate) const MEMBERS: TableDefinition<(u128, u64), &[u8]> = TableDefinition::new("members");
pub(crate) const REFS: TableDefinition<&str, &[u8]> = TableDefinition::new("refs");
/// Every fork, by the run and step it forked from and the run it made.
pub(crate) const BRANCHES: TableDefinition<(u128, u64, u128), ()> =
    TableDefinition::new("branches");
/// What packages define, by name: scripts, goals, strategies and the rest, as
/// the bytes their owner encodes.
pub(crate) const DEFINITIONS: TableDefinition<&str, &[u8]> = TableDefinition::new("definitions");

/// Creates every table, so readers never meet a missing one.
pub(crate) fn create_tables(txn: &redb::WriteTransaction) -> Result<(), redb::TableError> {
    txn.open_table(NODES)?;
    txn.open_table(RUNS)?;
    txn.open_table(STEPS)?;
    txn.open_table(STATES)?;
    txn.open_table(PINS)?;
    txn.open_table(TAGS)?;
    txn.open_table(TAGGED)?;
    txn.open_table(RUN_METRICS)?;
    txn.open_table(STEP_METRICS)?;
    txn.open_table(GROUPS)?;
    txn.open_table(MEMBERS)?;
    txn.open_table(REFS)?;
    txn.open_table(BRANCHES)?;
    txn.open_table(DEFINITIONS)?;
    Ok(())
}

/// A recorded run: where it starts and which of its states are stored.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Run {
    /// The ontology of the run's economies, which it is read back as.
    pub ontology: OntologyPath,
    /// The point this run forked from, whose state is its first; none for a
    /// run that starts from a state of its own.
    pub from: Option<Point>,
    /// The step number of the run's first state: zero, or the step it forked
    /// at, so forks number their steps as their source does.
    pub first: u64,
    /// The root of the run's first state, which is always stored.
    #[serde(with = "digest")]
    pub root: Hash,
    pub checkpoints: Checkpoints,
    pub finished: bool,
}

/// Which of a run's states are stored. Any other state is the nearest stored
/// state before it plus replay; pinning a step stores its state too.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Checkpoints {
    /// Every state: debugging, regression scenarios, transitions by root.
    EveryStep,
    /// Every state whose step is a multiple of n, and the last: any step is
    /// at most n - 1 replays from a stored state.
    Every(NonZeroU64),
    /// The first and last states: rollouts and episodes of any kind.
    Ends,
}

impl Checkpoints {
    /// Whether the state after `step` is stored while the run continues.
    pub(crate) fn keeps(self, step: u64) -> bool {
        match self {
            Self::EveryStep => true,
            Self::Every(n) => step.is_multiple_of(n.get()),
            Self::Ends => false,
        }
    }
}

/// One exchange of a run of `O`'s economies.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(bound(
    serialize = "O::Rate: Serialize, O::Role: Serialize, O::Account: Serialize",
    deserialize = "O::Rate: Deserialize<'de>, O::Role: Deserialize<'de>, O::Account: Deserialize<'de>"
))]
pub struct Step<O: Ontology> {
    pub exchange: Exchange<O>,
    /// The BLAKE3 hash of the receipt's postcard, which replay must
    /// reproduce.
    #[serde(with = "digest")]
    pub receipt: Hash,
}

/// Runs or samples produced together: the rollouts of one Monte Carlo
/// estimate, the samples of one MPPI decision, an evaluation suite.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Group {
    pub kind: GroupKindPath,
    /// The point its members started from, if they share one.
    pub source: Option<Point>,
    /// Parameters, as tags.
    pub tags: BTreeMap<TagPath, String>,
}

/// A group with the running summary of each metric its members reported.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GroupRecord {
    pub group: Group,
    pub summaries: BTreeMap<MetricPath, Summary>,
}

/// One sample of a group: a run, or the seed that reproduces one, with its
/// metrics.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Member {
    pub run: Option<RunId>,
    pub seed: Option<u64>,
    pub metrics: BTreeMap<MetricPath, f64>,
}

/// Count, mean, extremes and variance of a metric over a group's members,
/// updated as members are added.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Summary {
    pub count: u64,
    pub mean: f64,
    pub min: f64,
    pub max: f64,
    /// The sum of squared deviations from the mean (Welford's M2).
    sum_of_squares: f64,
}

impl Summary {
    pub(crate) fn of(value: f64) -> Self {
        Self {
            count: 1,
            mean: value,
            min: value,
            max: value,
            sum_of_squares: 0.0,
        }
    }

    pub(crate) fn add(&mut self, value: f64) {
        self.count += 1;
        let delta = value - self.mean;
        self.mean += delta / self.count as f64;
        self.sum_of_squares += delta * (value - self.mean);
        self.min = self.min.min(value);
        self.max = self.max.max(value);
    }

    /// The sample variance; zero for a single value.
    pub fn variance(&self) -> f64 {
        if self.count < 2 {
            0.0
        } else {
            self.sum_of_squares / (self.count - 1) as f64
        }
    }
}

/// What a ref names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Target {
    Run(RunId),
    Step(Point),
    Group(GroupId),
}

/// A digest: its hex digits where people read the format, its 32 bytes in the
/// store's records.
pub(crate) mod digest {
    use super::*;

    pub fn serialize<S: Serializer>(hash: &Hash, serializer: S) -> Result<S::Ok, S::Error> {
        if serializer.is_human_readable() {
            serializer.collect_str(&hash.to_hex())
        } else {
            serializer.serialize_bytes(hash.as_bytes())
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Hash, D::Error> {
        if deserializer.is_human_readable() {
            let text = std::borrow::Cow::<str>::deserialize(deserializer)?;
            return Hash::from_hex(text.as_bytes()).map_err(serde::de::Error::custom);
        }
        deserializer.deserialize_bytes(Bytes)
    }

    struct Bytes;

    impl serde::de::Visitor<'_> for Bytes {
        type Value = Hash;

        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("a 32-byte digest")
        }

        fn visit_bytes<E: serde::de::Error>(self, value: &[u8]) -> Result<Hash, E> {
            let bytes: [u8; 32] = value
                .try_into()
                .map_err(|_| E::invalid_length(value.len(), &self))?;
            Ok(Hash::from_bytes(bytes))
        }
    }
}
