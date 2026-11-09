//! What the server keeps, in an [`axionomy_store::Store`]: economies by
//! handle, assessments and run artifacts.
//!
//! Every economy is a point of a run. Authoring records a run that starts
//! from the authored state; applying exchanges records a run that forks from
//! the source economy's point, with one step per exchange. A handle, its
//! run's steps and the states they store commit in one transaction, so a
//! handle exists if and only if everything it names does. Handles are keyed
//! with a secret the store keeps, so they stay valid across restarts of a
//! server that keeps its store on disk, and stay infeasible to guess.

use crate::handles::{ArtifactId, AssessmentId, EconomyId};
use axionomy::paths;
use axionomy::paths::Paths;
use axionomy_service::RunArtifact;
use axionomy_store::{Checkpoints, Durability, Point, Reader, Store, StoreError};

/// A snapshot's state: MCP's economies are identified by paths.
pub(crate) type State = axionomy_store::State<Paths>;
use redb::{ReadableTable, TableDefinition};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::{
    collections::HashMap,
    hash::Hash,
    num::NonZeroU64,
    sync::{Arc, Mutex},
};

const ECONOMIES: TableDefinition<&str, &[u8]> = TableDefinition::new("mcp-economies");
const ASSESSMENTS: TableDefinition<&str, &[u8]> = TableDefinition::new("mcp-assessments");
const ARTIFACTS: TableDefinition<&str, &[u8]> = TableDefinition::new("mcp-artifacts");
const SECRETS: TableDefinition<&str, &[u8; 32]> = TableDefinition::new("mcp-secrets");
const HANDLE_SECRET: &str = "handles";

/// States a call's run stores besides its last: replaying to any step of a
/// long replay starts at most this many exchanges back.
const CHECKPOINT_EVERY: NonZeroU64 = NonZeroU64::new(64).expect("nonzero");
/// Loaded economies, assessments and artifacts kept in memory.
const CACHED: usize = 32;

/// How a stored economy was made from its parent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum Origin {
    /// Entries added by `axionomy_economy_create` or `axionomy_economy_extend`.
    Authored { entries: usize },
    /// Exchanges the kernel applied in order.
    Exchanged { exchanges: usize },
}

/// What a handle names.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct EconomyRecord {
    pub point: Point,
    pub parent: Option<EconomyId>,
    pub origin: Origin,
}

/// An exchange checked against the state an economy reached after `before`.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct AssessmentRecord {
    economy_id: EconomyId,
    before: Vec<paths::Exchange>,
    exchange: paths::Exchange,
}

/// An economy as the server serves it.
pub(crate) struct Loaded {
    pub record: EconomyRecord,
    pub state: State,
    /// The receipts of the exchanges that made it from its parent.
    pub receipts: Vec<paths::Receipt>,
}

/// An assessment as the server serves it, computed again from its record.
pub(crate) struct Assessed {
    pub economy_id: EconomyId,
    pub after_exchanges: usize,
    pub exchange: paths::Exchange,
    pub assessment: paths::Assessment,
}

/// The server's economies, assessments and artifacts on its store, with the
/// most recently used ones in memory.
#[derive(Clone)]
pub(crate) struct Archive {
    store: Store,
    secret: Arc<[u8; 32]>,
    economies: Cache<EconomyId, Loaded>,
    assessments: Cache<AssessmentId, Assessed>,
    artifacts: Cache<ArtifactId, RunArtifact>,
}

/// Whether a stored economy was stored by this call or an earlier one.
pub(crate) struct Stored {
    pub economy_id: EconomyId,
    pub deduplicated: bool,
}

impl Archive {
    /// Opens the server's tables in `store`, creating its handle secret the
    /// first time.
    pub fn open(store: Store) -> Result<Self, StoreError> {
        let mut fresh = [0; 32];
        getrandom::fill(&mut fresh).expect("operating system randomness is unavailable");
        let secret = store
            .transact(Durability::Immediate, move |txn| {
                let txn = txn.redb();
                txn.open_table(ECONOMIES)?;
                txn.open_table(ASSESSMENTS)?;
                txn.open_table(ARTIFACTS)?;
                let mut secrets = txn.open_table(SECRETS)?;
                let kept = secrets.get(HANDLE_SECRET)?.map(|secret| *secret.value());
                Ok(match kept {
                    Some(secret) => secret,
                    None => {
                        secrets.insert(HANDLE_SECRET, &fresh)?;
                        fresh
                    }
                })
            })
            .wait()?;
        Ok(Self {
            store,
            secret: Arc::new(secret),
            economies: Cache::default(),
            assessments: Cache::default(),
            artifacts: Cache::default(),
        })
    }

