//! How a problem names and writes its values: a [`Label`] for each
//! identifier, a [`Format`] for each asset's quantities, the
//! [`LabeledOntology`] that keys every identifier by its path and shows it
//! with those labels, the problem's scene and its leaderboards, and
//! [`Labeled`] paths for whatever else a front end shows by name.

use crate::{
    LeaderboardView, PathOntology, QuantityFormat, Scene, ViewId, ViewOntology, WrittenOntology,
};
use axionomy::{Economy, Ontology, PathKind, path::Path};
use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Deserialize, Serialize};
use std::{
    borrow::Cow,
    fmt,
    hash::{Hash, Hasher},
};

/// A path and the words people read for it, such as a phase of solver work
/// (`a-star-frontier`, `A* frontier`) or a problem's family. Front ends join
/// on the key and show the label they receive, so whoever declares the path
/// also says how it reads; no front end keeps a table of its own.
#[derive(Serialize, Deserialize)]
#[serde(bound = "")]
pub struct Labeled<K> {
    pub key: Path<K>,
    pub label: String,
}

impl<K> Labeled<K> {
    pub fn new(key: Path<K>, label: impl Into<String>) -> Self {
        Self {
            key,
            label: label.into(),
        }
    }
}

impl<K> Clone for Labeled<K> {
    fn clone(&self) -> Self {
        Self::new(self.key.clone(), self.label.clone())
    }
}

impl<K> PartialEq for Labeled<K> {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key && self.label == other.label
    }
}

impl<K> Eq for Labeled<K> {}

impl<K> Hash for Labeled<K> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.key.hash(state);
        self.label.hash(state);
    }
}

impl<K> fmt::Debug for Labeled<K> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} ({:?})", self.key, self.label)
    }
}

impl<K> fmt::Display for Labeled<K> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.label)
    }
}

/// Named for its kind: `LabeledPhase` for a labeled `PhasePath`.
impl<K: PathKind> JsonSchema for Labeled<K> {
    fn schema_name() -> Cow<'static, str> {
        format!(
            "Labeled{}",
            K::SCHEMA.strip_suffix("Path").unwrap_or(K::SCHEMA)
        )
        .into()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "description": format!(
                "A {} and the words people read for it. Front ends join on `key` and show `label`.",
                K::SCHEMA
            ),
            "type": "object",
            "properties": {
                "key": generator.subschema_for::<Path<K>>(),
                "label": { "type": "string" }
            },
            "required": ["key", "label"]
        })
    }
}

/// A value as people read it: an account, asset, rate or role of an
/// ontology, or any value a label names.
pub trait Label {
    fn label(&self) -> String;
}

/// How a view writes quantities of an asset: a count unless the problem
/// stores the asset scaled.
pub trait Format {
    fn format(&self) -> QuantityFormat {
        QuantityFormat::COUNT
    }
}

/// Assets written as plain counts: `counted!(maze::Asset)`.
#[macro_export]
macro_rules! counted {
    ($($type:path),+ $(,)?) => {$(
        impl $crate::Format for $type {}
    )+};
}

/// Values labeled by their `Debug` output as words, as [`crate::words`]
/// writes them: `debug_labels!(maze::Role)`.
#[macro_export]
macro_rules! debug_labels {
    ($($type:path),+ $(,)?) => {$(
        impl $crate::Label for $type {
            fn label(&self) -> String {
                $crate::words(self)
            }
        }
    )+};
}

/// The scene of the economy a snapshot shows, given the snapshot's index.
pub type SceneFn<O> = fn(u64, &Economy<O>) -> Option<Scene>;
/// The leaderboards of the economy a snapshot shows, given its index.
pub type LeaderboardsFn<O> = fn(u64, &Economy<O>) -> Vec<LeaderboardView>;

/// A view of an ontology whose identifiers are labeled: keys from
/// [`PathOntology`], under the ontology's name; labels from [`Label`];
/// formats from [`Format`]; and the problem's scene and leaderboards.
pub struct LabeledOntology<O: Ontology> {
    paths: PathOntology<O>,
    scene: Option<SceneFn<O>>,
    leaderboards: Option<LeaderboardsFn<O>>,
}

impl<O: Ontology> LabeledOntology<O> {
    /// A view with `scene`, if any, and no leaderboards.
    pub fn new(scene: Option<SceneFn<O>>) -> Self {
        Self {
            paths: PathOntology::default(),
            scene,
            leaderboards: None,
        }
    }

    pub fn with_leaderboards(mut self, leaderboards: LeaderboardsFn<O>) -> Self {
        self.leaderboards = Some(leaderboards);
        self
    }

    fn relabel<T: Label>(mut id: ViewId, value: &T) -> ViewId {
        id.label = value.label();
        id
    }
}

impl<O: WrittenOntology> ViewOntology<O> for LabeledOntology<O>
where
    O::Account: Label,
    O::Asset: Label + Format,
    O::Rate: Label,
    O::Role: Label,
{
    fn account(&self, id: &O::Account) -> ViewId {
        Self::relabel(self.paths.account(id), id)
    }

    fn asset(&self, id: &O::Asset) -> ViewId {
        Self::relabel(self.paths.asset(id), id)
    }

    fn rate(&self, id: &O::Rate) -> ViewId {
        Self::relabel(self.paths.rate(id), id)
    }

    fn role(&self, id: &O::Role) -> ViewId {
        Self::relabel(self.paths.role(id), id)
    }

    fn format(&self, asset: &O::Asset) -> QuantityFormat {
        Format::format(asset)
    }

    fn scene(&self, index: u64, economy: &Economy<O>) -> Option<Scene> {
        self.scene.and_then(|scene| scene(index, economy))
    }

    fn leaderboards(&self, index: u64, economy: &Economy<O>) -> Vec<LeaderboardView> {
        self.leaderboards
            .map_or_else(Vec::new, |leaderboards| leaderboards(index, economy))
    }
}
