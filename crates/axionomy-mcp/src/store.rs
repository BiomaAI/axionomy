use crate::wire::WireEconomy;
use std::{
    collections::HashMap,
    future::Future,
    sync::{Arc, RwLock},
};
use thiserror::Error;

/// Result of storing an immutable economy snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredSnapshot {
    pub economy_id: String,
    pub deduplicated: bool,
}

/// Caller-provided storage for immutable, content-addressed economy snapshots.
///
/// Implementations define the lifetime and location of snapshot handles. A
/// handle must always resolve to the same snapshot while it remains available;
/// replacing the contents behind an existing handle would violate the MCP
/// adapter's immutable-snapshot contract.
pub trait SnapshotStore: Clone + Send + Sync + 'static {
    type Error: std::error::Error + Send + Sync + 'static;

    fn put(
        &self,
        economy: WireEconomy,
    ) -> impl Future<Output = Result<StoredSnapshot, Self::Error>> + Send + '_;

    fn get<'a>(
        &'a self,
        economy_id: &'a str,
    ) -> impl Future<Output = Result<Option<Arc<WireEconomy>>, Self::Error>> + Send + 'a;
}

#[derive(Debug, Error)]
pub enum MemorySnapshotStoreError {
    #[error("economy snapshot could not be serialized: {0}")]
    Json(#[from] serde_json::Error),
    #[error("memory snapshot store lock is poisoned")]
    Poisoned,
    #[error("content-addressed snapshot hash collision for `{economy_id}`")]
    HashCollision { economy_id: String },
}

#[derive(Debug)]
struct SnapshotEntry {
    encoded: Arc<[u8]>,
    economy: Arc<WireEconomy>,
}

/// Process-local snapshot storage for the reference MCP server.
///
/// Clones share the same snapshots. Dropping the last clone releases every
/// snapshot and invalidates its handles, which makes process lifetime an
/// explicit property rather than an accidental persistence guarantee.
#[derive(Debug, Clone, Default)]
pub struct MemorySnapshotStore {
    snapshots: Arc<RwLock<HashMap<String, SnapshotEntry>>>,
}

impl SnapshotStore for MemorySnapshotStore {
    type Error = MemorySnapshotStoreError;

    async fn put(&self, economy: WireEconomy) -> Result<StoredSnapshot, Self::Error> {
        let encoded = Arc::<[u8]>::from(serde_json::to_vec(&economy)?);
        let economy_id = format!("eco_{}", blake3::hash(&encoded).to_hex());
        let mut snapshots = self
            .snapshots
            .write()
            .map_err(|_| MemorySnapshotStoreError::Poisoned)?;

        if let Some(existing) = snapshots.get(&economy_id) {
            if existing.encoded != encoded {
                return Err(MemorySnapshotStoreError::HashCollision { economy_id });
            }
            Ok(StoredSnapshot {
                economy_id,
                deduplicated: true,
            })
        } else {
            snapshots.insert(
                economy_id.clone(),
                SnapshotEntry {
                    encoded,
                    economy: Arc::new(economy),
                },
            );
            Ok(StoredSnapshot {
                economy_id,
                deduplicated: false,
            })
        }
    }

    async fn get(&self, economy_id: &str) -> Result<Option<Arc<WireEconomy>>, Self::Error> {
        let snapshots = self
            .snapshots
            .read()
            .map_err(|_| MemorySnapshotStoreError::Poisoned)?;
        Ok(snapshots
            .get(economy_id)
            .map(|entry| Arc::clone(&entry.economy)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axionomy::{Account, Basket, EconomyBuilder, Quantity};

    fn economy_with_balance(balance: u64) -> WireEconomy {
        let assets: Basket<String> = [("token".to_owned(), Quantity::new(balance))]
            .into_iter()
            .collect();
        EconomyBuilder::<String, String, String, String>::new()
            .account("holder".to_owned(), Account::new(assets))
            .build()
            .unwrap()
    }

    #[tokio::test]
    async fn identical_snapshots_deduplicate_to_one_handle() {
        let store = MemorySnapshotStore::default();
        let first = store.put(economy_with_balance(1)).await.unwrap();
        let second = store.put(economy_with_balance(1)).await.unwrap();

        assert_eq!(first.economy_id, second.economy_id);
        assert!(!first.deduplicated);
        assert!(second.deduplicated);
    }

    #[tokio::test]
    async fn different_snapshots_have_different_handles() {
        let store = MemorySnapshotStore::default();
        let first = store.put(economy_with_balance(1)).await.unwrap();
        let second = store.put(economy_with_balance(2)).await.unwrap();

        assert_ne!(first.economy_id, second.economy_id);
    }

    #[tokio::test]
    async fn clones_share_snapshots_but_fresh_stores_do_not() {
        let store = MemorySnapshotStore::default();
        let stored = store.put(economy_with_balance(1)).await.unwrap();

        assert!(
            store
                .clone()
                .get(&stored.economy_id)
                .await
                .unwrap()
                .is_some()
        );
        assert!(
            MemorySnapshotStore::default()
                .get(&stored.economy_id)
                .await
                .unwrap()
                .is_none()
        );
    }
}