    fn keyed(&self, parts: &[&[u8]]) -> blake3::Hash {
        let mut hasher = blake3::Hasher::new_keyed(&self.secret);
        for part in parts {
            hasher.update(&(part.len() as u64).to_le_bytes());
            hasher.update(part);
        }
        hasher.finalize()
    }

    /// The handle of what `operation` makes from `parent`: retrying an
    /// operation names what it stored the first time.
    fn handle(&self, parent: Option<&EconomyId>, operation: &[u8]) -> EconomyId {
        let parent = parent.map(EconomyId::as_str).unwrap_or_default();
        EconomyId::from_digest(&self.keyed(&[parent.as_bytes(), operation]))
    }

    /// Stores an authored economy as a run of its own.
    pub async fn author(
        &self,
        parent: Option<EconomyId>,
        operation: &[u8],
        entries: usize,
        economy: paths::Economy,
    ) -> Result<Stored, StoreError> {
        let economy_id = self.handle(parent.as_ref(), operation);
        if self.record(&economy_id).await?.is_some() {
            return Ok(Stored {
                economy_id,
                deduplicated: true,
            });
        }
        let state = State::new(economy);
        let run = self.store.mint_run();
        let record = EconomyRecord {
            point: Point { run, step: 0 },
            parent,
            origin: Origin::Authored { entries },
        };
        let tree = state.tree.clone();
        let handle = economy_id.clone();
        let stored = self
            .store
            .transact(Durability::Immediate, move |txn| {
                if exists(txn.redb(), &handle)? {
                    return Ok(false);
                }
                txn.create_run::<Paths>(run, None, 0, &tree, Checkpoints::Ends)?;
                txn.pin(record.point, None)?;
                put(txn.redb(), ECONOMIES, handle.as_str(), &record)?;
                Ok(true)
            })
            .await?;
        Ok(Stored {
            economy_id,
            deduplicated: !stored,
        })
    }

    /// Stores the state `exchanges` reached from `source`, which `state` and
    /// `receipts` are, as a run forked from the source's point.
    pub async fn exchanged(
        &self,
        source_id: &EconomyId,
        source: &Loaded,
        operation: &[u8],
        mut steps: Vec<(paths::Receipt, Option<axionomy_tree::Tree>)>,
        state: State,
    ) -> Result<Stored, StoreError> {
        let economy_id = self.handle(Some(source_id), operation);
        if self.record(&economy_id).await?.is_some() {
            return Ok(Stored {
                economy_id,
                deduplicated: true,
            });
        }
        let from = source.record.point;
        let run = self.store.mint_run();
        let exchanges = steps.len();
        let record = EconomyRecord {
            point: Point {
                run,
                step: from.step + exchanges as u64,
            },
            parent: Some(source_id.clone()),
            origin: Origin::Exchanged { exchanges },
        };
        let receipts: Vec<paths::Receipt> =
            steps.iter().map(|(receipt, _)| receipt.clone()).collect();
        let first = source.state.tree.clone();
        let last = state.tree.clone();
        let handle = economy_id.clone();
        let loaded = Loaded {
            record: record.clone(),
            state,
            receipts,
        };
        let stored = self
            .store
            .transact(Durability::Immediate, move |txn| {
                if exists(txn.redb(), &handle)? {
                    return Ok(false);
                }
                txn.create_run::<Paths>(run, Some(from), from.step, &first, every())?;
                for (index, (receipt, state)) in steps.drain(..).enumerate() {
                    let point = Point {
                        run,
                        step: from.step + index as u64 + 1,
                    };
                    txn.put_step::<Paths>(point, &receipt, state.as_ref())?;
                }
                txn.pin(record.point, Some(&last))?;
                put(txn.redb(), ECONOMIES, handle.as_str(), &record)?;
                Ok(true)
            })
            .await?;
        if stored {
            self.economies.insert(economy_id.clone(), loaded);
        }
        Ok(Stored {
            economy_id,
            deduplicated: !stored,
        })
    }

    /// Names the state after the first `step` exchanges of those that made
    /// `economy_id`, storing it so it reads like any economy.
    pub async fn step(
        &self,
        economy_id: &EconomyId,
        loaded: &Loaded,
        step: usize,
    ) -> Result<Stored, StoreError> {
        let handle = EconomyId::from_digest(&self.keyed(&[
            economy_id.as_str().as_bytes(),
            b"step",
            &(step as u64).to_le_bytes(),
        ]));
        if self.record(&handle).await?.is_some() {
            return Ok(Stored {
                economy_id: handle,
                deduplicated: true,
            });
        }
        let point = Point {
            run: loaded.record.point.run,
            step: loaded.record.point.step - (loaded.receipts.len() - step) as u64,
        };
        let record = EconomyRecord {
            point,
            parent: loaded.record.parent.clone(),
            origin: Origin::Exchanged { exchanges: step },
        };
        let store = self.store.clone();
        let state = tokio::task::spawn_blocking(move || store.read()?.state::<Paths>(point))
            .await
            .map_err(|_| StoreError::Closed)??
            .ok_or(StoreError::UnknownRun(point.run))?;
        let key = handle.clone();
        let stored = self
            .store
            .transact(Durability::Immediate, move |txn| {
                if exists(txn.redb(), &key)? {
                    return Ok(false);
                }
                txn.pin(point, Some(&state.tree))?;
                put(txn.redb(), ECONOMIES, key.as_str(), &record)?;
                Ok(true)
            })
            .await?;
        Ok(Stored {
            economy_id: handle,
            deduplicated: !stored,
        })
    }

