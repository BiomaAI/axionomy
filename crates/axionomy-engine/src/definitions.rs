//! What a package defines beside its economies: goals, objectives and
//! candidate sets, kept in the store by path and evaluated by the engine at
//! any point or on any economy (D-050).

use crate::script::ScriptPath;
use axionomy::{PathKind, path::Path};
use axionomy_search::pareto::ObjectiveDirection;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use thiserror::Error;

/// The kinds of path a package's definitions are named by.
pub mod kind {
    use axionomy::PathKind;

    /// A goal: balances an economy must hold.
    pub enum Goal {}
    /// An objective: a measure with a direction.
    pub enum Objective {}
    /// A set of candidate exchanges.
    pub enum Candidates {}
    /// A strategy: an algorithm with what it calls.
    pub enum Strategy {}
    /// An experiment: strategies run as arms under the same conditions.
    pub enum Experiment {}
    /// The results an experiment last recorded.
    pub enum Results {}
    /// A view: how people read an ontology's runs, named by the ontology.
    pub enum View {}

    impl PathKind for Goal {
        const SCHEMA: &'static str = "GoalPath";
        const EXAMPLE: &'static str = "maze/solved";
    }
    impl PathKind for Objective {
        const SCHEMA: &'static str = "ObjectivePath";
        const EXAMPLE: &'static str = "maze/energy";
    }
    impl PathKind for Candidates {
        const SCHEMA: &'static str = "CandidatesPath";
        const EXAMPLE: &'static str = "sokoban/moves";
    }
    impl PathKind for Strategy {
        const SCHEMA: &'static str = "StrategyPath";
        const EXAMPLE: &'static str = "maze/a-star";
    }
    impl PathKind for Experiment {
        const SCHEMA: &'static str = "ExperimentPath";
        const EXAMPLE: &'static str = "maze/routes";
    }
    impl PathKind for Results {
        const SCHEMA: &'static str = "ResultsPath";
        const EXAMPLE: &'static str = "maze/routes";
    }
    impl PathKind for View {
        const SCHEMA: &'static str = "ViewPath";
        const EXAMPLE: &'static str = "maze";
    }
}

pub type GoalPath = Path<kind::Goal>;
pub type ObjectivePath = Path<kind::Objective>;
pub type CandidatesPath = Path<kind::Candidates>;
pub type StrategyPath = Path<kind::Strategy>;
pub type ExperimentPath = Path<kind::Experiment>;

/// The name of a function a script defines: a Rhai identifier, such as
/// `heuristic` or `decide_seen`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ScriptFunction(Box<str>);

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error(
    "`{0}` is not a script function's name: letters, digits and `_`, not starting with a digit"
)]
pub struct ScriptFunctionError(String);

impl std::str::FromStr for ScriptFunction {
    type Err = ScriptFunctionError;

    fn from_str(name: &str) -> Result<Self, Self::Err> {
        let mut characters = name.chars();
        let starts = characters
            .next()
            .is_some_and(|first| first.is_ascii_alphabetic() || first == '_');
        if starts && characters.all(|next| next.is_ascii_alphanumeric() || next == '_') {
            Ok(Self(name.into()))
        } else {
            Err(ScriptFunctionError(name.to_owned()))
        }
    }
}

impl ScriptFunction {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ScriptFunction {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Serialize for ScriptFunction {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for ScriptFunction {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let name = std::borrow::Cow::<str>::deserialize(deserializer)?;
        name.parse().map_err(serde::de::Error::custom)
    }
}

/// One function of one script.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScriptCall {
    pub script: ScriptPath,
    pub function: ScriptFunction,
}

impl ScriptCall {
    pub fn new(script: ScriptPath, function: ScriptFunction) -> Self {
        Self { script, function }
    }
}

/// What an objective measures.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Measure {
    /// A function of the state at a point: `fn(state)`.
    State(ScriptCall),
    /// The sum, over every exchange of a point's history, of a function of
    /// the state before it, the exchange and the state after it:
    /// `fn(before, exchange, after)`, such as the interest a loan accrued.
    Steps(ScriptCall),
}

/// Where a candidate set's exchanges come from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CandidateSet {
    /// What a script function proposes for a state: `fn(state)`, returning
    /// an array of exchanges.
    Script(ScriptCall),
    /// These exchanges, the ones that apply at each state, in this order.
    Listed(Vec<axionomy::paths::Exchange>),
}

/// A measure with the direction that improves it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Objective {
    pub measure: Measure,
    pub direction: ObjectiveDirection,
}

/// The name a definition of `kind` at `path` is kept under in the store,
/// such as `script/maze/heuristic`.
pub(crate) fn definition<K: PathKind>(path: &Path<K>) -> axionomy_store::DefinitionPath {
    let kind = K::SCHEMA
        .strip_suffix("Path")
        .unwrap_or(K::SCHEMA)
        .to_ascii_lowercase();
    format!("{kind}/{path}")
        .parse()
        .expect("a kind and a path make a path")
}
