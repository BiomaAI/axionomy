//! The engine in Studio's browser Worker (D-045, D-047): an
//! `axionomy-engine` over the Worker's store, on the origin private file
//! system or in memory, offering the operations `axionomy-studio-api`
//! defines, as the Studio server does (P12). Packages load from their bytes,
//! however the Worker got them; the store remembers which URL each came from
//! and what that URL said of its response, so the Worker asks for it again
//! only to learn whether it changed.

use axionomy_engine::{Engine, EngineError, ExperimentPath, Loaded, PackagePath};
use axionomy_store::{Durability, RefPath, Store, StoreError};
use axionomy_studio_api::{self as api, LoadedView, PointId, Refusal};
use js_sys::Function;
use redb::{TableDefinition, TableError};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::ops::ControlFlow;
use wasm_bindgen::prelude::*;
use web_sys::FileSystemSyncAccessHandle;

/// Where each package came from: a URL, kept as the text HTTP defines, and
/// the record of what it served.
const SOURCES: TableDefinition<&str, &[u8]> = TableDefinition::new("studio-sources");

/// Search progress reported to the Worker once every this many reports, so
/// a search spends its time searching rather than posting messages.
const PROGRESS_EVERY: u64 = 16;

/// What an HTTP response said to tell a later response for the same URL
/// apart from it: its `ETag` and `Last-Modified`, as the server wrote them.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Validators {
    pub etag: Option<String>,
    pub last_modified: Option<String>,
}

/// What the store loaded from a URL: what the response said of itself, and
/// the packages it held.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Source {
    pub validators: Validators,
    pub packages: Vec<PackagePath>,
}

/// The engine of Studio's browser Worker, over the Worker's store.
#[wasm_bindgen(js_name = Engine)]
pub struct WorkerEngine {
    engine: Engine,
}

impl WorkerEngine {
    /// An engine over `store`.
    pub fn over(store: Store) -> Result<Self, EngineError> {
        Ok(Self {
            engine: Engine::new(store)?,
        })
    }

    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    /// What the store last loaded from `url`, while it still holds every
    /// package it loaded from there; `None` when the URL has to be fetched
    /// whole.
    pub fn source_of(&self, url: &str) -> Result<Option<Source>, EngineError> {
        let reader = self.engine.store().read()?;
        let table = match reader.redb().open_table(SOURCES) {
            Ok(table) => table,
            Err(TableError::TableDoesNotExist(_)) => return Ok(None),
            Err(error) => return Err(StoreError::from(error).into()),
        };
        let Some(bytes) = table.get(url).map_err(StoreError::from)? else {
            return Ok(None);
        };
        let source: Source = postcard::from_bytes(bytes.value())
            .map_err(|_| StoreError::Malformed("a package's source"))?;
        let held: Vec<PackagePath> = self
            .engine
            .manifests()?
            .into_iter()
            .map(|manifest| manifest.name)
            .collect();
        Ok(source
            .packages
            .iter()
            .all(|package| held.contains(package))
            .then_some(source))
    }

    /// Loads the package file `bytes`, a store file or a zip of one, as the
    /// engine loads any package: its manifests checked, then merged. With
    /// `from`, remembers it came from that URL with those validators, once it
    /// is merged.
    pub fn load_from(
        &self,
        bytes: &[u8],
        from: Option<(&str, Validators)>,
    ) -> Result<Loaded, EngineError> {
        self.merge(&axionomy_engine::package(bytes)?, from)
    }

    /// Merges the package store `package` as [`load_from`](Self::load_from)
    /// does.
    fn merge(
        &self,
        package: &Store,
        from: Option<(&str, Validators)>,
    ) -> Result<Loaded, EngineError> {
        let loaded = self.engine.load(package)?;
        if let Some((url, validators)) = from {
            let source = Source {
                validators,
                packages: loaded
                    .manifests
                    .iter()
                    .map(|manifest| manifest.name.clone())
                    .collect(),
            };
            let bytes = postcard::to_allocvec(&source).expect("a source encodes");
            let url = url.to_owned();
            self.engine
                .store()
                .transact(Durability::Immediate, move |txn| {
                    txn.redb()
                        .open_table(SOURCES)?
                        .insert(url.as_str(), bytes.as_slice())?;
                    Ok(())
                })
                .wait()?;
        }
        Ok(loaded)
    }
}

