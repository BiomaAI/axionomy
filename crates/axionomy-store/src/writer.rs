//! The store's one writer. Where threads run, a thread of its own commits
//! many callers' batches per transaction; on WebAssembly, which has none to
//! spare, each batch commits inline on its caller, in a transaction of its
//! own. Either way a batch commits or fails whole, and says when it is
//! durable.

use crate::StoreError;
use axionomy_tree::{Hash, Tree};
use redb::{Database, Durability, ReadableTable, WriteTransaction};
use std::{
    collections::HashSet,
    future::Future,
    pin::Pin,
    sync::{Arc, Condvar, Mutex},
    task::{Context, Poll, Waker},
};

#[cfg(any(target_family = "wasm", test))]
mod inline;
#[cfg(not(target_family = "wasm"))]
mod threaded;

#[cfg(target_family = "wasm")]
pub(crate) use inline::Writer;
#[cfg(not(target_family = "wasm"))]
pub(crate) use threaded::Writer;

/// Node hashes remembered as stored, so writing a state looks up only the
/// nodes it changed; forgotten wholesale past this many.
const KNOWN_NODES: usize = 1 << 20;

/// Work done inside the writer's transaction.
pub(crate) type Work<T> = Box<dyn FnOnce(&mut Transaction<'_>) -> Result<T, StoreError> + Send>;

/// One write transaction of the store, shared by the batches committed
/// together. Everything written through it commits or fails together.
pub struct Transaction<'a> {
    txn: &'a WriteTransaction,
    known: &'a mut Known,
}

impl Transaction<'_> {
    /// The redb transaction, for tables an application keeps beside the
    /// store's own, written atomically with them.
    pub fn redb(&self) -> &WriteTransaction {
        self.txn
    }

    /// Stores the nodes of `tree` the store does not hold yet and returns its
    /// root. Nodes are content addressed, so storing a tree twice, or two
    /// trees that share nodes, writes each node once.
    pub fn put_tree(&mut self, tree: &Tree) -> Result<Hash, StoreError> {
        let mut nodes = self.txn.open_table(crate::records::NODES)?;
        let Known {
            stored, written, ..
        } = &mut *self.known;
        let mut lookup = Ok(());
        let missing = tree.missing(|hash| {
            if stored.contains(hash) || written.contains(hash) {
                return true;
            }
            match nodes.get(hash.as_bytes()) {
                Ok(found) => found.is_some(),
                Err(error) => {
                    lookup = Err(error);
                    true
                }
            }
        });
        lookup?;
        for (hash, encoding) in missing {
            nodes.insert(hash.as_bytes(), encoding.as_slice())?;
            written.insert(hash);
        }
        Ok(tree.hash())
    }

    /// Forgets every node hash remembered as stored, after nodes are deleted.
    pub(crate) fn forget_nodes(&mut self) {
        self.known.forget = true;
    }
}

#[derive(Default)]
struct Known {
    stored: HashSet<Hash>,
    /// Written by the open transaction; stored once it commits.
    written: HashSet<Hash>,
    forget: bool,
}

impl Known {
    fn committed(&mut self) {
        if std::mem::take(&mut self.forget) || self.stored.len() > KNOWN_NODES {
            self.stored.clear();
        }
        self.stored.extend(self.written.drain());
    }

    fn aborted(&mut self) {
        self.written.clear();
        self.forget = false;
    }
}

trait Job: Send {
    fn run(&mut self, txn: &mut Transaction<'_>) -> Result<(), StoreError>;
    fn complete(self: Box<Self>, result: Result<(), StoreError>);
}

struct Task<T> {
    work: Option<Work<T>>,
    output: Option<T>,
    promise: Arc<Promise<T>>,
}

/// A job doing `work`, and the result its caller waits for.
fn job<T: Send + 'static>(work: Work<T>) -> (Box<dyn Job>, Pending<T>) {
    let promise = Arc::new(Promise::default());
    let task = Task {
        work: Some(work),
        output: None,
        promise: Arc::clone(&promise),
    };
    (Box::new(task), Pending { promise })
}

