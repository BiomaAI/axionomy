#![doc = include_str!("../README.md")]

mod ids;
mod ontology;
mod read;
mod recorder;
mod records;
pub mod state;
mod transaction;
mod writer;

pub use ids::{
    DefinitionPath, GroupId, GroupKindPath, IdError, MetricPath, Point, RefPath, RunId, TagPath,
    kind,
};
pub use ontology::{Id, Recordable};
pub use read::{Reader, State, Transition};
pub use recorder::{GroupRecorder, Recorder};
pub use records::{
    Checkpoints, FORMAT_VERSION, Group, GroupRecord, Member, Run, Step, Summary, Target,
};
pub use state::StateError;
pub use transaction::{Merged, Selection};
pub use writer::{Pending, Transaction};

use axionomy::{Economy, OntologyPath};
use axionomy_tree::Hash;
use records::{FORMAT, GROUPS, META, RUNS, STORE_PREFIX};
use redb::{Database, ReadableDatabase, ReadableTable, backends::InMemoryBackend};
use std::{
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};
use thiserror::Error;
use writer::Writer;

/// Whether a write waits for disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Durability {
    /// Durable when its transaction returns: for callers that wait, such as
    /// an MCP call or a checkpoint someone asked for.
    Immediate,
    /// Durable with the writer's next durable commit, within a second: for
    /// batches nobody waits on. A crash loses at most that tail, never
    /// consistency.
    Eventual,
}

#[derive(Debug, Clone, Error)]
pub enum StoreError {
    #[error("storage failed: {0}")]
    Storage(Arc<redb::Error>),
    #[error("a stored {0} does not decode")]
    Malformed(&'static str),
    #[error("tree node {0} is missing")]
    MissingNode(Hash),
    #[error("tree node {0} is malformed")]
    MalformedNode(Hash),
    #[error(transparent)]
    State(#[from] StateError),
    #[error("replaying step {} of {} does not give what was recorded", .0.step, .0.run)]
    Diverged(Point),
    #[error("{0} is not a run of this store")]
    UnknownRun(RunId),
    #[error("{0} is not a group of this store")]
    UnknownGroup(GroupId),
    #[error("no ref `{0}` is kept")]
    UnknownRef(RefPath),
    #[error("nothing defines `{0}`")]
    Undefined(DefinitionPath),
    /// A ref was not where a compare-and-set expected it; it is on
    /// `current`, or names nothing.
    #[error("ref `{name}` has moved")]
    Moved {
        name: RefPath,
        current: Option<Target>,
    },
    #[error("{run} records {recorded} economies, not {read}")]
    OtherOntology {
        run: RunId,
        recorded: OntologyPath,
        read: OntologyPath,
    },
    #[error("metric {0} is not a finite number")]
    NotFinite(MetricPath),
    #[error("the store's writer has stopped")]
    Closed,
    /// The store was written by a version that writes another format, or
    /// before formats were recorded; this version reads only its own.
    #[error(
        "the store is written in {}, but this version reads only format {FORMAT_VERSION}",
        written_in(.found)
    )]
    Format { found: Option<u64> },
}

fn written_in(found: &Option<u64>) -> String {
    match found {
        Some(format) => format!("format {format}"),
        None => "a format from before formats were recorded".to_owned(),
    }
}

macro_rules! storage_errors {
    ($($error:ty),*) => {$(
        impl From<$error> for StoreError {
            fn from(error: $error) -> Self {
                Self::Storage(Arc::new(error.into()))
            }
        }
    )*};
}

storage_errors!(
    redb::Error,
    redb::DatabaseError,
    redb::TransactionError,
    redb::TableError,
    redb::StorageError,
    redb::CommitError,
    redb::SetDurabilityError,
    redb::CompactionError
);

/// Runs, their states and what organizes them, in one redb database.
///
/// Clones share the database and its one writer thread; the last clone
/// dropped flushes and stops the writer.
#[derive(Clone)]
pub struct Store {
    inner: Arc<Inner>,
}

struct Inner {
    // Dropped first: stopping the writer flushes what it holds.
    writer: Writer,
    database: Arc<Database>,
    prefix: u64,
    counter: AtomicU64,
    in_memory: bool,
}

