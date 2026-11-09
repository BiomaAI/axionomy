//! The store's one writer: a thread that commits many callers' batches per
//! transaction.

use crate::StoreError;
use axionomy_tree::{Hash, Tree};
use redb::{Database, Durability, ReadableTable, WriteTransaction};
use std::{
    collections::HashSet,
    future::Future,
    pin::Pin,
    sync::{Arc, Condvar, Mutex, mpsc},
    task::{Context, Poll, Waker},
    time::{Duration, Instant},
};

/// Batches a caller does not wait for reach disk at least this often.
const FLUSH_INTERVAL: Duration = Duration::from_secs(1);
/// The most batches one transaction commits.
const GROUP: usize = 256;
/// Batches queued before callers wait: backpressure instead of memory.
pub(crate) const QUEUE: usize = 64;
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
    fn immediate(&self) -> bool;
    fn run(&mut self, txn: &mut Transaction<'_>) -> Result<(), StoreError>;
    fn complete(self: Box<Self>, result: Result<(), StoreError>);
}

struct Task<T> {
    work: Option<Work<T>>,
    output: Option<T>,
    immediate: bool,
    promise: Arc<Promise<T>>,
}

impl<T: Send> Job for Task<T> {
    fn immediate(&self) -> bool {
        self.immediate
    }

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

pub(crate) struct Writer {
    jobs: Option<mpsc::SyncSender<Box<dyn Job>>>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Writer {
    pub fn start(database: Arc<Database>) -> Self {
        let (jobs, queue) = mpsc::sync_channel(QUEUE);
        let thread = std::thread::Builder::new()
            .name("axionomy-store-writer".into())
            .spawn(move || write(&database, &queue))
            .expect("the writer thread starts");
        Self {
            jobs: Some(jobs),
            thread: Some(thread),
        }
    }

    /// Queues `work`, waiting while the queue is full. With `immediate`, the
    /// transaction that commits it is durable when it returns; otherwise it
    /// reaches disk with a later durable commit, within a second.
    pub fn submit<T: Send + 'static>(&self, immediate: bool, work: Work<T>) -> Pending<T> {
        let promise = Arc::new(Promise::default());
        let task = Task {
            work: Some(work),
            output: None,
            immediate,
            promise: Arc::clone(&promise),
        };
        if let Some(jobs) = &self.jobs {
            // A send fails only when the writer has stopped, and dropping the
            // task then resolves it as closed.
            let _ = jobs.send(Box::new(task));
        }
        Pending { promise }
    }
}

impl Drop for Writer {
    fn drop(&mut self) {
        self.jobs.take();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn write(database: &Database, queue: &mpsc::Receiver<Box<dyn Job>>) {
    let mut known = Known::default();
    let mut last_durable = Instant::now();
    let mut unflushed = false;
    loop {
        let first = if unflushed {
            match queue.recv_timeout(FLUSH_INTERVAL.saturating_sub(last_durable.elapsed())) {
                Ok(job) => job,
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    if flush(database).is_ok() {
                        unflushed = false;
                        last_durable = Instant::now();
                    }
                    continue;
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        } else {
            match queue.recv() {
                Ok(job) => job,
                Err(_) => break,
            }
        };
        let mut group = vec![first];
        while group.len() < GROUP {
            match queue.try_recv() {
                Ok(job) => group.push(job),
                Err(_) => break,
            }
        }
        let durable =
            group.iter().any(|job| job.immediate()) || last_durable.elapsed() >= FLUSH_INTERVAL;
        let result = commit(database, &mut known, &mut group, durable);
        match &result {
            Ok(()) => {
                known.committed();
                if durable {
                    last_durable = Instant::now();
                    unflushed = false;
                } else {
                    unflushed = true;
                }
            }
            Err(_) => known.aborted(),
        }
        for job in group {
            job.complete(result.clone());
        }
    }
    if unflushed {
        let _ = flush(database);
    }
}

fn commit(
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
/// a thread, or await it from async code.
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
