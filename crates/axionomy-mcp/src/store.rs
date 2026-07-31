use crate::wire::WireEconomy;
use jiff::Timestamp;
use std::path::Path;
use thiserror::Error;
use tokio_rusqlite::{Connection, params, rusqlite::OptionalExtension};

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("could not open SQLite store: {0}")]
    Open(#[from] tokio_rusqlite::rusqlite::Error),
    #[error("SQLite operation failed: {0}")]
    Database(#[from] tokio_rusqlite::Error),
    #[error("stored JSON is invalid: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Clone)]
pub struct PutEconomy {
    pub economy_id: String,
    pub deduplicated: bool,
}

#[derive(Clone)]
pub struct SqliteStore {
    connection: Connection,
}

impl std::fmt::Debug for SqliteStore {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SqliteStore")
            .finish_non_exhaustive()
    }
}

impl SqliteStore {
    pub async fn open(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        let connection = Connection::open(path).await?;
        let store = Self { connection };
        store.initialize().await?;
        Ok(store)
    }

    pub async fn open_in_memory() -> Result<Self, StoreError> {
        let connection = Connection::open_in_memory().await?;
        let store = Self { connection };
        store.initialize().await?;
        Ok(store)
    }

    async fn initialize(&self) -> Result<(), StoreError> {
        self.connection
            .call(|connection| {
                connection.execute_batch(
                    "PRAGMA foreign_keys = ON;
                     CREATE TABLE IF NOT EXISTS economies (
                         economy_id TEXT PRIMARY KEY,
                         economy_json TEXT NOT NULL,
                         created_at TEXT NOT NULL
                     );",
                )?;
                Ok(())
            })
            .await?;
        Ok(())
    }

    pub async fn put_economy(&self, economy: &WireEconomy) -> Result<PutEconomy, StoreError> {
        let economy_json = serde_json::to_string(economy)?;
        let economy_id = format!("eco_{}", blake3::hash(economy_json.as_bytes()).to_hex());
        let created_at = Timestamp::now().to_string();
        let stored_id = economy_id.clone();
        let inserted = self
            .connection
            .call(move |connection| {
                let changed = connection.execute(
                    "INSERT OR IGNORE INTO economies (economy_id, economy_json, created_at)
                     VALUES (?1, ?2, ?3)",
                    params![stored_id, economy_json, created_at],
                )?;
                Ok(changed == 1)
            })
            .await?;
        Ok(PutEconomy {
            economy_id,
            deduplicated: !inserted,
        })
    }

    pub async fn get_economy(&self, economy_id: &str) -> Result<Option<WireEconomy>, StoreError> {
        let economy_id = economy_id.to_owned();
        let json = self
            .connection
            .call(move |connection| {
                connection
                    .query_row(
                        "SELECT economy_json FROM economies WHERE economy_id = ?1",
                        params![economy_id],
                        |row| row.get::<_, String>(0),
                    )
                    .optional()
            })
            .await?;
        json.map(|json| serde_json::from_str(&json))
            .transpose()
            .map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axionomy::EconomyBuilder;

    #[tokio::test]
    async fn economies_are_immutable_and_content_addressed() {
        let store = SqliteStore::open_in_memory().await.unwrap();
        let economy = EconomyBuilder::<String, String, String, String>::new()
            .build()
            .unwrap();

        let first = store.put_economy(&economy).await.unwrap();
        let second = store.put_economy(&economy).await.unwrap();

        assert_eq!(first.economy_id, second.economy_id);
        assert!(!first.deduplicated);
        assert!(second.deduplicated);
        assert!(
            store
                .get_economy(&first.economy_id)
                .await
                .unwrap()
                .is_some()
        );
    }
}
