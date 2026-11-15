//! [`Labeled`] paths: a path and the words people read for it.

use axionomy::{PathKind, path::Path};
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