impl Store {
    /// Opens the store in a file, creating it if it does not exist.
    ///
    /// # Panics
    ///
    /// When creating a store and the operating system provides no
    /// randomness for its identifier prefix.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        Self::with(Database::create(path)?, false)
    }

    /// A store kept in memory holding the store file `bytes`, such as a
    /// package fetched or unzipped: changes stay in memory.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, StoreError> {
        use redb::StorageBackend;
        let backend = InMemoryBackend::new();
        let length = u64::try_from(bytes.len()).expect("a length fits in 64 bits");
        backend
            .set_len(length)
            .and_then(|()| backend.write(0, bytes))
            .map_err(redb::StorageError::from)?;
        Self::with(Database::builder().create_with_backend(backend)?, true)
    }

    /// Compacts the store file at `path`, which no store may have open, so
    /// it takes no more space than what it holds; returns whether it shrank.
    pub fn compact(path: impl AsRef<Path>) -> Result<bool, StoreError> {
        Ok(Database::open(path)?.compact()?)
    }

    /// A store kept in memory: the same store, gone when dropped.
    pub fn memory() -> Result<Self, StoreError> {
        Self::with(
            Database::builder().create_with_backend(InMemoryBackend::new())?,
            true,
        )
    }

    /// Whether the store is kept in memory, so everything in it is gone once
    /// its last clone is dropped.
    pub fn in_memory(&self) -> bool {
        self.inner.in_memory
    }

    fn with(database: Database, in_memory: bool) -> Result<Self, StoreError> {
        let txn = database.begin_write()?;
        let prefix = {
            let mut meta = txn.open_table(META)?;
            let stored = meta.get(STORE_PREFIX)?.map(|prefix| prefix.value());
            match stored {
                Some(prefix) => {
                    let found = meta.get(FORMAT)?.map(|format| format.value());
                    if found != Some(FORMAT_VERSION) {
                        return Err(StoreError::Format { found });
                    }
                    prefix
                }
                None => {
                    let mut bytes = [0; 8];
                    getrandom::fill(&mut bytes)
                        .expect("operating system randomness is unavailable");
                    let prefix = u64::from_le_bytes(bytes);
                    meta.insert(STORE_PREFIX, prefix)?;
                    meta.insert(FORMAT, FORMAT_VERSION)?;
                    prefix
                }
            }
        };
        let mine = u128::from(prefix) << 64..=(u128::from(prefix) << 64 | u128::from(u64::MAX));
        let mut next = 0;
        for table in [RUNS, GROUPS] {
            if let Some(entry) = txn.open_table(table)?.range(mine.clone())?.next_back() {
                next = next.max((entry?.0.value() as u64).saturating_add(1));
            }
        }
        records::create_tables(&txn)?;
        txn.commit()?;
        let database = Arc::new(database);
        Ok(Self {
            inner: Arc::new(Inner {
                writer: Writer::start(Arc::clone(&database)),
                database,
                prefix,
                counter: AtomicU64::new(next),
                in_memory,
            }),
        })
    }

    /// A read transaction over the store's latest commit.
    pub fn read(&self) -> Result<Reader, StoreError> {
        Ok(Reader::new(self.inner.database.begin_read()?))
    }

    /// Queues `work` for the writer, which runs it in a transaction with
    /// other queued work and commits them together.
    pub fn transact<T: Send + 'static>(
        &self,
        durability: Durability,
        work: impl FnOnce(&mut Transaction<'_>) -> Result<T, StoreError> + Send + 'static,
    ) -> Pending<T> {
        self.inner
            .writer
            .submit(durability == Durability::Immediate, Box::new(work))
    }

    /// A new run identifier, unique across stores.
    pub fn mint_run(&self) -> RunId {
        RunId::new(self.inner.prefix, self.mint())
    }

    /// A new group identifier, unique across stores.
    pub fn mint_group(&self) -> GroupId {
        GroupId::new(self.inner.prefix, self.mint())
    }

    fn mint(&self) -> u64 {
        self.inner.counter.fetch_add(1, Ordering::Relaxed)
    }

    /// Starts recording a new run from `economy`.
    pub fn record<O: Recordable>(
        &self,
        economy: Economy<O>,
        checkpoints: Checkpoints,
    ) -> Recorder<O> {
        Recorder::new(self.clone(), None, State::new(economy), checkpoints)
    }

    /// Starts recording a new run from the state at `point`, numbering its
    /// steps on from there. The run it forks must be of `O`'s economies.
    pub fn fork<O: Recordable>(
        &self,
        point: Point,
        checkpoints: Checkpoints,
    ) -> Result<Recorder<O>, StoreError> {
        let state = self
            .read()?
            .state::<O>(point)?
            .ok_or(StoreError::UnknownRun(point.run))?;
        Ok(Recorder::new(self.clone(), Some(point), state, checkpoints))
    }

    /// Starts recording a group of runs, such as one Monte Carlo estimate's
    /// rollouts.
    pub fn group(&self, group: Group) -> GroupRecorder {
        GroupRecorder::new(self.clone(), group)
    }

    /// Copies `selection` from this store into `to`: its refs and
    /// definitions as they are, its groups with their members, and every run
    /// its refs, points and groups name, back through the runs they forked
    /// from, with their steps, states, pins, tags and metrics and every tree
    /// node under their states. `to` gets what a package of the selection
    /// needs and nothing else.
    pub fn export(&self, selection: Selection, to: &Store) -> Pending<()> {
        match self.read() {
            Ok(reader) => to.transact(Durability::Immediate, move |txn| {
                transaction::export(txn, reader.redb(), &selection)
            }),
            Err(error) => to.transact(Durability::Eventual, move |_| Err(error)),
        }
    }

    /// Copies everything `other` holds that this store lacks. Run and group
    /// IDs are unique across stores and nodes are content addressed, so
    /// nothing conflicts but names: returns the refs `other` points elsewhere
    /// and the definitions it defines otherwise, which keep what they are
    /// here.
    pub fn merge(&self, other: &Store) -> Pending<Merged> {
        match other.read() {
            Ok(reader) => self.transact(Durability::Immediate, move |txn| {
                transaction::merge(txn, reader.redb())
            }),
            Err(error) => self.transact(Durability::Eventual, move |_| Err(error)),
        }
    }
}
