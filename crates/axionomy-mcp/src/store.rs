//! What the server keeps beside the engine's runs: handles for economies,
//! assessments, studies and refs.
//!
//! Every economy is a point the engine recorded. Authoring records a run that
//! starts from the authored state; applying exchanges records a run that forks
//! from the source economy's point, with one step per exchange. A study records
//! each of its documents the same way: its initial state as a run of its own,
//! and its trace as a run forked from it. A handle is written after the runs it
//! names, so a handle never names something missing. Handles are keyed with a
//! secret the store keeps, so they stay valid across restarts of a server that
//! keeps its store on disk, and stay infeasible to guess.
//!
//! Every point a handle names is indexed under the first handle that named
//! it, by the engine's one name for the point: a ref, which names a point,
//! reads back as that handle, and a step the ref was moved over as the handle
//! of the economy the step made. A step no handle names yet gets one minted
//! from its point: the economy its exchange made from the step before. A
//! point a package names, or an experiment reached, is named the same way: by
//! the first handle that named it, or by one minted from it, the economy its
//! run's exchanges made from where the run starts, or an authored one when
//! its run starts from an economy of its own.

use crate::{
    handles::{AssessmentId, EconomyId, RefId, StudyId},
    messages::DocumentSummary,
    studies::Trajectory,
};
use axionomy::paths;
use axionomy_engine::{
    Engine, EngineError, ExperimentPath, Following, Frame, GoalPath, Reached, Results,
};
use axionomy_store::{Durability, Point, Reader, RefPath, StoreError};
use axionomy_study::{Study, StudyKey};
use axionomy_view::DocumentPath;

/// An economy's state: MCP's economies are identified by paths.
pub(crate) type State = axionomy_engine::State;
use redb::{ReadableTable, TableDefinition};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::{
    collections::HashMap,
    hash::Hash,
    ops::ControlFlow,
    sync::{Arc, Mutex},
    time::Duration,
};

const ECONOMIES: TableDefinition<&str, &[u8]> = TableDefinition::new("mcp-economies");
const ASSESSMENTS: TableDefinition<&str, &[u8]> = TableDefinition::new("mcp-assessments");
const STUDIES: TableDefinition<&str, &[u8]> = TableDefinition::new("mcp-studies");
const SECRETS: TableDefinition<&str, &[u8; 32]> = TableDefinition::new("mcp-secrets");
/// The handle that first named each point, by the point's encoding.
const POINTS: TableDefinition<&[u8], &str> = TableDefinition::new("mcp-points");
const HANDLE_SECRET: &str = "handles";

/// Assessments and studies kept in memory.
const CACHED: usize = 32;

/// How a stored economy was made from its parent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
    #[serde(serialize_with = "hex")]
    root: axionomy_tree::Hash,
}

/// A root written as its hex digest, as every handle's operation writes it.
fn hex<S: serde::Serializer>(root: &axionomy_tree::Hash, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.collect_str(&root.to_hex())
}

/// An economy as the server serves it: what its handle names, and the point
/// the engine reached.
pub(crate) struct Loaded {
    pub record: EconomyRecord,
    pub reached: Arc<Reached>,
}

impl Loaded {
    pub fn state(&self) -> &State {
        &self.reached.state
    }

    /// The receipts of the exchanges that made it from its parent: the last
    /// steps its point's run took to reach it.
    pub fn receipts(&self) -> &[paths::Receipt] {
        let receipts = &self.reached.receipts;
        match self.record.origin {
            Origin::Exchanged { exchanges } => {
                &receipts[receipts.len().saturating_sub(exchanges)..]
            }
            Origin::Authored { .. } => receipts,
        }
    }
}

/// An assessment as the server serves it, computed again from its record.
pub(crate) struct Assessed {
    pub economy_id: EconomyId,
    pub after_exchanges: usize,
    pub exchange: paths::Exchange,
    pub assessment: paths::Assessment,
}

/// The server's handles for economies, assessments and studies, over the
/// engine that records them, with the most recently used assessments and
/// studies in memory.
#[derive(Clone)]
pub(crate) struct Archive {
    engine: Engine,
    secret: Arc<[u8; 32]>,
    assessments: Cache<AssessmentId, Assessed>,
    studies: Cache<StudyId, Study>,
}

/// Whether a stored economy was stored by this call or an earlier one.
pub(crate) struct Stored {
    pub economy_id: EconomyId,
    pub deduplicated: bool,
}

