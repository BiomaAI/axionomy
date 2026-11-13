//! The writer where there are no threads, as in a browser: each batch
//! commits on its caller, in a transaction of its own, before `transact`
//! returns. With no thread there is no timer either, so a batch nobody waits
//! on is durable with the next durable commit, or when the store closes.

use super::{Known, Pending, Work, commit, flush, job};
use redb::Database;
use std::sync::{Arc, Mutex};

pub(crate) struct Writer {
    database: Arc<Database>,
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    known: Known,
    /// Whether a commit since the last durable one is not durable yet.
    unflushed: bool,
}

impl Writer {
    pub fn start(database: Arc<Database>) -> Self {
        Self {
            database,
            state: Mutex::default(),
        }
    }

    /// Commits `work` now. With `immediate`, the commit is durable, and so is
    /// every commit before it; otherwise it is durable with the next durable
    /// commit, or when the writer stops.
    pub fn submit<T: Send + 'static>(&self, immediate: bool, work: Work<T>) -> Pending<T> {
        let (task, pending) = job(work);
        let mut state = self.state.lock().expect("writer lock");
        if commit(&self.database, &mut state.known, vec![task], immediate) {
            state.unflushed = !immediate;
        }
        pending
    }
}

impl Drop for Writer {
    fn drop(&mut self) {
        let state = self.state.get_mut().expect("writer lock");
        if state.unflushed {
            let _ = flush(&self.database);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Writer;
    use crate::StoreError;
    use redb::{
        Database, ReadableDatabase, StorageBackend, TableDefinition, backends::InMemoryBackend,
    };
    use std::{
        io,
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
    };

    const TABLE: TableDefinition<u64, u64> = TableDefinition::new("numbers");

    /// Memory that counts how often redb asks it to be durable.
    #[derive(Debug, Default)]
    struct Counted {
        bytes: InMemoryBackend,
        syncs: Arc<AtomicUsize>,
    }

    impl StorageBackend for Counted {
        fn len(&self) -> Result<u64, io::Error> {
            self.bytes.len()
        }

        fn read(&self, offset: u64, out: &mut [u8]) -> Result<(), io::Error> {
            self.bytes.read(offset, out)
        }

        fn set_len(&self, len: u64) -> Result<(), io::Error> {
            self.bytes.set_len(len)
        }

        fn sync_data(&self) -> Result<(), io::Error> {
            self.syncs.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        fn write(&self, offset: u64, data: &[u8]) -> Result<(), io::Error> {
            self.bytes.write(offset, data)
        }
    }

    fn writer() -> (Writer, Arc<Database>, Arc<AtomicUsize>) {
        let backend = Counted::default();
        let syncs = Arc::clone(&backend.syncs);
        let database = Arc::new(Database::builder().create_with_backend(backend).unwrap());
        (Writer::start(Arc::clone(&database)), database, syncs)
    }

    fn put(writer: &Writer, immediate: bool, key: u64) -> crate::Pending<u64> {
        writer.submit(
            immediate,
            Box::new(move |txn| {
                txn.redb().open_table(TABLE)?.insert(key, key * 10)?;
                Ok(key)
            }),
        )
    }

    fn held(database: &Database, key: u64) -> Option<u64> {
        let txn = database.begin_read().unwrap();
        let table = txn.open_table(TABLE).ok()?;
        table.get(key).unwrap().map(|value| value.value())
    }

    #[test]
    fn work_commits_before_submit_returns() {
        let (writer, database, _) = writer();
        let pending = put(&writer, false, 1);
        assert!(matches!(pending.try_take(), Some(Ok(1))));
        assert_eq!(held(&database, 1), Some(10));
    }

    #[test]
    fn an_immediate_commit_makes_the_eventual_ones_before_it_durable() {
        // The database stays open: closing it writes too.
        let (writer, _database, syncs) = writer();
        let before = syncs.load(Ordering::SeqCst);
        put(&writer, false, 1).wait().unwrap();
        put(&writer, false, 2).wait().unwrap();
        assert_eq!(
            syncs.load(Ordering::SeqCst),
            before,
            "eventual commits wait"
        );
        put(&writer, true, 3).wait().unwrap();
        let durable = syncs.load(Ordering::SeqCst);
        assert!(durable > before, "an immediate commit is durable");
        // Nothing is left to flush, so stopping writes nothing.
        drop(writer);
        assert_eq!(syncs.load(Ordering::SeqCst), durable);
    }

    #[test]
    fn stopping_makes_the_last_eventual_commits_durable() {
        let (writer, _database, syncs) = writer();
        put(&writer, false, 1).wait().unwrap();
        let before = syncs.load(Ordering::SeqCst);
        drop(writer);
        assert!(syncs.load(Ordering::SeqCst) > before);
    }

    #[test]
    fn failed_work_writes_nothing_and_later_work_commits() {
        let (writer, database, _) = writer();
        let failed = writer.submit(
            true,
            Box::new(|txn| {
                txn.redb().open_table(TABLE)?.insert(1, 1)?;
                Err::<(), _>(StoreError::Closed)
            }),
        );
        assert!(matches!(failed.wait(), Err(StoreError::Closed)));
        assert_eq!(held(&database, 1), None);
        assert_eq!(put(&writer, true, 2).wait().unwrap(), 2);
        assert_eq!(held(&database, 2), Some(20));
    }
}
