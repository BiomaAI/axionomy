//! What the server keeps, in an [`axionomy_store::Store`]: economies by
//! handle, assessments and studies.
//!
//! Every economy is a point of a run. Authoring records a run that starts
//! from the authored state; applying exchanges records a run that forks from
//! the source economy's point, with one step per exchange. A study records
//! each of its documents the same way: its initial state as a run of its own,
//! and its trace as a run forked from it. A handle, its run's steps and the
//! states they store commit in one transaction, so a handle exists if and
//! only if everything it names does. Handles are keyed with a secret the
//! store keeps, so they stay valid across restarts of a server that keeps its
//! store on disk, and stay infeasible to guess.

use crate::{
    authoring,
    handles::{AssessmentId, EconomyId, StudyId},
    messages::DocumentSummary,
    studies::Trajectory,
};
use axionomy::paths;
use axionomy::paths::Paths;
use axionomy_service::{Study, StudyKey};
use axionomy_store::{Checkpoints, Durability, Point, Reader, Store, StoreError};
use axionomy_view::DocumentPath;

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
const STUDIES: TableDefinition<&str, &[u8]> = TableDefinition::new("mcp-studies");
const SECRETS: TableDefinition<&str, &[u8; 32]> = TableDefinition::new("mcp-secrets");
const HANDLE_SECRET: &str = "handles";

/// States a call's run stores besides its last: replaying to any step of a
/// long replay starts at most this many exchanges back.
const CHECKPOINT_EVERY: NonZeroU64 = NonZeroU64::new(64).expect("nonzero");
/// Loaded economies, assessments and studies kept in memory.
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

/// A study as the server keeps it: what it was computed from, and each
/// document's economies. Its views are computed again from its key.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct StudyRecord {
    pub key: StudyKey,
    pub selected_document: DocumentPath,
    pub documents: Vec<DocumentSummary>,
}

/// The operation a study document's initial economy is stored by: the study,
/// the document and the state it starts from.
#[derive(Serialize)]
struct StudiedDocument<'a> {
    study: &'a StudyKey,
    document: &'a DocumentPath,
    root: String,
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