impl Archive {
    /// Opens the server's tables in the engine's store, creating its handle
    /// secret the first time.
    pub fn open(engine: Engine) -> Result<Self, StoreError> {
        let mut fresh = [0; 32];
        getrandom::fill(&mut fresh).expect("operating system randomness is unavailable");
        let secret = engine
            .store()
            .transact(Durability::Immediate, move |txn| {
                let txn = txn.redb();
                txn.open_table(ECONOMIES)?;
                txn.open_table(ASSESSMENTS)?;
                txn.open_table(STUDIES)?;
                txn.open_table(POINTS)?;
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
            engine,
            secret: Arc::new(secret),
            assessments: Cache::default(),
            studies: Cache::default(),
        })
    }

    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    /// Names `point`, which the engine recorded as a run forked from
    /// `source_id`'s point, under the handle replaying its exchanges on
    /// `source_id` would give: the same exchanges name the same economy.
    pub async fn reached(
        &self,
        source_id: &EconomyId,
        point: Point,
    ) -> Result<Stored, EngineError> {
        let engine = self.engine.clone();
        let reached = blocking(move || engine.reach(point)).await?;
        let exchanges: Vec<_> = reached
            .receipts
            .iter()
            .map(|receipt| receipt.exchange().clone())
            .collect();
        let economy_id = self.handle(Some(source_id), &operation("exchanges", &exchanges));
        if self.record(&economy_id)?.is_some() {
            return Ok(Stored {
                economy_id,
                deduplicated: true,
            });
        }
        let record = EconomyRecord {
            point,
            parent: Some(source_id.clone()),
            origin: Origin::Exchanged {
                exchanges: exchanges.len(),
            },
        };
        self.keep(economy_id, record).await
    }