    async fn record(&self, economy_id: &EconomyId) -> Result<Option<EconomyRecord>, StoreError> {
        if let Some(loaded) = self.economies.get(economy_id) {
            return Ok(Some(loaded.record.clone()));
        }
        let reader = self.store.read()?;
        get(&reader, ECONOMIES, economy_id.as_str())
    }

    /// The economy a handle names, loaded with the receipts that made it.
    pub async fn economy(&self, economy_id: &EconomyId) -> Result<Option<Arc<Loaded>>, StoreError> {
        if let Some(loaded) = self.economies.get(economy_id) {
            return Ok(Some(loaded));
        }
        let store = self.store.clone();
        let handle = economy_id.clone();
        let loaded = tokio::task::spawn_blocking(move || -> Result<_, StoreError> {
            let reader = store.read()?;
            let Some(record) = get::<EconomyRecord>(&reader, ECONOMIES, handle.as_str())? else {
                return Ok(None);
            };
            let point = record.point;
            let (state, receipts) = match record.origin {
                Origin::Authored { .. } => (reader.state::<Paths>(point)?, Vec::new()),
                Origin::Exchanged { exchanges } => {
                    let first = point.step - exchanges as u64;
                    let mut state = reader.state::<Paths>(Point {
                        run: point.run,
                        step: first,
                    })?;
                    let receipts = match &mut state {
                        Some(state) => reader.replay(state, point.run, first, point.step)?,
                        None => Vec::new(),
                    };
                    (state, receipts)
                }
            };
            let state = state.ok_or(StoreError::UnknownRun(point.run))?;
            Ok(Some(Loaded {
                record,
                state,
                receipts,
            }))
        })
        .await
        .map_err(|_| StoreError::Closed)??;
        Ok(loaded.map(|loaded| self.economies.insert(economy_id.clone(), loaded)))
    }

    /// Keeps an assessment of `exchange` against the state `economy_id`
    /// reached after `before`, and returns its handle.
    pub async fn keep_assessment(
        &self,
        economy_id: &EconomyId,
        before: &[paths::Exchange],
        exchange: &paths::Exchange,
        assessment: paths::Assessment,
    ) -> Result<(AssessmentId, Arc<Assessed>), StoreError> {
        let record = AssessmentRecord {
            economy_id: economy_id.clone(),
            before: before.to_vec(),
            exchange: exchange.clone(),
        };
        let assessment_id = AssessmentId::from_digest(&self.keyed(&[
            b"assessment",
            &serde_json::to_vec(&record).expect("records serialize"),
        ]));
        let key = assessment_id.to_string();
        self.store
            .transact(Durability::Eventual, move |txn| {
                put(txn.redb(), ASSESSMENTS, &key, &record)
            })
            .await?;
        let assessed = Assessed {
            economy_id: economy_id.clone(),
            after_exchanges: before.len(),
            exchange: exchange.clone(),
            assessment,
        };
        Ok((
            assessment_id.clone(),
            self.assessments.insert(assessment_id, assessed),
        ))
    }

    pub async fn assessment(
        &self,
        assessment_id: &AssessmentId,
    ) -> Result<Option<Arc<Assessed>>, StoreError> {
        if let Some(assessed) = self.assessments.get(assessment_id) {
            return Ok(Some(assessed));
        }
        let reader = self.store.read()?;
        let Some(record) = get::<AssessmentRecord>(&reader, ASSESSMENTS, assessment_id.as_str())?
        else {
            return Ok(None);
        };
        let Some(loaded) = self.economy(&record.economy_id).await? else {
            return Ok(None);
        };
        let mut economy = loaded.state.economy.clone();
        for exchange in &record.before {
            economy
                .apply(exchange.clone())
                .map_err(|_| StoreError::Malformed("assessment whose exchanges no longer apply"))?;
        }
        let assessed = Assessed {
            assessment: economy.assess(&record.exchange),
            economy_id: record.economy_id,
            after_exchanges: record.before.len(),
            exchange: record.exchange,
        };
        Ok(Some(
            self.assessments.insert(assessment_id.clone(), assessed),
        ))
    }

