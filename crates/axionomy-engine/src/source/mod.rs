//! Builds a package from versioned source documents in an isolated store.
//! Only a successful build can be exported or installed.
mod compare;
pub use compare::{BuildComparison, compare};
mod files;
mod scripts;
mod tests;
use crate::{
    CandidateSet, CandidatesPath, Engine, EngineError, Experiment, ExperimentPath, GoalPath,
    Manifest, Objective, ObjectivePath, PackagePath, ScriptPath, Strategy, StrategyPath, Version,
    View,
};
use axionomy::{
    AccountPath, AssetPath, OntologyPath, ParameterPath, PathKind, Pattern, RatePath, RolePath,
    path::Path, paths,
};
use axionomy_store::{RefPath, Store};
pub use files::{Diagnostic, SourceFile, SourceFiles, SourceTree, TestPath};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Header {
    format: Format,
    name: PackagePath,
    version: Version,
    #[serde(default)]
    branches: BTreeMap<RefPath, Branch>,
    #[serde(default)]
    script_ontologies: BTreeMap<ScriptPath, BTreeSet<OntologyPath>>,
}
#[derive(Deserialize)]
enum Format {
    #[serde(rename = "axionomy.package/1")]
    V1,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Branch {
    from: RefPath,
    exchanges: Vec<paths::Exchange>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Vocabulary {
    accounts: BTreeSet<AccountPath>,
    assets: BTreeSet<AssetPath>,
    #[serde(default)]
    account_families: Vec<Family<AccountPath>>,
    #[serde(default)]
    asset_families: Vec<Family<AssetPath>>,
    #[serde(default)]
    asset_schema: Option<axionomy_units::AssetSchema<AssetPath>>,
    roles: BTreeSet<RolePath>,
    rates: BTreeSet<Pattern<RatePath>>,
    #[serde(default)]
    parameters: BTreeSet<ParameterPath>,
}
#[derive(Deserialize)]
#[serde(
    deny_unknown_fields,
    bound(deserialize = "T: axionomy::Identifier + serde::de::DeserializeOwned")
)]
struct Family<T> {
    pattern: Pattern<T>,
    variables: Vec<axionomy::Variable>,
}
impl<T: axionomy::Identifier> Family<T> {
    fn validate(&self) -> Result<(), String> {
        let used: BTreeSet<_> = self.pattern.variables().collect();
        let names: BTreeSet<_> = self
            .variables
            .iter()
            .map(axionomy::Variable::name)
            .collect();
        if used != names || names.is_empty() || names.len() != self.variables.len() {
            return Err(format!(
                "each variable of {} must have exactly one domain",
                self.pattern
            ));
        }
        for variable in &self.variables {
            let empty = match variable.domain() {
                axionomy::Domain::Range { first, last } => first > last,
                axionomy::Domain::Values(values) => values.is_empty(),
            };
            if empty {
                return Err(format!("empty domain for {}", variable.name()));
            }
        }
        Ok(())
    }
    fn matches(&self, parts: &[&str]) -> bool {
        self.pattern.read(parts).is_some_and(|args| {
            self.variables.iter().all(|v| {
                args.get(v.name())
                    .is_some_and(|value| v.domain().contains(value))
            })
        })
    }
}
fn declared<T: Ord + std::fmt::Display>(
    ids: &BTreeSet<T>,
    kind: &str,
    id: &T,
) -> Result<(), String> {
    if ids.contains(id) {
        Ok(())
    } else {
        Err(format!("undeclared {kind} `{id}`"))
    }
}
impl Vocabulary {
    fn account(&self, id: &AccountPath) -> Result<(), String> {
        if self.accounts.contains(id)
            || self
                .account_families
                .iter()
                .any(|f| f.matches(&id.segments().collect::<Vec<_>>()))
        {
            return Ok(());
        }
        Err(format!("undeclared account `{id}`"))
    }
    fn asset(&self, id: &AssetPath) -> Result<(), String> {
        if !self.assets.contains(id)
            && !self
                .asset_families
                .iter()
                .any(|f| f.matches(&id.segments().collect::<Vec<_>>()))
        {
            return Err(format!("undeclared asset `{id}`"));
        }
        if let Some(schema) = &self.asset_schema {
            let asset: axionomy_units::UnitAsset<AssetPath> =
                axionomy::path::read(id).map_err(|e| format!("asset {id}: {e}"))?;
            schema
                .validate_asset(&asset)
                .map_err(|e| format!("asset {id}: {e}"))?;
        }
        Ok(())
    }
    fn validate(&self, economy: &paths::Economy) -> Result<(), String> {
        economy.try_visit_accounts(|id| self.account(id))?;
        economy.try_visit_assets(|id| self.asset(id))?;
        for (key, rate) in economy.rates() {
            declared(&self.rates, "rate", key)?;
            for role in rate.roles() {
                declared(&self.roles, "role", role)?;
            }
            for parameter in rate.parameter_names() {
                declared(&self.parameters, "parameter", parameter)?;
            }
        }
        Ok(())
    }
}