/// The Worker's operations take and give JSON, as the Studio server's
/// endpoints do, with the same types. A refused operation throws the JSON
/// of `{status, body}`: the status the server answers with, and its body.
#[wasm_bindgen(js_class = Engine)]
impl WorkerEngine {
    /// Opens the Worker's store on `file`, a synchronous access handle to a
    /// file of the origin private file system, where it stays across
    /// reloads; in memory without one.
    pub fn open(file: Option<FileSystemSyncAccessHandle>) -> Result<WorkerEngine, JsError> {
        let store = match file {
            #[cfg(target_arch = "wasm32")]
            Some(file) => Store::on(crate::opfs::Opfs::new(file)),
            #[cfg(not(target_arch = "wasm32"))]
            Some(_) => unreachable!("only a browser has an origin private file system"),
            None => Store::memory(),
        }
        .map_err(failed)?;
        Self::over(store).map_err(failed)
    }

    /// Whether the store stays across reloads.
    #[wasm_bindgen(getter)]
    pub fn persistent(&self) -> bool {
        !self.engine.store().in_memory()
    }

    /// How many times the engine has moved a ref, making one included:
    /// every move goes through it, so a follower waiting in the Worker looks
    /// again once this changes.
    #[wasm_bindgen(getter)]
    pub fn moves(&self) -> u64 {
        self.engine.moves()
    }

    /// What the store last loaded from `url`, as JSON, while it still holds
    /// every package it loaded from there: the response's `validators`,
    /// its `etag` and `last_modified`, to ask the URL whether it serves
    /// something else, and the `packages` it held. `undefined` when the URL
    /// has to be fetched whole.
    pub fn source(&self, url: &str) -> Result<Option<String>, JsValue> {
        self.source_of(url)
            .map_err(Refusal::from)
            .map_err(thrown)?
            .map(|source| answer(Ok(source)))
            .transpose()
    }

    /// Loads the package file `bytes`, a store file or a zip of one: checks
    /// its manifests, merges it into the store and, given the `url` it came
    /// from, remembers the response's `etag` and `last_modified`. Gives a
    /// `LoadedView`.
    pub fn load(
        &self,
        bytes: &[u8],
        url: Option<String>,
        etag: Option<String>,
        last_modified: Option<String>,
    ) -> Result<String, JsValue> {
        let validators = Validators {
            etag,
            last_modified,
        };
        let package = axionomy_engine::package(bytes)
            .map_err(|error| Refusal::BadRequest(format!("not a package file: {error}")))
            .map_err(thrown)?;
        answer(
            self.merge(&package, url.as_deref().map(|url| (url, validators)))
                .map(|loaded| LoadedView::from(&loaded))
                .map_err(Refusal::from),
        )
    }

    /// The packages the store holds: a `PackageList`.
    pub fn packages(&self) -> Result<String, JsValue> {
        answer(api::packages(&self.engine))
    }

    /// The paths of the experiments the store keeps.
    pub fn experiments(&self) -> Result<String, JsValue> {
        answer(self.engine.experiments().map_err(Refusal::from))
    }

    /// The history of `point`, a `PointId`, drawn with its view: a
    /// `ViewDocument`.
    pub fn draw(&self, point: &str) -> Result<String, JsValue> {
        answer(point_id(point).and_then(|point| api::draw(&self.engine, point)))
    }

    /// The point at an earlier step of `point`'s history; `query` is an
    /// `AncestorQuery`.
    pub fn ancestor(&self, point: &str, query: &str) -> Result<String, JsValue> {
        answer(point_id(point).and_then(|point| api::ancestor(&self.engine, point, parsed(query)?)))
    }

    /// Every account at `point` and what it holds there: a `Balances`.
    pub fn balances(&self, point: &str) -> Result<String, JsValue> {
        answer(point_id(point).and_then(|point| api::balances(&self.engine, point)))
    }

    /// What a candidate set offers at `point`, each exchange assessed;
    /// `query` is a `CandidatesQuery`.
    pub fn candidates(&self, point: &str, query: &str) -> Result<String, JsValue> {
        answer(
            point_id(point).and_then(|point| api::candidates(&self.engine, point, parsed(query)?)),
        )
    }

    /// An exchange assessed at `point`; `request` is an `AssessRequest`.
    pub fn assess(&self, point: &str, request: &str) -> Result<String, JsValue> {
        answer(point_id(point).and_then(|point| api::assess(&self.engine, point, parsed(request)?)))
    }

    /// Exchanges applied at `point`, all of them or none; `request` is an
    /// `ApplyRequest`.
    pub fn apply(&self, point: &str, request: &str) -> Result<String, JsValue> {
        answer(point_id(point).and_then(|point| api::apply(&self.engine, point, parsed(request)?)))
    }

    /// Runs a strategy from `point` toward a goal, `request` a
    /// `StrategyRequest` naming both, calling `progress` with its
    /// `GraphSearchProgress` as JSON now and then; gives a `RanView`.
    #[wasm_bindgen(js_name = runStrategy)]
    pub fn run_strategy(
        &self,
        point: &str,
        request: &str,
        progress: &Function,
    ) -> Result<String, JsValue> {
        let mut watch = reporter(progress);
        answer(
            point_id(point).and_then(|point| {
                api::run_strategy(&self.engine, point, parsed(request)?, &mut watch)
            }),
        )
    }