    /// Whether the store is in memory, so its handles end with the server.
    pub fn in_memory(&self) -> bool {
        self.engine.store().in_memory()
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

    /// Records an authored economy as a run of its own, under the handle
    /// `operation` mints from `parent`.
    pub async fn author(
        &self,
        parent: Option<EconomyId>,
        operation: &[u8],
        entries: usize,
        economy: paths::Economy,
    ) -> Result<Stored, EngineError> {
        let economy_id = self.handle(parent.as_ref(), operation);
        if self.record(&economy_id)?.is_some() {
            return Ok(Stored {
                economy_id,
                deduplicated: true,
            });
        }
        let engine = self.engine.clone();
        let point = blocking(move || engine.author(economy)).await?;
        let record = EconomyRecord {
            point,
            parent,
            origin: Origin::Authored { entries },
        };
        self.keep(economy_id, record).await
    }

    /// Applies `exchanges` at `source`'s point, all of them or none, under
    /// the handle their operation mints from `source_id`. Applying one
    /// exchange and replaying it alone are the same operation, so they share
    /// a handle. A refusal stores nothing.
    pub async fn exchanged(
        &self,
        source_id: &EconomyId,
        source: &Loaded,
        exchanges: Vec<paths::Exchange>,
    ) -> Result<Stored, EngineError> {
        let economy_id = self.handle(Some(source_id), &operation("exchanges", &exchanges));
        if self.record(&economy_id)?.is_some() {
            return Ok(Stored {
                economy_id,
                deduplicated: true,
            });
        }
        let count = exchanges.len();
        let at = source.record.point;
        let engine = self.engine.clone();
        let point = blocking(move || engine.apply(at, &exchanges)).await?;
        let record = EconomyRecord {
            point,
            parent: Some(source_id.clone()),
            origin: Origin::Exchanged { exchanges: count },
        };
        self.keep(economy_id, record).await
    }

    /// Names the state after the first `step` exchanges of those that made
    /// `economy_id`, so it reads like any economy.
    pub async fn step(
        &self,
        economy_id: &EconomyId,
        loaded: &Loaded,
        step: usize,
    ) -> Result<Stored, EngineError> {
        let handle = EconomyId::from_digest(&self.keyed(&[
            economy_id.as_str().as_bytes(),
            b"step",
            &(step as u64).to_le_bytes(),
        ]));
        if self.record(&handle)?.is_some() {
            return Ok(Stored {
                economy_id: handle,
                deduplicated: true,
            });
        }
        let at = loaded.record.point;
        let first = at.step - loaded.receipts().len() as u64;
        let engine = self.engine.clone();
        let point = blocking(move || engine.ancestor(at, first + step as u64)).await?;
        let record = EconomyRecord {
            point,
            parent: loaded.record.parent.clone(),
            origin: Origin::Exchanged { exchanges: step },
        };
        self.keep(handle, record).await
    }

    /// Writes `record` under `economy_id`, unless a concurrent call wrote it
    /// first.
    async fn keep(
        &self,
        economy_id: EconomyId,
        record: EconomyRecord,
    ) -> Result<Stored, EngineError> {
        let handle = economy_id.clone();
        let stored = self
            .engine
            .store()
            .transact(Durability::Immediate, move |txn| {
                write(txn.redb(), &handle, &record)
            })
            .await?;
        Ok(Stored {
            economy_id,
            deduplicated: !stored,
        })
    }

    /// Writes every record of `records` that a concurrent call did not write
    /// first, in one transaction: a record may name an earlier one as its
    /// parent.
    async fn keep_all(&self, records: Vec<(EconomyId, EconomyRecord)>) -> Result<(), StoreError> {
        if records.is_empty() {
            return Ok(());
        }
        self.engine
            .store()
            .transact(Durability::Immediate, move |txn| {
                for (economy_id, record) in &records {
                    write(txn.redb(), economy_id, record)?;
                }
                Ok(())
            })
            .await
    }

    /// The handle that first named `point`: every point a ref of this server
    /// is at has one, indexed before the ref moved there.
    pub fn handle_of(&self, point: Point) -> Result<EconomyId, StoreError> {
        named(&self.engine.store().read()?, point)?
            .ok_or(StoreError::Malformed("a ref at a point no handle names"))
    }

    /// Makes a new ref naming `economy`'s point, under a handle minted from
    /// fresh randomness, so every call makes a ref of its own; and the
    /// handle the ref names the point by.
    pub async fn refer(
        &self,
        economy_id: &EconomyId,
        economy: &Loaded,
    ) -> Result<(RefId, EconomyId), EngineError> {
        let mut nonce = [0; 32];
        getrandom::fill(&mut nonce).expect("operating system randomness is unavailable");
        let ref_id = RefId::from_digest(&self.keyed(&[b"ref", &nonce]));
        let at = economy.record.point;
        let named = self.index(economy_id, at).await?;
        let (engine, name) = (self.engine.clone(), ref_path(&ref_id));
        blocking(move || engine.refer(&name, at)).await?;
        Ok((ref_id, named))
    }

    /// Moves the ref from `expected` forward to `to`, the point of
    /// `economy_id`, and gives the handle the ref names it by.
    pub async fn advance(
        &self,
        ref_id: &RefId,
        expected: Point,
        economy_id: &EconomyId,
        to: Point,
    ) -> Result<EconomyId, EngineError> {
        let named = self.index(economy_id, to).await?;
        let (engine, name) = (self.engine.clone(), ref_path(ref_id));
        blocking(move || engine.advance(&name, expected, to)).await?;
        Ok(named)
    }

    /// The economy the ref is at, `None` for a ref this store does not hold.
    pub fn referred(&self, ref_id: &RefId) -> Result<Option<EconomyId>, EngineError> {
        match self.engine.referred(&ref_path(ref_id)) {
            Ok(point) => Ok(Some(self.handle_of(point)?)),
            Err(EngineError::UnknownRef(_)) => Ok(None),
            Err(error) => Err(error),
        }
    }

    /// The steps the ref was moved over after `after`, the point of
    /// `after_id`, at most `limit` of them, each with the handle of the
    /// economy it made and whether that economy holds `goal`; and where the
    /// ref is. When the ref is still at `after`, waits up to `wait` for it to
    /// move first.
    pub async fn frames(
        &self,
        ref_id: &RefId,
        after_id: &EconomyId,
        after: Point,
        goal: Option<paths::Goal>,
        limit: usize,
        wait: Duration,
    ) -> Result<(Vec<(EconomyId, Frame)>, EconomyId), EngineError> {
        let (engine, name) = (self.engine.clone(), ref_path(ref_id));
        let (frames, current) = blocking(move || {
            engine.wait(&name, after, wait)?;
            let goal = goal.map(|goal| kept_goal(&engine, &goal)).transpose()?;
            let following = Following {
                objectives: Vec::new(),
                goal,
            };
            let frames = engine.frames(&name, after, &following, limit)?;
            Ok((frames, engine.referred(&name)?))
        })
        .await?;
        let reader = self.engine.store().read()?;
        let mut previous = named(&reader, after)?.unwrap_or_else(|| after_id.clone());
        let mut minted = Vec::new();
        let mut named_frames = Vec::with_capacity(frames.len());
        for frame in frames {
            let point = frame.point;
            let economy_id = match named(&reader, point)? {
                Some(economy_id) => economy_id,
                None => {
                    // A frame's point is a step of its run, so its receipt
                    // is the run's last up to it.
                    if reader
                        .run(point.run)?
                        .is_none_or(|run| point.step == run.first)
                    {
                        return Err(StoreError::Malformed("a frame that is no step").into());
                    }
                    let economy_id = self.minted(point);
                    let record = EconomyRecord {
                        point,
                        parent: Some(previous),
                        origin: Origin::Exchanged { exchanges: 1 },
                    };
                    minted.push((economy_id.clone(), record));
                    economy_id
                }
            };
            previous = economy_id.clone();
            named_frames.push((economy_id, frame));
        }
        drop(reader);
        self.keep_all(minted).await?;
        Ok((named_frames, self.handle_of(current)?))
    }

    /// The handle minted for `point`, which no handle named first.
    fn minted(&self, point: Point) -> EconomyId {
        EconomyId::from_digest(&self.keyed(&[b"point", &encode(&point)]))
    }

    /// The handles of `points`, which this server did not make, such as
    /// those a package's refs name and those an experiment reached: each
    /// point's first handle, or one minted from it and recorded as the
    /// economy its run's exchanges made from where the run starts, itself
    /// named so; a run that starts from an economy of its own starts from
    /// an authored one.
    pub async fn names(&self, points: Vec<Point>) -> Result<Vec<EconomyId>, EngineError> {
        let archive = self.clone();
        let (names, minted) = blocking(move || {
            let reader = archive.engine.store().read()?;
            let mut naming = Naming {
                archive: &archive,
                reader: &reader,
                minted: Vec::new(),
                minted_at: HashMap::new(),
            };
            let names = points
                .into_iter()
                .map(|point| naming.name(point))
                .collect::<Result<Vec<_>, _>>()?;
            Ok((names, naming.minted))
        })
        .await?;
        self.keep_all(minted).await?;
        Ok(names)
    }

    /// Runs the experiment at `path` where blocking is allowed, recording
    /// its runs and results.
    pub async fn run_experiment(&self, path: ExperimentPath) -> Result<Results, EngineError> {
        let engine = self.engine.clone();
        blocking(move || engine.run_experiment(&path, &mut |_| ControlFlow::Continue(()))).await
    }

    /// Indexes `point` by `economy_id` unless a handle names it already, and
    /// gives the handle that names it.
    async fn index(&self, economy_id: &EconomyId, point: Point) -> Result<EconomyId, StoreError> {
        if let Some(named) = named(&self.engine.store().read()?, point)? {
            return Ok(named);
        }
        let handle = economy_id.clone();
        self.engine
            .store()
            .transact(Durability::Immediate, move |txn| {
                index(txn.redb(), point, &handle)
            })
            .await
    }

    fn record(&self, economy_id: &EconomyId) -> Result<Option<EconomyRecord>, StoreError> {
        let reader = self.engine.store().read()?;
        get(&reader, ECONOMIES, economy_id.as_str())
    }

    /// The economy a handle names, with the receipts that made it.
    pub async fn economy(
        &self,
        economy_id: &EconomyId,
    ) -> Result<Option<Arc<Loaded>>, EngineError> {
        let Some(record) = self.record(economy_id)? else {
            return Ok(None);
        };
        let engine = self.engine.clone();
        let at = record.point;
        let reached = blocking(move || engine.reach(at)).await?;
        Ok(Some(Arc::new(Loaded { record, reached })))
    }

    /// Whether `exchange` would apply to `economy`, and why not.
    pub async fn assess(
        &self,
        economy: &Loaded,
        exchange: &paths::Exchange,
    ) -> Result<paths::Assessment, EngineError> {
        let engine = self.engine.clone();
        let (at, exchange) = (economy.record.point, exchange.clone());
        blocking(move || engine.assess(at, &[], &exchange)).await
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
        let assessment_id =
            AssessmentId::from_digest(&self.keyed(&[b"assessment", &encode(&record)]));
        let key = assessment_id.to_string();
        self.engine
            .store()
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
    ) -> Result<Option<Arc<Assessed>>, EngineError> {
        if let Some(assessed) = self.assessments.get(assessment_id) {
            return Ok(Some(assessed));
        }
        let reader = self.engine.store().read()?;
        let Some(record) = get::<AssessmentRecord>(&reader, ASSESSMENTS, assessment_id.as_str())?
        else {
            return Ok(None);
        };
        let Some(economy) = self.record(&record.economy_id)? else {
            return Ok(None);
        };
        let engine = self.engine.clone();
        let (at, before, exchange) = (
            economy.point,
            record.before.clone(),
            record.exchange.clone(),
        );
        let assessment = blocking(move || engine.assess(at, &before, &exchange)).await?;
        let assessed = Assessed {
            assessment,
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
    ) -> Result<(StudyId, StudyRecord), EngineError> {
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
        self.engine
            .store()
            .transact(Durability::Immediate, move |txn| {
                put(txn.redb(), STUDIES, &key, &kept)
            })
            .await?;
        self.studies.insert(study_id.clone(), study);
        Ok((study_id, record))
    }

    fn study_id(&self, key: &StudyKey) -> StudyId {
        StudyId::from_digest(&self.keyed(&[b"study", &encode(key)]))
    }

    /// Records a document's initial state as a run of its own and its
    /// exchanges as a run forked from it, and returns both handles: the
    /// initial one twice when it has no exchanges.
    async fn record_trajectory(
        &self,
        key: &StudyKey,
        trajectory: Trajectory,
    ) -> Result<(EconomyId, EconomyId), EngineError> {
        let studied = trajectory_operations(key, &trajectory).0;
        let Trajectory {
            initial, exchanges, ..
        } = trajectory;
        let entries = crate::authoring::entries(&initial.economy);
        let initial_id = self
            .author(None, &studied, entries, initial.economy)
            .await?
            .economy_id;
        if exchanges.is_empty() {
            return Ok((initial_id.clone(), initial_id));
        }
        let source = self
            .economy(&initial_id)
            .await?
            .ok_or(StoreError::Malformed("study document's initial economy"))?;
        let final_id = self
            .exchanged(&initial_id, &source, exchanges)
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
        let reader = self.engine.store().read()?;
        let table = reader
            .redb()
            .open_table(STUDIES)
            .map_err(StoreError::from)?;
        let Some(bytes) = table.get(study_id.as_str()).map_err(StoreError::from)? else {
            return Ok(None);
        };
        postcard::from_bytes(bytes.value())
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
    pub fn store(&self) -> axionomy_store::Store {
        self.engine.store().clone()
    }

    /// The bytes a study is kept as.
    pub fn study_bytes(&self, study_id: &StudyId) -> Option<Vec<u8>> {
        let reader = self.engine.store().read().unwrap();
        let table = reader.redb().open_table(STUDIES).unwrap();
        let bytes = table.get(study_id.as_str()).unwrap()?;
        Some(bytes.value().to_vec())
    }

    /// Replaces the bytes a study is kept as.
    pub async fn replace_study_bytes(&self, study_id: &StudyId, bytes: Vec<u8>) {
        let key = study_id.to_string();
        self.engine
            .store()
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

/// Points named so far by one call to [`Archive::names`], and the records
/// of the handles it minted.
struct Naming<'a> {
    archive: &'a Archive,
    reader: &'a Reader,
    minted: Vec<(EconomyId, EconomyRecord)>,
    minted_at: HashMap<Point, EconomyId>,
}

impl Naming<'_> {
    /// The handle of `point`: its first handle, or one minted for it.
    fn name(&mut self, point: Point) -> Result<EconomyId, EngineError> {
        if let Some(named) = named(self.reader, point)? {
            return Ok(named);
        }
        if let Some(minted) = self.minted_at.get(&point) {
            return Ok(minted.clone());
        }
        let run = self
            .reader
            .run(point.run)?
            .ok_or(StoreError::Malformed("a point of a run the store lacks"))?;
        // A point that starts its run starts from an economy of its own; any
        // other is the economy its run's exchanges made from the point the
        // run starts from, which a fork's first state is named by.
        let (parent, origin) = if point.step == run.first {
            let reached = self.archive.engine.reach(point)?;
            let entries = crate::authoring::entries(&reached.state.economy);
            (None, Origin::Authored { entries })
        } else {
            let first = run.from.unwrap_or(Point {
                run: point.run,
                step: run.first,
            });
            let exchanges = usize::try_from(point.step - run.first)
                .map_err(|_| StoreError::Malformed("a run longer than memory"))?;
            (Some(self.name(first)?), Origin::Exchanged { exchanges })
        };
        let economy_id = self.archive.minted(point);
        let record = EconomyRecord {
            point,
            parent,
            origin,
        };
        self.minted.push((economy_id.clone(), record));
        self.minted_at.insert(point, economy_id.clone());
        Ok(economy_id)
    }
}

/// The bytes the handle of what an operation makes is minted from, tagged by
/// kind so different operations never share them.
pub(crate) fn operation(kind: &str, value: &impl Serialize) -> Vec<u8> {
    [kind.as_bytes(), &[0], &encode(value)].concat()
}

/// A record, or what a handle is minted from, in postcard.
fn encode(value: &impl Serialize) -> Vec<u8> {
    postcard::to_allocvec(value).expect("records encode")
}

/// The name of a definition MCP keeps, from what it is: `mcp/` and the
/// digest of its encoding, so the same goal or candidates share one.
pub(crate) fn content_name<K>(value: &impl Serialize) -> axionomy::path::Path<K> {
    format!("mcp/{}", blake3::hash(&encode(value)).to_hex())
        .parse()
        .expect("a digest is a path")
}

/// `goal`, kept under its content name unless it is already.
fn kept_goal(engine: &Engine, goal: &paths::Goal) -> Result<GoalPath, EngineError> {
    let path = content_name(goal);
    if let Err(EngineError::Undefined(_)) = engine.goal(&path) {
        engine.keep_goal(&path, goal)?;
    }
    Ok(path)
}

/// The engine's name for a ref this server made: `mcp/` and its handle.
fn ref_path(ref_id: &RefId) -> RefPath {
    format!("mcp/{ref_id}")
        .parse()
        .expect("a handle is a path segment")
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
            root: trajectory.initial.root(),
        },
    );
    let exchanges = &trajectory.exchanges;
    let exchanged = (!exchanges.is_empty()).then(|| operation("exchanges", exchanges));
    (studied, exchanged)
}

/// Runs the engine's synchronous `work` where blocking is allowed.
async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, EngineError> + Send + 'static,
) -> Result<T, EngineError> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|_| EngineError::Store(StoreError::Closed))?
}

