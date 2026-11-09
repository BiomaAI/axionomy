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
    GroupId, GroupKindPath, IdError, MetricPath, OntologyPath, Point, RefPath, RunId, TagPath, kind,
};
pub use ontology::{ApplyError, Economy, Exchange, Id, Receipt, Recordable};
pub use read::{Reader, State, Transition};
pub use recorder::Recorder;
pub use records::{Checkpoints, Group, GroupRecord, Member, Run, Step, StepOf, Summary, Target};
pub use state::StateError;
pub use writer::{Pending, Transaction};

use axionomy_tree::Hash;
use records::{GROUPS, META, RUNS, STORE_PREFIX};
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
    #[error("{run} records {recorded} economies, not {read}")]
    OtherOntology {
        run: RunId,
        recorded: OntologyPath,
        read: &'static str,
    },
    #[error("ontology name `{0}` is not a path")]
    OntologyName(&'static str),
    #[error("metric {0} is not a finite number")]
    NotFinite(MetricPath),
    #[error("the store's writer has stopped")]
    Closed,
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
    redb::SetDurabilityError
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
}

impl Store {
    /// Opens the store in a file, creating it if it does not exist.
    ///
    /// # Panics
    ///
    /// When creating a store and the operating system provides no
    /// randomness for its identifier prefix.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        Self::with(Database::create(path)?)
    }

    /// A store kept in memory: the same store, gone when dropped.
    pub fn memory() -> Result<Self, StoreError> {
        Self::with(Database::builder().create_with_backend(InMemoryBackend::new())?)
    }

    fn with(database: Database) -> Result<Self, StoreError> {
        let txn = database.begin_write()?;
        let prefix = {
            let mut meta = txn.open_table(META)?;
            let stored = meta.get(STORE_PREFIX)?.map(|prefix| prefix.value());
            match stored {
                Some(prefix) => prefix,
                None => {
                    let mut bytes = [0; 8];
                    getrandom::fill(&mut bytes)
                        .expect("operating system randomness is unavailable");
                    let prefix = u64::from_le_bytes(bytes);
                    meta.insert(STORE_PREFIX, prefix)?;
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

    /// Copies everything `other` holds that this store lacks. Run and group
    /// IDs are unique across stores and nodes are content addressed, so
    /// nothing conflicts but refs: returns the refs `other` points elsewhere,
    /// which keep their targets here.
    pub fn merge(&self, other: &Store) -> Pending<Vec<RefPath>> {
        match other.read() {
            Ok(reader) => self.transact(Durability::Immediate, move |txn| {
                transaction::merge(txn, reader.redb())
            }),
            Err(error) => self.transact(Durability::Eventual, move |_| Err(error)),
        }
    }
}