    /// Runs the experiment at `path`, calling `progress` with a search's
    /// progress as JSON now and then; gives a `ResultsView`.
    #[wasm_bindgen(js_name = runExperiment)]
    pub fn run_experiment(&self, path: &str, progress: &Function) -> Result<String, JsValue> {
        let mut watch = reporter(progress);
        answer(
            path_of::<ExperimentPath>(path)
                .and_then(|path| api::run_experiment(&self.engine, path, &mut watch)),
        )
    }

    /// The results the experiment at `path` last recorded, a `ResultsView`;
    /// refused with 404 before it has run.
    pub fn results(&self, path: &str) -> Result<String, JsValue> {
        answer(path_of(path).and_then(|path| api::results(&self.engine, path)))
    }

    /// Points compared on objectives; `request` is a `CompareRequest`.
    pub fn compare(&self, request: &str) -> Result<String, JsValue> {
        answer(parsed(request).and_then(|request| api::compare(&self.engine, request)))
    }

    /// Names a point with a new ref; `request` is a `RefCreateRequest`.
    #[wasm_bindgen(js_name = createRef)]
    pub fn create_ref(&self, request: &str) -> Result<String, JsValue> {
        answer(parsed(request).and_then(|request| api::create_ref(&self.engine, request)))
    }

    /// The point the ref `name` names, a `RefView`.
    #[wasm_bindgen(js_name = readRef)]
    pub fn read_ref(&self, name: &str) -> Result<String, JsValue> {
        answer(path_of::<RefPath>(name).and_then(|name| api::read_ref(&self.engine, name)))
    }

    /// Moves the ref `name` forward; `request` is a `RefAdvanceRequest`.
    /// Refused with 409 and where the ref is when it is not at `expected`.
    #[wasm_bindgen(js_name = advanceRef)]
    pub fn advance_ref(&self, name: &str, request: &str) -> Result<String, JsValue> {
        answer(
            path_of::<RefPath>(name)
                .and_then(|name| api::advance_ref(&self.engine, name, parsed(request)?)),
        )
    }

    /// The steps the ref `name` was moved over after a point, at once;
    /// `query` is a `RefFramesQuery`. The engine's own wait blocks a thread,
    /// so the Worker makes the wait: it looks again once
    /// [`moves`](Self::moves) changes, or when the wait is over.
    pub fn frames(&self, name: &str, query: &str) -> Result<String, JsValue> {
        answer(
            path_of::<RefPath>(name)
                .and_then(|name| api::frames(&self.engine, name, parsed(query)?)),
        )
    }
}

/// A watcher calling `progress` with every [`PROGRESS_EVERY`]th report of a
/// run's progress, as JSON, the first included.
fn reporter(
    progress: &Function,
) -> impl FnMut(&axionomy_search::GraphSearchProgress) -> ControlFlow<()> + '_ {
    let mut reports = 0_u64;
    move |watched| {
        reports += 1;
        if reports % PROGRESS_EVERY == 1
            && let Ok(watched) = serde_json::to_string(watched)
        {
            let _ = progress.call1(&JsValue::NULL, &JsValue::from_str(&watched));
        }
        ControlFlow::Continue(())
    }
}

fn point_id(text: &str) -> Result<PointId, Refusal> {
    text.parse()
        .map_err(|error: axionomy_studio_api::PointIdError| Refusal::BadRequest(error.to_string()))
}

fn path_of<T: std::str::FromStr<Err: std::fmt::Display>>(text: &str) -> Result<T, Refusal> {
    text.parse()
        .map_err(|error: T::Err| Refusal::BadRequest(error.to_string()))
}

/// A request's JSON, read as the server's JSON extractor reads it.
fn parsed<T: DeserializeOwned>(json: &str) -> Result<T, Refusal> {
    serde_json::from_str(json).map_err(|error| Refusal::BadRequest(error.to_string()))
}

/// An answer's JSON, or the refusal thrown.
fn answer<T: Serialize>(result: Result<T, Refusal>) -> Result<String, JsValue> {
    let value = result.map_err(thrown)?;
    serde_json::to_string(&value).map_err(|error| thrown(Refusal::Failed(error.to_string())))
}

/// A refusal as the Worker throws it: the JSON of `{status, body}`.
fn thrown(refusal: Refusal) -> JsValue {
    JsValue::from_str(
        &serde_json::to_string(&refusal.refused()).expect("a refusal always serializes"),
    )
}

fn failed(error: impl std::fmt::Display) -> JsError {
    JsError::new(&error.to_string())
}