/// The server's economies, assessments and studies on its store, with the
/// most recently used ones in memory.
#[derive(Clone)]
pub(crate) struct Archive {
    store: Store,
    secret: Arc<[u8; 32]>,
    economies: Cache<EconomyId, Loaded>,
    assessments: Cache<AssessmentId, Assessed>,
    studies: Cache<StudyId, Study>,
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
                txn.open_table(STUDIES)?;
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
            studies: Cache::default(),
        })
    }

    /// Whether the store is in memory, so its handles end with the server.
    pub fn in_memory(&self) -> bool {
        self.store.in_memory()
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

    /// Stores an authored state as a run of its own.
    pub async fn author(
        &self,
        parent: Option<EconomyId>,
        operation: &[u8],
        entries: usize,
        state: State,
    ) -> Result<Stored, StoreError> {
        let economy_id = self.handle(parent.as_ref(), operation);
        if self.record(&economy_id).await?.is_some() {
            return Ok(Stored {
                economy_id,
                deduplicated: true,
            });
        }
        let run = self.store.mint_run();
        let record = EconomyRecord {
            point: Point { run, step: 0 },
            parent,
            origin: Origin::Authored { entries },
        };
        let tree = state.tree.clone();
        let handle = economy_id.clone();
        let kept = record.clone();
        let stored = self
            .store
            .transact(Durability::Immediate, move |txn| {
                if exists(txn.redb(), &handle)? {
                    return Ok(false);
                }
                txn.create_run::<Paths>(run, None, 0, &tree, Checkpoints::Ends)?;
                txn.pin(kept.point, None)?;
                put(txn.redb(), ECONOMIES, handle.as_str(), &kept)?;
                Ok(true)
            })
            .await?;
        if stored {
            let loaded = Loaded {
                record,
                state,
                receipts: Vec::new(),
            };
            self.economies.insert(economy_id.clone(), loaded);
        }
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

    /// Records a study's documents as economies and keeps the study under a
    /// handle minted from its key, so the same study always has the same
    /// handle. `trajectories` are its documents', in order. The record keeps
    /// the key and each document's handles, never the study's views.
    pub async fn keep_study(
        &self,
        study: Study,
        trajectories: Vec<Trajectory>,
    ) -> Result<(StudyId, StudyRecord), StoreError> {
        let mut documents = Vec::with_capacity(trajectories.len());
        for (document, trajectory) in study.documents.iter().zip(trajectories) {
            let (initial_economy_id, final_economy_id) =
                self.record_trajectory(&study.key, trajectory).await?;
            documents.push(DocumentSummary {
                id: document.id.clone(),
                title: document.title.clone(),
                frames: document.frames.len(),
                initial_economy_id,
                final_economy_id,
            });
        }
        let record = StudyRecord {
            key: study.key.clone(),
            selected_document: study.selected_document.clone(),
            documents,
        };
        let study_id = self.study_id(&study.key);
        let key = study_id.to_string();
        let kept = record.clone();
        self.store
            .transact(Durability::Immediate, move |txn| {
                put(txn.redb(), STUDIES, &key, &kept)
            })
            .await?;
        self.studies.insert(study_id.clone(), study);
        Ok((study_id, record))
    }

    fn study_id(&self, key: &StudyKey) -> StudyId {
        let key = serde_json::to_vec(key).expect("study keys serialize");
        StudyId::from_digest(&self.keyed(&[b"study", &key]))
    }

    /// Records a document's initial state as a run of its own and its
    /// exchanges as a run forked from it, and returns both handles: the
    /// initial one twice when it has no exchanges.
    async fn record_trajectory(
        &self,
        key: &StudyKey,
        trajectory: Trajectory,
    ) -> Result<(EconomyId, EconomyId), StoreError> {
        let (studied, exchanged) = trajectory_operations(key, &trajectory);
        let Trajectory {
            initial,
            steps,
            last,
            ..
        } = trajectory;
        let entries = authoring::entries(&initial.economy);
        let initial_id = self
            .author(None, &studied, entries, initial)
            .await?
            .economy_id;
        let Some(exchanged) = exchanged else {
            return Ok((initial_id.clone(), initial_id));
        };
        let source = self
            .economy(&initial_id)
            .await?
            .ok_or(StoreError::Malformed("study document's initial economy"))?;
        let final_id = self
            .exchanged(&initial_id, &source, &exchanged, steps, last)
            .await?
            .economy_id;
        Ok((initial_id, final_id))
    }

    /// Whether `trajectories` are those `record` recorded: the same
    /// documents, from the same initial states, by the same exchanges. It
    /// compares the handles recording them would mint, and stores nothing.
    pub fn recorded(&self, record: &StudyRecord, trajectories: &[Trajectory]) -> bool {
        record.documents.len() == trajectories.len()
            && record
                .documents
                .iter()
                .zip(trajectories)
                .all(|(document, trajectory)| {
                    let (studied, exchanged) = trajectory_operations(&record.key, trajectory);
                    let initial = self.handle(None, &studied);
                    let last = match exchanged {
                        Some(exchanged) => self.handle(Some(&initial), &exchanged),
                        None => initial.clone(),
                    };
                    document.id == trajectory.document
                        && document.initial_economy_id == initial
                        && document.final_economy_id == last
                })
    }

    /// A study's record: its key and its documents' handles.
    pub fn study_record(&self, study_id: &StudyId) -> Result<Option<StudyRecord>, StoreError> {
        let reader = self.store.read()?;
        let table = reader
            .redb()
            .open_table(STUDIES)
            .map_err(StoreError::from)?;
        let Some(bytes) = table.get(study_id.as_str()).map_err(StoreError::from)? else {
            return Ok(None);
        };
        serde_json::from_slice(bytes.value())
            .map(Some)
            .map_err(|_| StoreError::Malformed("study record, kept by another version"))
    }

    /// A study's views, if they are in memory.
    pub fn cached_study(&self, study_id: &StudyId) -> Option<Arc<Study>> {
        self.studies.get(study_id)
    }

    /// Keeps a study's views in memory.
    pub fn cache_study(&self, study_id: StudyId, study: Study) -> Arc<Study> {
        self.studies.insert(study_id, study)
    }
}

#[cfg(test)]
impl Archive {
    /// The store, for a server that starts with nothing in memory.
    pub fn store(&self) -> Store {
        self.store.clone()
    }

    /// The bytes a study is kept as.
    pub fn study_bytes(&self, study_id: &StudyId) -> Option<Vec<u8>> {
        let reader = self.store.read().unwrap();
        let table = reader.redb().open_table(STUDIES).unwrap();
        let bytes = table.get(study_id.as_str()).unwrap()?;
        Some(bytes.value().to_vec())
    }

    /// Replaces the bytes a study is kept as.
    pub async fn replace_study_bytes(&self, study_id: &StudyId, bytes: Vec<u8>) {
        let key = study_id.to_string();
        self.store
            .transact(Durability::Immediate, move |txn| {
                txn.redb()
                    .open_table(STUDIES)?
                    .insert(key.as_str(), bytes.as_slice())?;
                Ok(())
            })
            .await
            .unwrap();
    }
}

/// The bytes the handle of what an operation makes is minted from, tagged by
/// kind so different operations never share them.
pub(crate) fn operation(kind: &str, value: &impl Serialize) -> Vec<u8> {
    let mut bytes = kind.as_bytes().to_vec();
    bytes.push(0);
    serde_json::to_writer(&mut bytes, value).expect("operations serialize");
    bytes
}

/// The operations a trajectory's economies are stored by: its initial state,
/// and its exchanges applied to that state, which is the operation of
/// replaying them with `axionomy_trace_replay`. No exchanges, no second
/// economy.
fn trajectory_operations(key: &StudyKey, trajectory: &Trajectory) -> (Vec<u8>, Option<Vec<u8>>) {
    let studied = operation(
        "study",
        &StudiedDocument {
            study: key,
            document: &trajectory.document,
            root: trajectory.initial.root().to_hex().to_string(),
        },
    );
    let exchanges = trajectory.exchanges();
    let exchanged = (!exchanges.is_empty()).then(|| operation("exchanges", &exchanges));
    (studied, exchanged)
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
