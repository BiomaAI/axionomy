//! Packages (P8, D-048): a store file holding economies as runs and points,
//! refs naming them, and the definitions that use them, with a manifest
//! saying what it is. Exporting copies what chosen refs and definitions
//! reach into a new store; loading merges a package into the engine's store.
//! A package file is a redb file, or a zip holding one.

use crate::{
    CandidateSet, Chooser, Engine, EngineError, Experiment, Measure, Objective, Results,
    ScriptCall, Strategy, View, definitions::kind, script::HOST_API,
};
use axionomy::{OntologyPath, path::Path};
use axionomy_store::{
    DefinitionPath, Durability, GroupId, Merged, Point, RefPath, RunId, Selection, Store,
    StoreError, Target,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::{
    collections::BTreeSet,
    fmt,
    io::{Cursor, Read, Write},
    str::FromStr,
};
use thiserror::Error;

/// A package's name, such as `maze`: everything it refers to by ref is
/// named under it.
pub type PackagePath = Path<kind::Package>;

/// A package's version, `major.minor.patch`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Version {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("`{0}` is not a version: three whole numbers, such as `1.0.2`")]
pub struct VersionError(String);

impl FromStr for Version {
    type Err = VersionError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let numbers: Vec<u64> = text
            .split('.')
            .map(str::parse)
            .collect::<Result<_, _>>()
            .map_err(|_| VersionError(text.to_owned()))?;
        match numbers[..] {
            [major, minor, patch] => Ok(Self {
                major,
                minor,
                patch,
            }),
            _ => Err(VersionError(text.to_owned())),
        }
    }
}

impl fmt::Display for Version {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// What a package says of itself, kept in it as `package/<name>`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    pub name: PackagePath,
    pub version: Version,
    /// The store format the package is written in.
    pub format: u64,
    /// The script host API its scripts are written for: a package needs an
    /// engine whose API is at least this.
    pub host_api: u64,
    /// The refs it names, each under its name.
    pub refs: Vec<RefPath>,
    /// The definitions it carries.
    pub definitions: Vec<DefinitionPath>,
}

/// What to put in a package: refs and definitions. Everything they reach
/// comes with them: the definitions they use, the refs an experiment starts
/// from, the views of their runs' ontologies, and the points and runs an
/// experiment's results name.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Contents {
    pub refs: Vec<RefPath>,
    pub definitions: Vec<DefinitionPath>,
}

/// What loading a package did: the manifests it held, and the names the
/// engine's store already gave something else, which it kept.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Loaded {
    pub manifests: Vec<Manifest>,
    pub kept: Merged,
}

/// Why a package cannot be made or loaded.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PackageError {
    #[error("ref `{reference}` is not named under the package's name `{package}`")]
    Outside {
        reference: RefPath,
        package: PackagePath,
    },
    #[error("the store holds no package manifest")]
    NoManifest,
    #[error("package `{package}` needs script host API {needs}; this engine has {HOST_API}")]
    HostApi { package: PackagePath, needs: u64 },
    /// A manifest saying its package is written in a store format other
    /// than this version's.
    #[error(
        "package `{package}` is written in store format {format}; this version reads only format {}",
        axionomy_store::FORMAT_VERSION
    )]
    Format { package: PackagePath, format: u64 },
    #[error("`{0}` is no kind of definition a package carries")]
    UnknownKind(DefinitionPath),
    /// A zip that holds no store file, several, or cannot be read; the
    /// message is the archive's own.
    #[error("not a package archive: {0}")]
    Archive(String),
}

/// A zip file's first bytes.
const ZIP: &[u8] = b"PK\x03\x04";
/// The name a zipped package's store file has in its archive.
const ENTRY: &str = "package.redb";

