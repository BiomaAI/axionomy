use crate::wire::{WireEconomy, WireReceipt};
use std::{
    collections::HashMap,
    future::Future,
    sync::{Arc, RwLock},
};
use thiserror::Error;

/// An immutable economy and how it was made from its parent.
#[derive(Debug, Clone)]
pub struct Snapshot {
    pub parent_id: Option<String>,
    pub origin: Origin,
    pub economy: Arc<WireEconomy>,
}

#[derive(Debug, Clone)]
pub enum Origin {
    /// Entries added by `axionomy_economy_create` or `axionomy_economy_extend`.
    Authored { entries: usize },
    /// Exchanges the kernel applied in order, with their receipts.
    Exchanged { receipts: Arc<[WireReceipt]> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredSnapshot {
    pub economy_id: String,
    pub deduplicated: bool,
}

/// Immutable snapshot storage.
///
/// A handle is derived from the parent's handle and the bytes of the operation
/// that made the snapshot, so retrying an operation returns the same handle
/// without storing the economy again. Implementations must never replace the
/// snapshot behind a live handle, and should make handles infeasible to guess
/// without the server, since an unauthenticated caller holding a handle can
/// read the economy.
pub trait SnapshotStore: Clone + Send + Sync + 'static {
    type Error: std::error::Error + Send + Sync + 'static;

    fn put<'a>(
        &'a self,
        operation: &'a [u8],
        snapshot: Snapshot,
    ) -> impl Future<Output = Result<StoredSnapshot, Self::Error>> + Send + 'a;

    fn get<'a>(
        &'a self,
        economy_id: &'a str,
    ) -> impl Future<Output = Result<Option<Arc<Snapshot>>, Self::Error>> + Send + 'a;
}

#[derive(Debug, Error)]
pub enum MemorySnapshotStoreError {
    #[error("memory snapshot store lock is poisoned")]
    Poisoned,
}

/// Process-local snapshots under handles keyed with a secret drawn from the
/// operating system when the store is created. Clones share one store; a new
/// store or a restarted process resolves none of the old handles.
#[derive(Debug, Clone)]
pub struct MemorySnapshotStore {
    key: Arc<[u8; 32]>,
    snapshots: Arc<RwLock<HashMap<String, Arc<Snapshot>>>>,
}

impl Default for MemorySnapshotStore {
    fn default() -> Self {
        Self {
            key: Arc::new(secret_key()),
            snapshots: Arc::default(),
        }
    }
}

impl MemorySnapshotStore {
    fn handle(&self, parent_id: Option<&str>, operation: &[u8]) -> String {
        let mut hasher = blake3::Hasher::new_keyed(&self.key);
        hasher.update(parent_id.unwrap_or_default().as_bytes());
        hasher.update(&[0]);
        hasher.update(operation);
        format!("eco_{}", hasher.finalize().to_hex())
    }
}

impl SnapshotStore for MemorySnapshotStore {
    type Error = MemorySnapshotStoreError;

    async fn put(
        &self,
        operation: &[u8],
        snapshot: Snapshot,
    ) -> Result<StoredSnapshot, Self::Error> {
        let economy_id = self.handle(snapshot.parent_id.as_deref(), operation);
        let mut snapshots = self
            .snapshots
            .write()
            .map_err(|_| MemorySnapshotStoreError::Poisoned)?;
        let deduplicated = snapshots.contains_key(&economy_id);
        if !deduplicated {
            snapshots.insert(economy_id.clone(), Arc::new(snapshot));
        }
        Ok(StoredSnapshot {
            economy_id,
            deduplicated,
        })
    }

    async fn get(&self, economy_id: &str) -> Result<Option<Arc<Snapshot>>, Self::Error> {
        let snapshots = self
            .snapshots
            .read()
            .map_err(|_| MemorySnapshotStoreError::Poisoned)?;
        Ok(snapshots.get(economy_id).cloned())
    }
}

/// 32 bytes from the operating system's random source.
///
/// # Panics
///
/// If the operating system provides no randomness.
pub(crate) fn secret_key() -> [u8; 32] {
    let mut key = [0; 32];
    getrandom::fill(&mut key).expect("operating system randomness is unavailable");
    key
}

#[cfg(test)]
mod tests {
    use super::*;
    use axionomy::EconomyBuilder;

    fn authored() -> Snapshot {
        Snapshot {
            parent_id: None,
            origin: Origin::Authored { entries: 0 },
            economy: Arc::new(EconomyBuilder::new().build().unwrap()),
        }
    }

    #[tokio::test]
    async fn the_same_operation_on_the_same_parent_returns_one_handle() {
        let store = MemorySnapshotStore::default();
        let first = store.put(b"create", authored()).await.unwrap();
        let second = store.put(b"create", authored()).await.unwrap();
        let other = store.put(b"other", authored()).await.unwrap();

        assert_eq!(first.economy_id, second.economy_id);
        assert!(!first.deduplicated);
        assert!(second.deduplicated);
        assert_ne!(first.economy_id, other.economy_id);
    }

    #[tokio::test]
    async fn handles_depend_on_the_store_secret() {
        let first = MemorySnapshotStore::default();
        let second = MemorySnapshotStore::default();
        let left = first.put(b"create", authored()).await.unwrap();
        let right = second.put(b"create", authored()).await.unwrap();

        assert_ne!(left.economy_id, right.economy_id);
        assert!(second.get(&left.economy_id).await.unwrap().is_none());
    }
}