/// Writes `record` under `economy_id` and indexes its point, unless a
/// concurrent call wrote it first; whether this call did.
fn write(
    txn: &redb::WriteTransaction,
    economy_id: &EconomyId,
    record: &EconomyRecord,
) -> Result<bool, StoreError> {
    if exists(txn, economy_id)? {
        return Ok(false);
    }
    put(txn, ECONOMIES, economy_id.as_str(), record)?;
    index(txn, record.point, economy_id)?;
    Ok(true)
}

/// Names `point` by `economy_id`, unless a handle names it already, and
/// gives the handle that names it.
fn index(
    txn: &redb::WriteTransaction,
    point: Point,
    economy_id: &EconomyId,
) -> Result<EconomyId, StoreError> {
    let key = encode(&point);
    let mut points = txn.open_table(POINTS).map_err(StoreError::from)?;
    if let Some(named) = points.get(key.as_slice()).map_err(StoreError::from)? {
        return named
            .value()
            .parse()
            .map_err(|_| StoreError::Malformed("server record"));
    }
    points
        .insert(key.as_slice(), economy_id.as_str())
        .map_err(StoreError::from)?;
    Ok(economy_id.clone())
}

fn named(reader: &Reader, point: Point) -> Result<Option<EconomyId>, StoreError> {
    let points = reader.redb().open_table(POINTS).map_err(StoreError::from)?;
    let Some(handle) = points
        .get(encode(&point).as_slice())
        .map_err(StoreError::from)?
    else {
        return Ok(None);
    };
    handle
        .value()
        .parse()
        .map(Some)
        .map_err(|_| StoreError::Malformed("server record"))
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
    let bytes = encode(value);
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
    postcard::from_bytes(bytes.value())
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
