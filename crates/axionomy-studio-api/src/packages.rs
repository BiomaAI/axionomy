//! Packages (D-048): what the engine's store holds of each, and loading one
//! from its bytes.

use crate::{Natural, PackageVersion, PointId, RefView, Refusal};
use axionomy::{PathKind, path::Path};
use axionomy_engine::{
    CandidatesPath, Engine, EngineError, Experiment, ExperimentPath, GoalPath, Loaded,
    ObjectivePath, PackagePath, StrategyPath,
};
use axionomy_search::pareto::ObjectiveDirection;
use axionomy_store::{DefinitionPath, RefPath};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// The packages the engine's store holds, in name order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PackageList {
    pub packages: Vec<PackageView>,
}

/// One package: what its manifest says of it, the points its refs name, and
/// what it defines for them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PackageView {
    pub name: PackagePath,
    pub version: PackageVersion,
    /// The store format the package is written in.
    pub format: u64,
    /// The script host API its scripts are written for.
    pub host_api: u64,
    /// Its refs that name a point, in name order: those its manifest names
    /// and any made under its name since, such as a branch kept as a ref.
    pub refs: Vec<RefView>,
    pub experiments: Vec<ExperimentView>,
    pub strategies: Vec<StrategyPath>,
    pub objectives: Vec<ObjectiveView>,
    pub goals: Vec<GoalPath>,
    /// Its candidate sets: the exchanges each offers at a point.
    pub candidates: Vec<CandidatesPath>,
}

/// An objective and the direction that improves it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ObjectiveView {
    pub objective: ObjectivePath,
    pub direction: ObjectiveDirection,
}

/// Strategies run as arms under the same conditions, measured alike.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ExperimentView {
    pub experiment: ExperimentPath,
    /// The ref naming the point the arms start from.
    pub start: RefPath,
    pub arms: Vec<ArmView>,
    pub objectives: Vec<ObjectiveView>,
    pub seed: Natural,
    pub budget: Natural,
    /// How many trials each arm runs.
    pub trials: Natural,
    /// Whether the store keeps results the experiment recorded.
    pub recorded: bool,
}

/// One arm: a strategy, from the experiment's start or from a ref of its
/// own.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ArmView {
    pub strategy: StrategyPath,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start: Option<RefPath>,
}

/// What loading a package did: the packages it held, and the names the
/// store already gave something else, which it kept.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LoadedView {
    pub packages: Vec<PackagePath>,
    pub kept: KeptView,
}

/// Names a load found the store giving something else, which keep what
/// they are.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct KeptView {
    pub refs: Vec<RefPath>,
    pub definitions: Vec<DefinitionPath>,
}

/// The sample packages a static site serves beside Studio, each at
/// `packages/<name>.zip`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SamplePackages {
    pub packages: Vec<PackagePath>,
}

impl From<&Loaded> for LoadedView {
    fn from(loaded: &Loaded) -> Self {
        Self {
            packages: loaded
                .manifests
                .iter()
                .map(|manifest| manifest.name.clone())
                .collect(),
            kept: KeptView {
                refs: loaded.kept.refs.clone(),
                definitions: loaded.kept.definitions.clone(),
            },
        }
    }
}

/// Every package the store holds.
pub fn packages(engine: &Engine) -> Result<PackageList, Refusal> {
    let mut manifests = engine.manifests()?;
    manifests.sort_by(|left, right| left.name.cmp(&right.name));
    let mut packages = Vec::with_capacity(manifests.len());
    for manifest in manifests {
        let mut named: BTreeSet<RefPath> = manifest.refs.iter().cloned().collect();
        named.extend(engine.named_under(&manifest.name)?.refs);
        let mut refs = Vec::with_capacity(named.len());
        for name in named {
            match engine.referred(&name) {
                Ok(point) => refs.push(RefView {
                    name,
                    point: PointId::from(point),
                }),
                // A ref naming a run or a group, or nothing, names no point.
                Err(EngineError::UnknownRef(_)) => {}
                Err(error) => return Err(error.into()),
            }
        }
        let experiments = carried(&manifest.definitions)
            .into_iter()
            .map(|path: ExperimentPath| {
                let experiment = engine.experiment(&path)?;
                experiment_view(engine, path, experiment)
            })
            .collect::<Result<_, EngineError>>()?;
        packages.push(PackageView {
            experiments,
            strategies: carried(&manifest.definitions),
            objectives: objectives(engine, &carried(&manifest.definitions))?,
            goals: carried(&manifest.definitions),
            candidates: carried(&manifest.definitions),
            name: manifest.name,
            version: PackageVersion(manifest.version),
            format: manifest.format,
            host_api: manifest.host_api,
            refs,
        });
    }
    Ok(PackageList { packages })
}

/// An experiment as Studio shows it.
fn experiment_view(
    engine: &Engine,
    path: ExperimentPath,
    experiment: Experiment,
) -> Result<ExperimentView, EngineError> {
    let recorded = match engine.results(&path) {
        Ok(_) => true,
        Err(EngineError::Undefined(_)) => false,
        Err(error) => return Err(error),
    };
    Ok(ExperimentView {
        start: experiment.start,
        arms: experiment
            .arms
            .into_iter()
            .map(|arm| ArmView {
                strategy: arm.strategy,
                start: arm.start,
            })
            .collect(),
        objectives: objectives(engine, &experiment.objectives)?,
        seed: experiment.seed.into(),
        budget: experiment.budget.into(),
        trials: experiment.trials.into(),
        recorded,
        experiment: path,
    })
}

/// Objectives with their directions.
pub(crate) fn objectives(
    engine: &Engine,
    objectives: &[ObjectivePath],
) -> Result<Vec<ObjectiveView>, EngineError> {
    objectives
        .iter()
        .map(|objective| {
            Ok(ObjectiveView {
                direction: engine.objective(objective)?.direction,
                objective: objective.clone(),
            })
        })
        .collect()
}

/// Loads the package file `bytes`, a store file or a zip of one: its
/// manifests checked, then merged into the store.
pub fn load(engine: &Engine, bytes: &[u8]) -> Result<LoadedView, Refusal> {
    let package = axionomy_engine::package(bytes)
        .map_err(|error| Refusal::BadRequest(format!("not a package file: {error}")))?;
    Ok(LoadedView::from(&engine.load(&package)?))
}

/// The paths of the definitions of `K`'s kind among `definitions`, such as
/// the strategies among `strategy/maze/a-star` and `goal/maze/solved`.
fn carried<K: PathKind>(definitions: &[DefinitionPath]) -> Vec<Path<K>> {
    let kind = K::SCHEMA
        .strip_suffix("Path")
        .unwrap_or(K::SCHEMA)
        .to_ascii_lowercase();
    definitions
        .iter()
        .filter_map(|definition| {
            let (category, path) = definition.as_str().split_once('/')?;
            (category == kind).then(|| path.parse().ok()).flatten()
        })
        .collect()
}