/// The store a package file's `bytes` hold: a redb file, or a zip holding
/// one, kept in memory.
pub fn package(bytes: &[u8]) -> Result<Store, EngineError> {
    if !bytes.starts_with(ZIP) {
        return Ok(Store::from_bytes(bytes)?);
    }
    let archive = |error: zip::result::ZipError| PackageError::Archive(error.to_string());
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).map_err(archive)?;
    let stores: Vec<String> = zip
        .file_names()
        .map(|name| name.map(|name| name.into_owned()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(archive)?
        .into_iter()
        .filter(|name| name.ends_with(".redb"))
        .collect();
    let [name] = &stores[..] else {
        return Err(PackageError::Archive(format!(
            "it holds {} store files, not one",
            stores.len()
        ))
        .into());
    };
    let mut file = Vec::new();
    zip.by_name(name)
        .map_err(archive)?
        .read_to_end(&mut file)
        .map_err(|error| PackageError::Archive(error.to_string()))?;
    Ok(Store::from_bytes(&file)?)
}

/// A package's store file `bytes`, zipped.
pub fn zipped(bytes: &[u8]) -> Result<Vec<u8>, EngineError> {
    let archive = |error: zip::result::ZipError| PackageError::Archive(error.to_string());
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    zip.start_file(
        ENTRY,
        zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated),
    )
    .map_err(archive)?;
    zip.write_all(bytes)
        .map_err(|error| PackageError::Archive(error.to_string()))?;
    Ok(zip.finish().map_err(archive)?.into_inner())
}

impl Engine {
    /// Copies `contents` and everything it reaches into `to`, a new store,
    /// with a manifest naming it `name` at `version`. Every ref it names must
    /// be under `name`, so loading it never moves anyone else's refs.
    pub fn export(
        &self,
        name: &PackagePath,
        version: Version,
        contents: Contents,
        to: &Store,
    ) -> Result<Manifest, EngineError> {
        let mut closure = Closure::default();
        closure.refs.extend(contents.refs);
        closure.queue.extend(contents.definitions);
        loop {
            while let Some(definition) = closure.queue.pop() {
                if closure.definitions.insert(definition.clone()) {
                    self.reaches(&definition, &mut closure)?;
                }
            }
            // The views of every run's ontology come with the runs.
            let mut views = Vec::new();
            for ontology in self.ontologies(&closure)? {
                let view = crate::definitions::definition(&crate::view::view_path(&ontology));
                if self.store.read()?.definition(&view)?.is_some()
                    && !closure.definitions.contains(&view)
                {
                    views.push(view);
                }
            }
            if views.is_empty() {
                break;
            }
            closure.queue.extend(views);
        }
        for reference in &closure.refs {
            if !under(reference.as_str(), name) {
                return Err(PackageError::Outside {
                    reference: reference.clone(),
                    package: name.clone(),
                }
                .into());
            }
        }
        let manifest = Manifest {
            name: name.clone(),
            version,
            format: axionomy_store::FORMAT_VERSION,
            host_api: HOST_API,
            refs: closure.refs.iter().cloned().collect(),
            definitions: closure.definitions.iter().cloned().collect(),
        };
        self.store
            .export(
                Selection {
                    refs: manifest.refs.clone(),
                    definitions: manifest.definitions.clone(),
                    points: closure.points,
                    groups: closure.groups,
                },
                to,
            )
            .wait()?;
        let named = crate::definitions::definition(name);
        let bytes = postcard::to_allocvec(&manifest).expect("a manifest encodes");
        to.transact(Durability::Immediate, move |txn| {
            txn.define(&named, Some(&bytes))
        })
        .wait()?;
        Ok(manifest)
    }

    /// Merges `package` into the engine's store, after checking every
    /// manifest it holds is written in this version's store format and asks
    /// for a script host API this engine has. Names the store already gives
    /// something else keep what they are, and are returned.
    pub fn load(&self, package: &Store) -> Result<Loaded, EngineError> {
        let manifests = manifests(package)?;
        if manifests.is_empty() {
            return Err(PackageError::NoManifest.into());
        }
        if let Some(other) = manifests
            .iter()
            .find(|manifest| manifest.format != axionomy_store::FORMAT_VERSION)
        {
            return Err(PackageError::Format {
                package: other.name.clone(),
                format: other.format,
            }
            .into());
        }
        if let Some(newer) = manifests
            .iter()
            .find(|manifest| manifest.host_api > HOST_API)
        {
            return Err(PackageError::HostApi {
                package: newer.name.clone(),
                needs: newer.host_api,
            }
            .into());
        }
        let kept = self.store.merge(package).wait()?;
        Ok(Loaded { manifests, kept })
    }

