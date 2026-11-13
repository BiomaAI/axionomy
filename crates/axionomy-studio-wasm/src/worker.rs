//! The engine in Studio's browser Worker (D-045, D-047): an
//! `axionomy-engine` over the Worker's store, on the origin private file
//! system or in memory. Packages load from their bytes, however the Worker
//! got them; the store remembers which URL each came from and what that URL
//! said of its response, so the Worker asks for it again only to learn
//! whether it changed.

use axionomy_engine::{Engine, EngineError, ExperimentPath, Loaded, Manifest, PackagePath};
use axionomy_store::{DefinitionPath, Durability, Point, RefPath, Store, StoreError};
use js_sys::Function;
use redb::{TableDefinition, TableError};
use serde::{Deserialize, Serialize};
use std::ops::ControlFlow;
use wasm_bindgen::prelude::*;
use web_sys::FileSystemSyncAccessHandle;

/// Where each package came from: a URL, kept as the text HTTP defines, and
/// the record of what it served.
const SOURCES: TableDefinition<&str, &[u8]> = TableDefinition::new("studio-sources");

/// Search progress reported to the Worker once every this many reports, so
/// a search spends its time searching rather than posting messages.
const PROGRESS_EVERY: u64 = 1024;

/// What an HTTP response said to tell a later response for the same URL
/// apart from it: its `ETag` and `Last-Modified`, as the server wrote them.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Validators {
    pub etag: Option<String>,
    pub last_modified: Option<String>,
}

/// What the store loaded from a URL.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Source {
    validators: Validators,
    packages: Vec<PackagePath>,
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

    /// The validators of what the store last loaded from `url`, while it
    /// still holds every package it loaded from there; `None` when the URL
    /// has to be fetched whole.
    pub fn source_of(&self, url: &str) -> Result<Option<Validators>, EngineError> {
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
            .then_some(source.validators))
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
        let loaded = self.engine.load(&axionomy_engine::package(bytes)?)?;
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

/// What loading a package did, as the Worker reports it.
#[derive(Serialize)]
struct LoadedReport<'a> {
    manifests: &'a [Manifest],
    kept: Kept<'a>,
}

/// Names the store already gave something else, which it kept.
#[derive(Serialize)]
struct Kept<'a> {
    refs: &'a [RefPath],
    definitions: &'a [DefinitionPath],
}

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

    /// The validators of what the store last loaded from `url`, as JSON,
    /// while it still holds every package it loaded from there: what to ask
    /// the URL whether it serves something else. `undefined` when the URL
    /// has to be fetched whole.
    pub fn source(&self, url: &str) -> Result<Option<String>, JsError> {
        self.source_of(url)
            .map_err(failed)?
            .map(|validators| json(&validators))
            .transpose()
    }

    /// Loads the package file `bytes`, a store file or a zip of one: checks
    /// its manifests, merges it into the store and, given the `url` it came
    /// from, remembers the response's `etag` and `last_modified`. Gives the
    /// manifests and the names the store kept, as JSON.
    pub fn load(
        &self,
        bytes: &[u8],
        url: Option<String>,
        etag: Option<String>,
        last_modified: Option<String>,
    ) -> Result<String, JsError> {
        let validators = Validators {
            etag,
            last_modified,
        };
        let loaded = self
            .load_from(bytes, url.as_deref().map(|url| (url, validators)))
            .map_err(failed)?;
        json(&LoadedReport {
            manifests: &loaded.manifests,
            kept: Kept {
                refs: &loaded.kept.refs,
                definitions: &loaded.kept.definitions,
            },
        })
    }

    /// The manifests of the packages the store holds, as JSON.
    pub fn packages(&self) -> Result<String, JsError> {
        json(&self.engine.manifests().map_err(failed)?)
    }

    /// The paths of the experiments the store keeps, as JSON.
    pub fn experiments(&self) -> Result<String, JsError> {
        json(&self.engine.experiments().map_err(failed)?)
    }

    /// Runs the experiment at `path`, calling `progress` with a search's
    /// progress as JSON now and then, and gives its results as JSON.
    #[wasm_bindgen(js_name = runExperiment)]
    pub fn run_experiment(&self, path: &str, progress: &Function) -> Result<String, JsError> {
        let path: ExperimentPath = path.parse().map_err(failed)?;
        let mut reports = 0_u64;
        let results = self
            .engine
            .run_experiment(&path, &mut |watched| {
                reports += 1;
                if reports % PROGRESS_EVERY == 1
                    && let Ok(watched) = serde_json::to_string(watched)
                {
                    let _ = progress.call1(&JsValue::NULL, &JsValue::from_str(&watched));
                }
                ControlFlow::Continue(())
            })
            .map_err(failed)?;
        json(&results)
    }

    /// The results the experiment at `path` last recorded, as JSON, or
    /// `undefined` before it has run.
    pub fn results(&self, path: &str) -> Result<Option<String>, JsError> {
        let path: ExperimentPath = path.parse().map_err(failed)?;
        match self.engine.results(&path) {
            Ok(results) => json(&results).map(Some),
            Err(EngineError::Undefined(_)) => Ok(None),
            Err(error) => Err(failed(error)),
        }
    }

    /// The history of a point drawn with its view, as a JSON document.
    /// `point` is JSON as results write a point: `{"run": "run-…", "step": 3}`.
    pub fn draw(&self, point: &str) -> Result<String, JsError> {
        let point: Point = serde_json::from_str(point).map_err(failed)?;
        json(&self.engine.draw(point).map_err(failed)?)
    }
}

fn json(value: &impl Serialize) -> Result<String, JsError> {
    serde_json::to_string(value).map_err(failed)
}

fn failed(error: impl std::fmt::Display) -> JsError {
    JsError::new(&error.to_string())
}