    /// Keeps a run's artifact under a handle minted from its key, so the same
    /// run always has the same handle.
    pub async fn keep_artifact(&self, artifact: RunArtifact) -> Result<ArtifactId, StoreError> {
        let key = serde_json::to_vec(&artifact.key).expect("artifact keys serialize");
        let id = ArtifactId::from_digest(&self.keyed(&[b"artifact", &key]));
        let bytes = serde_json::to_vec(&artifact).expect("artifacts serialize");
        let stored = id.clone();
        self.store
            .transact(Durability::Eventual, move |txn| {
                txn.redb()
                    .open_table(ARTIFACTS)?
                    .insert(stored.as_str(), bytes.as_slice())?;
                Ok(())
            })
            .await?;
        self.artifacts.insert(id.clone(), artifact);
        Ok(id)
    }

    pub async fn artifact(
        &self,
        artifact_id: &ArtifactId,
    ) -> Result<Option<Arc<RunArtifact>>, StoreError> {
        if let Some(artifact) = self.artifacts.get(artifact_id) {
            return Ok(Some(artifact));
        }
        let store = self.store.clone();
        let key = artifact_id.as_str().to_owned();
        let artifact = tokio::task::spawn_blocking(move || {
            get::<RunArtifact>(&store.read()?, ARTIFACTS, &key)
        })
        .await
        .map_err(|_| StoreError::Closed)??;
        Ok(artifact.map(|artifact| self.artifacts.insert(artifact_id.clone(), artifact)))
    }
}

fn every() -> Checkpoints {
    Checkpoints::Every(CHECKPOINT_EVERY)
}

/// Whether the state after step `step` of a call's run is stored before the
/// call's last state.
pub(crate) fn checkpoint(step: u64) -> bool {
    step.is_multiple_of(CHECKPOINT_EVERY.get())
}

fn exists(txn: &redb::WriteTransaction, economy_id: &EconomyId) -> Result<bool, StoreError> {
    Ok(txn
        .open_table(ECONOMIES)
        .map_err(StoreError::from)?
        .get(economy_id.as_str())
        .map_err(StoreError::from)?
        .is_some())
}

fn put(
    txn: &redb::WriteTransaction,
    table: TableDefinition<&str, &[u8]>,
    key: &str,
    value: &impl Serialize,
) -> Result<(), StoreError> {
    let bytes = serde_json::to_vec(value).expect("records serialize");
    txn.open_table(table)
        .map_err(StoreError::from)?
        .insert(key, bytes.as_slice())
        .map_err(StoreError::from)?;
    Ok(())
}

fn get<T: DeserializeOwned>(
    reader: &Reader,
    table: TableDefinition<&str, &[u8]>,
    key: &str,
) -> Result<Option<T>, StoreError> {
    let table = reader.redb().open_table(table).map_err(StoreError::from)?;
    let Some(bytes) = table.get(key).map_err(StoreError::from)? else {
        return Ok(None);
    };
    serde_json::from_slice(bytes.value())
        .map(Some)
        .map_err(|_| StoreError::Malformed("server record"))
}

/// A few recently used values, shared by clones.
struct Cache<K, V> {
    entries: Arc<Mutex<Entries<K, V>>>,
}

struct Entries<K, V> {
    values: HashMap<K, (Arc<V>, u64)>,
    clock: u64,
}

impl<K, V> Clone for Cache<K, V> {
    fn clone(&self) -> Self {
        Self {
            entries: Arc::clone(&self.entries),
        }
    }
}

impl<K, V> Default for Cache<K, V> {
    fn default() -> Self {
        Self {
            entries: Arc::new(Mutex::new(Entries {
                values: HashMap::new(),
                clock: 0,
            })),
        }
    }
}

impl<K: Eq + Hash + Clone, V> Cache<K, V> {
    fn get<Q>(&self, key: &Q) -> Option<Arc<V>>
    where
        K: std::borrow::Borrow<Q>,
        Q: Eq + Hash + ?Sized,
    {
        let mut entries = self.entries.lock().expect("cache lock");
        entries.clock += 1;
        let clock = entries.clock;
        let (value, used) = entries.values.get_mut(key)?;
        *used = clock;
        Some(Arc::clone(value))
    }

    /// Keeps `value`, forgetting the least recently used value when full.
    fn insert(&self, key: K, value: V) -> Arc<V> {
        let mut entries = self.entries.lock().expect("cache lock");
        entries.clock += 1;
        let clock = entries.clock;
        if entries.values.len() >= CACHED
            && !entries.values.contains_key(&key)
            && let Some(oldest) = entries
                .values
                .iter()
                .min_by_key(|(_, (_, used))| *used)
                .map(|(key, _)| key.clone())
        {
            entries.values.remove(&oldest);
        }
        let value = Arc::new(value);
        entries.values.insert(key, (Arc::clone(&value), clock));
        value
    }
}