    /// The manifests of the packages this engine's store holds.
    pub fn manifests(&self) -> Result<Vec<Manifest>, EngineError> {
        manifests(&self.store)
    }

    /// Everything the engine's store names under `name`: the refs, and the
    /// definitions of every kind but manifests. What a package of that name
    /// holds when nothing narrower is chosen.
    pub fn named_under(&self, name: &PackagePath) -> Result<Contents, EngineError> {
        let reader = self.store.read()?;
        let refs = reader
            .refs(name.as_str(), usize::MAX)?
            .into_iter()
            .map(|(reference, _)| reference)
            .filter(|reference| under(reference.as_str(), name))
            .collect();
        let definitions = reader
            .definitions("", usize::MAX)?
            .into_iter()
            .map(|(definition, _)| definition)
            .filter(|definition| {
                definition
                    .as_str()
                    .split_once('/')
                    .is_some_and(|(category, path)| category != "package" && under(path, name))
            })
            .collect();
        Ok(Contents { refs, definitions })
    }

    /// Adds what `definition` refers to to `closure`.
    fn reaches(
        &self,
        definition: &DefinitionPath,
        closure: &mut Closure,
    ) -> Result<(), EngineError> {
        let bytes = self
            .store
            .read()?
            .definition(definition)?
            .ok_or_else(|| EngineError::Undefined(definition.clone()))?;
        let (category, path) = definition
            .as_str()
            .split_once('/')
            .ok_or_else(|| PackageError::UnknownKind(definition.clone()))?;
        let mut calls = Vec::new();
        match category {
            "script" | "goal" | "package" => {}
            "objective" => {
                let Objective { measure, .. } = decoded(&bytes)?;
                let (Measure::State(call) | Measure::Steps(call)) = measure;
                calls.push(call);
            }
            "candidates" => {
                if let CandidateSet::Script(call) = decoded(&bytes)? {
                    calls.push(call);
                }
            }
            "strategy" => {
                let strategy: Strategy = decoded(&bytes)?;
                strategy_reaches(strategy, closure, &mut calls);
            }
            "experiment" => {
                let experiment: Experiment = decoded(&bytes)?;
                closure.refs.insert(experiment.start);
                closure
                    .queue
                    .push(crate::definitions::definition(&experiment.goal));
                for arm in experiment.arms {
                    closure.refs.extend(arm.start);
                    closure
                        .queue
                        .push(crate::definitions::definition(&arm.strategy));
                }
                for objective in &experiment.objectives {
                    closure
                        .queue
                        .push(crate::definitions::definition(objective));
                }
                let results = format!("results/{path}")
                    .parse::<DefinitionPath>()
                    .expect("a kind and a path make a path");
                if self.store.read()?.definition(&results)?.is_some() {
                    closure.queue.push(results);
                }
            }
            "results" => {
                let results: Results = decoded(&bytes)?;
                closure.groups.push(results.group);
                for arm in results.arms {
                    closure.points.push(arm.start);
                    closure
                        .points
                        .extend(arm.trials.iter().map(|trial| trial.end));
                }
            }
            "view" => {
                let view: View = decoded(&bytes)?;
                calls.extend(
                    [
                        view.labels,
                        view.formats,
                        view.carry,
                        view.scene,
                        view.leaderboards,
                    ]
                    .into_iter()
                    .flatten(),
                );
            }
            _ => return Err(PackageError::UnknownKind(definition.clone()).into()),
        }
        closure.queue.extend(
            calls
                .into_iter()
                .map(|call| crate::definitions::definition(&call.script)),
        );
        Ok(())
    }

