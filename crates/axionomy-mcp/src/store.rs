use crate::wire::WireEconomy;
use jiff::Timestamp;
use rmcp::model::{DetailedTask, JsonObject, Task, TaskPayload, TaskStatus};
use serde_json::json;
use std::path::Path;
use thiserror::Error;
use tokio_rusqlite::{Connection, params, rusqlite::OptionalExtension};
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("could not open SQLite store: {0}")]
    Open(#[from] tokio_rusqlite::rusqlite::Error),
    #[error("SQLite operation failed: {0}")]
    Database(#[from] tokio_rusqlite::Error),
    #[error("stored JSON is invalid: {0}")]
    Json(#[from] serde_json::Error),
    #[error("idempotency key was already used with different search parameters")]
    IdempotencyConflict,
    #[error("stored task contains invalid data: {0}")]
    InvalidTask(String),
}

#[derive(Debug, Clone)]
pub struct PutEconomy {
    pub economy_id: String,
    pub deduplicated: bool,
}

#[derive(Debug, Clone)]
pub struct CreatedTask {
    pub task: Task,
    pub created: bool,
}

#[derive(Debug)]
struct RawTask {
    task_id: String,
    status: String,
    status_message: Option<String>,
    created_at: String,
    last_updated_at: String,
    ttl_ms: Option<i64>,
    poll_interval_ms: Option<i64>,
    result_json: Option<String>,
    error_json: Option<String>,
}

enum RawTaskCreation {
    Created(RawTask),
    Existing(RawTask),
    Conflict,
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
        let recovered_at = Timestamp::now().to_string();
        let recovery_error = json!({
            "code": -32603,
            "message": "server restarted before the task completed"
        })
        .to_string();
        self.connection
            .call(move |connection| {
                connection.execute_batch(
                    "PRAGMA foreign_keys = ON;
                     CREATE TABLE IF NOT EXISTS economies (
                         economy_id TEXT PRIMARY KEY,
                         economy_json TEXT NOT NULL,
                         created_at TEXT NOT NULL
                     );
                     CREATE TABLE IF NOT EXISTS tasks (
                         task_id TEXT PRIMARY KEY,
                         request_kind TEXT NOT NULL,
                         request_json TEXT NOT NULL,
                         request_hash TEXT NOT NULL,
                         idempotency_key TEXT UNIQUE,
                         status TEXT NOT NULL,
                         status_message TEXT,
                         created_at TEXT NOT NULL,
                         last_updated_at TEXT NOT NULL,
                         ttl_ms INTEGER,
                         poll_interval_ms INTEGER,
                         result_json TEXT,
                         error_json TEXT,
                         cancel_requested INTEGER NOT NULL DEFAULT 0
                     );",
                )?;
                connection.execute(
                    "UPDATE tasks
                     SET status = 'failed', status_message = ?1, last_updated_at = ?2,
                         error_json = ?3
                     WHERE status = 'working'",
                    params![
                        "server restarted before the task completed",
                        recovered_at,
                        recovery_error
                    ],
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

    pub async fn create_search_task(
        &self,
        request_json: String,
        idempotency_key: Option<String>,
    ) -> Result<CreatedTask, StoreError> {
        let request_hash = blake3::hash(request_json.as_bytes()).to_hex().to_string();
        let task_id = Uuid::new_v4().to_string();
        let now = Timestamp::now().to_string();
        let created_task_id = task_id.clone();
        let created_at = now.clone();
        let creation = self
            .connection
            .call(move |connection| {
                if let Some(key) = idempotency_key.as_deref()
                    && let Some((existing, existing_hash)) = connection
                        .query_row(
                            "SELECT task_id, status, status_message, created_at,
                                    last_updated_at, ttl_ms, poll_interval_ms, result_json,
                                    error_json, request_hash
                             FROM tasks WHERE idempotency_key = ?1",
                            params![key],
                            |row| Ok((raw_task_from_row(row)?, row.get::<_, String>(9)?)),
                        )
                        .optional()?
                {
                    return if existing_hash == request_hash {
                        Ok(RawTaskCreation::Existing(existing))
                    } else {
                        Ok(RawTaskCreation::Conflict)
                    };
                }

                connection.execute(
                    "INSERT INTO tasks (
                         task_id, request_kind, request_json, request_hash, idempotency_key,
                         status, status_message, created_at, last_updated_at, poll_interval_ms
                     ) VALUES (?1, 'bfs', ?2, ?3, ?4, 'working', 'queued', ?5, ?5, 100)",
                    params![
                        created_task_id,
                        request_json,
                        request_hash,
                        idempotency_key,
                        created_at
                    ],
                )?;
                Ok(RawTaskCreation::Created(RawTask {
                    task_id,
                    status: "working".to_owned(),
                    status_message: Some("queued".to_owned()),
                    created_at: now.clone(),
                    last_updated_at: now,
                    ttl_ms: None,
                    poll_interval_ms: Some(100),
                    result_json: None,
                    error_json: None,
                }))
            })
            .await?;

        let (raw, created) = match creation {
            RawTaskCreation::Created(raw) => (raw, true),
            RawTaskCreation::Existing(raw) => (raw, false),
            RawTaskCreation::Conflict => return Err(StoreError::IdempotencyConflict),
        };
        let detailed = detailed_task_from_raw(raw)?;
        Ok(CreatedTask {
            task: detailed.task,
            created,
        })
    }

    pub async fn get_task(&self, task_id: &str) -> Result<Option<DetailedTask>, StoreError> {
        let task_id = task_id.to_owned();
        let raw = self
            .connection
            .call(move |connection| {
                connection
                    .query_row(
                        "SELECT task_id, status, status_message, created_at, last_updated_at,
                                ttl_ms, poll_interval_ms, result_json, error_json
                         FROM tasks WHERE task_id = ?1",
                        params![task_id],
                        raw_task_from_row,
                    )
                    .optional()
            })
            .await?;
        raw.map(detailed_task_from_raw).transpose()
    }

    pub async fn task_exists(&self, task_id: &str) -> Result<bool, StoreError> {
        Ok(self.get_task(task_id).await?.is_some())
    }

    pub async fn request_cancellation(&self, task_id: &str) -> Result<bool, StoreError> {
        let task_id = task_id.to_owned();
        let now = Timestamp::now().to_string();
        let changed = self
            .connection
            .call(move |connection| {
                connection.execute(
                    "UPDATE tasks SET cancel_requested = 1, last_updated_at = ?2
                     WHERE task_id = ?1",
                    params![task_id, now],
                )
            })
            .await?;
        Ok(changed == 1)
    }

    pub async fn cancellation_requested(&self, task_id: &str) -> Result<bool, StoreError> {
        let task_id = task_id.to_owned();
        let requested = self
            .connection
            .call(move |connection| {
                connection
                    .query_row(
                        "SELECT cancel_requested FROM tasks WHERE task_id = ?1",
                        params![task_id],
                        |row| row.get::<_, bool>(0),
                    )
                    .optional()
            })
            .await?;
        Ok(requested.unwrap_or(true))
    }

    pub async fn update_task_progress(
        &self,
        task_id: &str,
        status_message: String,
    ) -> Result<(), StoreError> {
        let task_id = task_id.to_owned();
        let now = Timestamp::now().to_string();
        self.connection
            .call(move |connection| {
                connection.execute(
                    "UPDATE tasks SET status_message = ?2, last_updated_at = ?3
                     WHERE task_id = ?1 AND status = 'working'",
                    params![task_id, status_message, now],
                )?;
                Ok(())
            })
            .await?;
        Ok(())
    }

    pub async fn complete_task(
        &self,
        task_id: &str,
        status_message: String,
        result: JsonObject,
    ) -> Result<(), StoreError> {
        self.settle_task(
            task_id,
            "completed",
            status_message,
            Some(serde_json::to_string(&result)?),
            None,
        )
        .await
    }

    pub async fn fail_task(&self, task_id: &str, message: String) -> Result<(), StoreError> {
        let error = json!({ "code": -32603, "message": message });
        self.settle_task(
            task_id,
            "failed",
            "search failed".to_owned(),
            None,
            Some(serde_json::to_string(&error)?),
        )
        .await
    }

    pub async fn cancel_task(&self, task_id: &str) -> Result<(), StoreError> {
        self.settle_task(
            task_id,
            "cancelled",
            "search cancelled".to_owned(),
            None,
            None,
        )
        .await
    }

    async fn settle_task(
        &self,
        task_id: &str,
        status: &'static str,
        status_message: String,
        result_json: Option<String>,
        error_json: Option<String>,
    ) -> Result<(), StoreError> {
        let task_id = task_id.to_owned();
        let now = Timestamp::now().to_string();
        self.connection
            .call(move |connection| {
                connection.execute(
                    "UPDATE tasks
                     SET status = ?2, status_message = ?3, last_updated_at = ?4,
                         result_json = ?5, error_json = ?6
                     WHERE task_id = ?1 AND status = 'working'",
                    params![
                        task_id,
                        status,
                        status_message,
                        now,
                        result_json,
                        error_json
                    ],
                )?;
                Ok(())
            })
            .await?;
        Ok(())
    }
}

fn raw_task_from_row(
    row: &tokio_rusqlite::rusqlite::Row<'_>,
) -> tokio_rusqlite::rusqlite::Result<RawTask> {
    Ok(RawTask {
        task_id: row.get(0)?,
        status: row.get(1)?,
        status_message: row.get(2)?,
        created_at: row.get(3)?,
        last_updated_at: row.get(4)?,
        ttl_ms: row.get(5)?,
        poll_interval_ms: row.get(6)?,
        result_json: row.get(7)?,
        error_json: row.get(8)?,
    })
}

fn detailed_task_from_raw(raw: RawTask) -> Result<DetailedTask, StoreError> {
    let status = match raw.status.as_str() {
        "working" => TaskStatus::Working,
        "input_required" => TaskStatus::InputRequired,
        "completed" => TaskStatus::Completed,
        "failed" => TaskStatus::Failed,
        "cancelled" => TaskStatus::Cancelled,
        value => return Err(StoreError::InvalidTask(format!("unknown status `{value}`"))),
    };
    let mut task = Task::new(raw.task_id, status, raw.created_at, raw.last_updated_at);
    task.status_message = raw.status_message;
    task.ttl_ms = optional_u64(raw.ttl_ms, "ttl_ms")?;
    task.poll_interval_ms = optional_u64(raw.poll_interval_ms, "poll_interval_ms")?;
    let payload = match status {
        TaskStatus::Working => TaskPayload::Working,
        TaskStatus::Completed => TaskPayload::Completed {
            result: parse_object(raw.result_json, "completed result")?,
        },
        TaskStatus::Failed => TaskPayload::Failed {
            error: parse_object(raw.error_json, "failure error")?,
        },
        TaskStatus::Cancelled => TaskPayload::Cancelled,
        TaskStatus::InputRequired => {
            return Err(StoreError::InvalidTask(
                "input_required is not used by Axionomy tasks".to_owned(),
            ));
        }
        _ => {
            return Err(StoreError::InvalidTask(
                "unsupported task status".to_owned(),
            ));
        }
    };
    Ok(DetailedTask::new(task, payload))
}

fn optional_u64(value: Option<i64>, field: &str) -> Result<Option<u64>, StoreError> {
    value
        .map(|value| {
            u64::try_from(value).map_err(|_| {
                StoreError::InvalidTask(format!("{field} contains a negative integer"))
            })
        })
        .transpose()
}

fn parse_object(json: Option<String>, label: &str) -> Result<JsonObject, StoreError> {
    let value: serde_json::Value = serde_json::from_str(
        json.as_deref()
            .ok_or_else(|| StoreError::InvalidTask(format!("missing {label}")))?,
    )?;
    value
        .as_object()
        .cloned()
        .ok_or_else(|| StoreError::InvalidTask(format!("{label} is not a JSON object")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axionomy::EconomyBuilder;
    use rmcp::model::TaskPayload;
    use serde_json::json;

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

    #[tokio::test]
    async fn search_task_retries_are_idempotent() {
        let store = SqliteStore::open_in_memory().await.unwrap();
        let first = store
            .create_search_task("{\"goal\":1}".to_owned(), Some("retry-key".to_owned()))
            .await
            .unwrap();
        let retry = store
            .create_search_task("{\"goal\":1}".to_owned(), Some("retry-key".to_owned()))
            .await
            .unwrap();

        assert!(first.created);
        assert!(!retry.created);
        assert_eq!(first.task.task_id, retry.task.task_id);
        assert!(matches!(
            store
                .create_search_task("{\"goal\":2}".to_owned(), Some("retry-key".to_owned()))
                .await,
            Err(StoreError::IdempotencyConflict)
        ));
    }

    #[tokio::test]
    async fn terminal_task_results_are_durable() {
        let store = SqliteStore::open_in_memory().await.unwrap();
        let created = store
            .create_search_task("{}".to_owned(), None)
            .await
            .unwrap();
        store
            .update_task_progress(&created.task.task_id, "expanded=4".to_owned())
            .await
            .unwrap();
        store
            .complete_task(
                &created.task.task_id,
                "finished".to_owned(),
                json!({ "answer": 42 }).as_object().unwrap().clone(),
            )
            .await
            .unwrap();

        let detailed = store
            .get_task(&created.task.task_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(detailed.task.status_message.as_deref(), Some("finished"));
        assert!(matches!(
            detailed.payload,
            TaskPayload::Completed { result } if result["answer"] == 42
        ));
    }

    #[tokio::test]
    async fn restart_turns_abandoned_work_into_an_explicit_failure() {
        let database = tempfile::NamedTempFile::new().unwrap();
        let task_id = {
            let store = SqliteStore::open(database.path()).await.unwrap();
            store
                .create_search_task("{}".to_owned(), None)
                .await
                .unwrap()
                .task
                .task_id
        };

        let reopened = SqliteStore::open(database.path()).await.unwrap();
        let detailed = reopened.get_task(&task_id).await.unwrap().unwrap();
        assert!(matches!(detailed.payload, TaskPayload::Failed { .. }));
        assert_eq!(
            detailed.task.status_message.as_deref(),
            Some("server restarted before the task completed")
        );
    }
}