#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Definitions {
    goals: BTreeMap<GoalPath, AuthoredGoal>,
    candidates: BTreeMap<CandidatesPath, CandidateSet>,
    candidate_ontologies: BTreeMap<CandidatesPath, OntologyPath>,
    objectives: BTreeMap<ObjectivePath, Objective>,
    strategies: BTreeMap<StrategyPath, Strategy>,
    experiments: BTreeMap<ExperimentPath, Experiment>,
    views: BTreeMap<OntologyPath, View>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AuthoredGoal {
    ontology: OntologyPath,
    goal: paths::Goal,
}

pub struct BuiltPackage {
    pub name: PackagePath,
    pub version: Version,
    pub engine: Engine,
    pub tests: Vec<TestPath>,
}
impl BuiltPackage {
    pub fn export(&self, store: &Store) -> Result<Manifest, EngineError> {
        self.engine.export(
            &self.name,
            self.version,
            self.engine.named_under(&self.name)?,
            store,
        )
    }
}

pub use axionomy_authoring::LawSetPath;
type LawSet = axionomy_authoring::LawSet<paths::Paths>;
type Instance = axionomy_authoring::Instance<paths::Paths>;

/// Parses and checks all files, then builds the source into a fresh store.
pub fn build(files: &SourceFiles) -> Result<BuiltPackage, Diagnostic> {
    let manifest = &SourceFile::Package;
    let header: Header = files::parse(
        manifest,
        files
            .get(manifest)
            .ok_or_else(|| Diagnostic::new(manifest, "missing package manifest"))?,
    )?;
    let Format::V1 = header.format;
    let engine = Engine::new(Store::memory().map_err(|e| Diagnostic::new(manifest, e))?)
        .map_err(|e| Diagnostic::new(manifest, e))?;
    let mut vocabularies = BTreeMap::<_, Vocabulary>::new();
    let mut law_sets = BTreeMap::<_, LawSet>::new();
    let mut instances = BTreeMap::<_, Instance>::new();
    let mut definitions = Definitions::default();
    let mut scripts = BTreeMap::new();
    let mut tests = BTreeMap::<_, tests::EconomicTest>::new();
    for (file, text) in files {
        match file {
            SourceFile::Package => {}
            SourceFile::Definitions => definitions = files::parse(file, text)?,
            SourceFile::Ontology(id) => {
                let vocab: Vocabulary = files::parse(file, text)?;
                for f in &vocab.account_families {
                    f.validate().map_err(|e| Diagnostic::new(file, e))?;
                }
                for f in &vocab.asset_families {
                    f.validate().map_err(|e| Diagnostic::new(file, e))?;
                }
                for id in &vocab.assets {
                    vocab.asset(id).map_err(|e| Diagnostic::new(file, e))?;
                }
                vocabularies.insert(id, vocab);
            }
            SourceFile::Laws(id) => {
                let declarations = files::parse(file, text)?;
                law_sets.insert(
                    id,
                    axionomy_authoring::expand(declarations)
                        .map_err(|e| Diagnostic::new(file, e))?,
                );
            }
            SourceFile::Instance(id) => {
                instances.insert(id, files::parse(file, text)?);
            }
            SourceFile::Script(id) => {
                scripts.insert(id, text.as_str());
            }
            SourceFile::Test(id) => {
                tests.insert(id.clone(), files::parse(file, text)?);
            }
        }
    }
    for id in header.script_ontologies.keys() {
        if !scripts.contains_key(id) {
            return Err(Diagnostic::new(
                manifest,
                format!("interface for missing script {id}"),
            ));
        }
    }
    for id in scripts.keys() {
        scoped(&header.name, id.as_str())
            .map_err(|e| Diagnostic::new(SourceFile::Script((*id).clone()), e))?;
    }
    let script_ids: Vec<_> = scripts.keys().copied().collect();
    for (id, instance) in instances {
        let file = SourceFile::Instance(id.clone());
        scoped(&header.name, id.as_str()).map_err(|e| Diagnostic::new(&file, e))?;
        let vocabulary = vocabularies.get(&instance.ontology).ok_or_else(|| {
            Diagnostic::new(&file, format!("unknown ontology `{}`", instance.ontology))
        })?;
        let ontology = instance.ontology.clone();
        let economy = instance
            .build(|set| law_sets.get(set))
            .map_err(|e| Diagnostic::new(&file, e))?;
        vocabulary
            .validate(&economy)
            .map_err(|e| Diagnostic::new(&file, e))?;
        let at = engine
            .author_in(ontology, economy)
            .and_then(|at| engine.refer(id, at));
        at.map_err(|e| Diagnostic::new(&file, e))?;
    }
    let mut branches = header.branches;
    while !branches.is_empty() {
        let next = branches
            .iter()
            .find(|(_, b)| !branches.contains_key(&b.from))
            .map(|(id, _)| id.clone())
            .ok_or_else(|| Diagnostic::new(manifest, "cycle in branch refs"))?;
        scoped(&header.name, next.as_str()).map_err(|e| Diagnostic::new(manifest, e))?;
        if engine.referred(&next).is_ok() {
            return Err(Diagnostic::new(
                manifest,
                format!("branch {next} shadows an instance"),
            ));
        }
        let branch = branches.remove(&next).expect("selected from branches");
        let start = engine
            .referred(&branch.from)
            .map_err(|e| Diagnostic::new(manifest, e))?;
        let ontology = engine
            .ontology(start)
            .map_err(|e| Diagnostic::new(manifest, e))?;
        for exchange in &branch.exchanges {
            vocabularies[&ontology]
                .exchange(exchange)
                .map_err(|e| Diagnostic::new(manifest, e))?;
        }
        let result = engine
            .apply(start, &branch.exchanges)
            .and_then(|end| engine.refer(&next, end));
        result.map_err(|e| Diagnostic::new(manifest, format!("branch {next}: {e}")))?;
    }
    while let Some((&id, _)) = scripts.first_key_value() {
        let mut load = id;
        let mut visiting = BTreeSet::new();
        loop {
            if !visiting.insert(load) {
                return Err(Diagnostic::new(
                    SourceFile::Script(load.clone()),
                    "cyclic script import",
                ));
            }
            match engine.keep_script(load, scripts[load]) {
                Ok(()) => {
                    scripts.remove(load);
                    break;
                }
                Err(EngineError::UnknownImport { ref import, .. })
                    if scripts.contains_key(import) =>
                {
                    load = *scripts.get_key_value(import).expect("present").0;
                }
                Err(e) => return Err(Diagnostic::new(SourceFile::Script(load.clone()), e)),
            }
        }
    }
    for id in script_ids {
        let file = SourceFile::Script(id.clone());
        let compiled = engine.compiled(id).map_err(|e| Diagnostic::new(&file, e))?;
        let interfaces = if let Some(ids) = header.script_ontologies.get(id) {
            ids.iter().collect::<Vec<_>>()
        } else if vocabularies.len() == 1 {
            vocabularies.keys().copied().collect()
        } else {
            return Err(Diagnostic::new(
                manifest,
                format!("declare script_ontologies for {id}"),
            ));
        };
        if interfaces.is_empty() {
            return Err(Diagnostic::new(
                manifest,
                format!("script {id} needs an ontology"),
            ));
        }
        for ontology in interfaces {
            let vocab = vocabularies.get(ontology).ok_or_else(|| {
                Diagnostic::new(
                    manifest,
                    format!("script {id} has unknown ontology {ontology}"),
                )
            })?;
            vocab.script(&file, &compiled.ast)?;
        }
    }
    let file = SourceFile::Definitions;
    let names = definitions
        .goals
        .keys()
        .map(Path::as_str)
        .chain(definitions.objectives.keys().map(Path::as_str))
        .chain(definitions.candidates.keys().map(Path::as_str))
        .chain(definitions.strategies.keys().map(Path::as_str))
        .chain(definitions.experiments.keys().map(Path::as_str));
    for name in names {
        scoped(&header.name, name).map_err(|e| Diagnostic::new(&file, e))?;
    }
    let err = |e: EngineError| Diagnostic::new(&file, e);
    let goal_types: BTreeMap<_, _> = definitions
        .goals
        .iter()
        .map(|(id, g)| (id.clone(), g.ontology.clone()))
        .collect();
    for (id, authored) in definitions.goals {
        let vocabulary = vocabularies.get(&authored.ontology).ok_or_else(|| {
            Diagnostic::new(
                &file,
                format!("goal {id}: unknown ontology {}", authored.ontology),
            )
        })?;
        for (account, assets) in authored.goal.requirements() {
            vocabulary
                .account(account)
                .map_err(|e| Diagnostic::new(&file, format!("goal {id}: {e}")))?;
            for (asset, _) in assets.iter() {
                vocabulary
                    .asset(asset)
                    .map_err(|e| Diagnostic::new(&file, format!("goal {id}: {e}")))?;
            }
        }
        engine.keep_goal(&id, &authored.goal).map_err(err)?;
    }
    for (id, set) in definitions.candidates {
        if let CandidateSet::Listed(exchanges) = &set {
            let ontology = match definitions.candidate_ontologies.get(&id) {
                Some(ontology) => vocabularies.get(ontology),
                None if vocabularies.len() == 1 => vocabularies.values().next(),
                None => None,
            }
            .ok_or_else(|| {
                Diagnostic::new(&file, format!("declare a candidate ontology for {id}"))
            })?;
            for exchange in exchanges {
                ontology
                    .exchange(exchange)
                    .map_err(|e| Diagnostic::new(&file, format!("candidates {id}: {e}")))?;
            }
        }
        engine.keep_candidates(&id, &set).map_err(err)?;
    }
    for (id, objective) in definitions.objectives {
        engine.keep_objective(&id, &objective).map_err(err)?;
    }
    for (id, strategy) in definitions.strategies {
        engine.keep_strategy(&id, &strategy).map_err(err)?;
    }
    for (id, view) in definitions.views {
        if !vocabularies.contains_key(&id) {
            return Err(Diagnostic::new(
                &file,
                format!("view of undeclared ontology {id}"),
            ));
        }
        engine.keep_view(&id, &view).map_err(err)?;
    }
    for (id, experiment) in definitions.experiments {
        for start in std::iter::once(&experiment.start)
            .chain(experiment.arms.iter().filter_map(|a| a.start.as_ref()))
        {
            let ontology = engine
                .ontology(engine.referred(start).map_err(err)?)
                .map_err(err)?;
            if goal_types.get(&experiment.goal) != Some(&ontology) {
                return Err(Diagnostic::new(
                    &file,
                    format!(
                        "experiment {id}: goal {} must belong to the ontology {ontology} of {start}",
                        experiment.goal
                    ),
                ));
            }
        }
        engine.keep_experiment(&id, &experiment).map_err(err)?;
    }
    for (id, test) in &tests {
        test.run(&engine, &vocabularies)
            .map_err(|e| Diagnostic::new(SourceFile::Test(id.clone()), e))?;
    }
    engine
        .define(&source_path(&header.name), files)
        .map_err(|e| Diagnostic::new(manifest, e))?;
    Ok(BuiltPackage {
        tests: tests.into_keys().collect(),
        name: header.name,
        version: header.version,
        engine,
    })
}

fn scoped(package: &PackagePath, id: &str) -> Result<(), String> {
    if id == package.as_str()
        || id
            .strip_prefix(package.as_str())
            .is_some_and(|s| s.starts_with('/'))
    {
        Ok(())
    } else {
        Err(format!("`{id}` is outside package `{package}`"))
    }
}

enum Source {}
impl PathKind for Source {
    const SCHEMA: &'static str = "SourcePath";
}
fn source_path(name: &PackagePath) -> Path<Source> {
    name.as_str().parse().expect("a package name is a path")
}

impl Engine {
    /// The original UTF-8 files carried by a package compiled from source.
    pub fn package_source(&self, name: &PackagePath) -> Result<SourceFiles, EngineError> {
        self.defined(&source_path(name))
    }
}