    /// The ontologies of every run the closure's refs, points and groups name.
    fn ontologies(&self, closure: &Closure) -> Result<BTreeSet<OntologyPath>, EngineError> {
        let reader = self.store.read()?;
        let mut runs: Vec<RunId> = closure.points.iter().map(|point| point.run).collect();
        let mut groups: Vec<GroupId> = closure.groups.clone();
        for reference in &closure.refs {
            match reader.reference(reference)? {
                Some(Target::Step(point)) => runs.push(point.run),
                Some(Target::Run(run)) => runs.push(run),
                Some(Target::Group(group)) => groups.push(group),
                None => return Err(StoreError::UnknownRef(reference.clone()).into()),
            }
        }
        for group in groups {
            runs.extend(
                reader
                    .members(group, 0, usize::MAX)?
                    .into_iter()
                    .filter_map(|(_, member)| member.run),
            );
        }
        let mut ontologies = BTreeSet::new();
        for run in runs {
            let record = reader.run(run)?.ok_or(StoreError::UnknownRun(run))?;
            ontologies.insert(record.ontology);
        }
        Ok(ontologies)
    }
}

/// What an export has reached so far.
#[derive(Default)]
struct Closure {
    refs: BTreeSet<RefPath>,
    definitions: BTreeSet<DefinitionPath>,
    queue: Vec<DefinitionPath>,
    points: Vec<Point>,
    groups: Vec<GroupId>,
}

fn strategy_reaches(strategy: Strategy, closure: &mut Closure, calls: &mut Vec<ScriptCall>) {
    let mut candidates = |set| closure.queue.push(crate::definitions::definition(&set));
    match strategy {
        Strategy::Policy { policy, .. } => calls.push(policy),
        Strategy::Plan { plan } => calls.push(plan),
        Strategy::BreadthFirst { candidates: set } => candidates(set),
        Strategy::Cheapest {
            candidates: set,
            cost,
            heuristic,
        }
        | Strategy::BestFirst {
            candidates: set,
            cost,
            heuristic,
        } => {
            candidates(set);
            calls.push(cost);
            calls.extend(heuristic);
        }
        Strategy::BranchAndBound {
            candidates: set,
            cost,
            bound,
        } => {
            candidates(set);
            calls.extend([cost, bound]);
        }
        Strategy::Beam {
            candidates: set,
            score,
            ..
        } => {
            candidates(set);
            calls.push(score);
        }
        Strategy::Pareto {
            candidates: set,
            objectives,
            ..
        } => {
            candidates(set);
            closure
                .queue
                .extend(objectives.iter().map(crate::definitions::definition));
        }
        Strategy::Game {
            nature,
            actor,
            players,
            ..
        } => {
            calls.extend(nature);
            calls.push(actor);
            for player in players {
                match player {
                    Chooser::Policy(policy) => calls.push(policy),
                    Chooser::Minimax {
                        candidates: set,
                        evaluate,
                        ..
                    } => {
                        candidates(set);
                        calls.push(evaluate);
                    }
                    Chooser::Mcts {
                        candidates: set,
                        payoffs,
                        estimate,
                        chance,
                        rollout,
                        ..
                    } => {
                        candidates(set);
                        calls.extend([payoffs, estimate]);
                        calls.extend(chance.into_iter().chain(rollout));
                    }
                    Chooser::Ismcts {
                        candidates: set,
                        observe,
                        chance,
                        payoffs,
                        estimate,
                        rollout,
                        ..
                    } => {
                        candidates(set);
                        calls.extend([observe, chance, payoffs, estimate]);
                        calls.extend(rollout);
                    }
                }
            }
        }
    }
}

/// Whether `path` is `name` or a path under it.
fn under(path: &str, name: &PackagePath) -> bool {
    path.strip_prefix(name.as_str())
        .is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
}

fn decoded<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, EngineError> {
    postcard::from_bytes(bytes).map_err(|_| StoreError::Malformed("a definition").into())
}

/// The manifests `store` holds.
fn manifests(store: &Store) -> Result<Vec<Manifest>, EngineError> {
    store
        .read()?
        .definitions("package/", usize::MAX)?
        .into_iter()
        .map(|(_, bytes)| decoded(&bytes))
        .collect()
}
