//! The writer where threads run: a thread of its own that commits many
//! callers' batches per transaction, and makes batches nobody waits on
//! durable within a second.

use super::{Job, Known, Pending, Work, commit, flush, job};
use redb::Database;
use std::{
    sync::{Arc, mpsc},
    time::{Duration, Instant},
};

/// Batches a caller does not wait for reach disk at least this often.
const FLUSH_INTERVAL: Duration = Duration::from_secs(1);
/// The most batches one transaction commits.
const GROUP: usize = 256;
/// Batches queued before callers wait: backpressure instead of memory.
const QUEUE: usize = 64;

pub(crate) struct Writer {
    jobs: Option<mpsc::SyncSender<Queued>>,
    thread: Option<std::thread::JoinHandle<()>>,
}

/// A job, and whether its caller waits for it to be durable.
struct Queued {
    immediate: bool,
    job: Box<dyn Job>,
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
        let (job, pending) = job(work);
        if let Some(jobs) = &self.jobs {
            // A send fails only when the writer has stopped, and dropping the
            // job then resolves it as closed.
            let _ = jobs.send(Queued { immediate, job });
        }
        pending
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

fn write(database: &Database, queue: &mpsc::Receiver<Queued>) {
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
        let mut immediate = first.immediate;
        let mut group = vec![first.job];
        while group.len() < GROUP {
            match queue.try_recv() {
                Ok(queued) => {
                    immediate |= queued.immediate;
                    group.push(queued.job);
                }
                Err(_) => break,
            }
        }
        let durable = immediate || last_durable.elapsed() >= FLUSH_INTERVAL;
        if commit(database, &mut known, group, durable) {
            if durable {
                last_durable = Instant::now();
                unflushed = false;
            } else {
                unflushed = true;
            }
        }
    }
    if unflushed {
        let _ = flush(database);
    }
}