impl<T: Send> Job for Task<T> {
    fn run(&mut self, txn: &mut Transaction<'_>) -> Result<(), StoreError> {
        let work = self.work.take().expect("a job runs once");
        self.output = Some(work(txn)?);
        Ok(())
    }

    fn complete(mut self: Box<Self>, result: Result<(), StoreError>) {
        let result = result.map(|()| self.output.take().expect("a committed job has its output"));
        self.promise.resolve(result);
    }
}

impl<T> Drop for Task<T> {
    fn drop(&mut self) {
        // A job dropped before completing, when the writer has stopped.
        self.promise.resolve(Err(StoreError::Closed));
    }
}

/// Runs `group` in one write transaction, commits it, durable or not, and
/// gives every job the outcome: all of them commit or none does. Returns
/// whether they did.
fn commit(
    database: &Database,
    known: &mut Known,
    mut group: Vec<Box<dyn Job>>,
    durable: bool,
) -> bool {
    let result = transaction(database, known, &mut group, durable);
    match &result {
        Ok(()) => known.committed(),
        Err(_) => known.aborted(),
    }
    let committed = result.is_ok();
    for job in group {
        job.complete(result.clone());
    }
    committed
}

fn transaction(
    database: &Database,
    known: &mut Known,
    group: &mut [Box<dyn Job>],
    durable: bool,
) -> Result<(), StoreError> {
    let mut txn = database.begin_write()?;
    txn.set_durability(if durable {
        Durability::Immediate
    } else {
        Durability::None
    })?;
    let mut context = Transaction { txn: &txn, known };
    for job in group.iter_mut() {
        job.run(&mut context)?;
    }
    txn.commit()?;
    Ok(())
}

/// Makes every earlier commit durable.
fn flush(database: &Database) -> Result<(), StoreError> {
    let txn = database.begin_write()?;
    txn.commit()?;
    Ok(())
}

/// The result of queued work, once its transaction commits. Wait for it from
/// a thread, or await it from async code. Where the writer runs inline, the
/// result is there when [`Store::transact`](crate::Store::transact) returns.
#[must_use = "dropping a pending write does not cancel it; wait for it to know it committed"]
pub struct Pending<T> {
    promise: Arc<Promise<T>>,
}

impl<T> Pending<T> {
    /// The result, if the transaction has finished.
    pub(crate) fn try_take(&self) -> Option<Result<T, StoreError>> {
        self.promise
            .state
            .lock()
            .expect("pending lock")
            .result
            .take()
    }

    pub fn wait(self) -> Result<T, StoreError> {
        let mut state = self.promise.state.lock().expect("pending lock");
        loop {
            if let Some(result) = state.result.take() {
                return result;
            }
            state = self.promise.ready.wait(state).expect("pending lock");
        }
    }
}

impl<T> Future for Pending<T> {
    type Output = Result<T, StoreError>;

    fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        let mut state = self.promise.state.lock().expect("pending lock");
        match state.result.take() {
            Some(result) => Poll::Ready(result),
            None => {
                state.waker = Some(context.waker().clone());
                Poll::Pending
            }
        }
    }
}

struct Promise<T> {
    state: Mutex<PromiseState<T>>,
    ready: Condvar,
}

struct PromiseState<T> {
    result: Option<Result<T, StoreError>>,
    resolved: bool,
    waker: Option<Waker>,
}

impl<T> Default for Promise<T> {
    fn default() -> Self {
        Self {
            state: Mutex::new(PromiseState {
                result: None,
                resolved: false,
                waker: None,
            }),
            ready: Condvar::new(),
        }
    }
}

impl<T> Promise<T> {
    /// Settles the promise; only the first result counts.
    fn resolve(&self, result: Result<T, StoreError>) {
        let mut state = self.state.lock().expect("pending lock");
        if std::mem::replace(&mut state.resolved, true) {
            return;
        }
        state.result = Some(result);
        if let Some(waker) = state.waker.take() {
            waker.wake();
        }
        self.ready.notify_all();
    }
}
