//! The history tree: every history a package's refs are on and every
//! branch growing from it, and the changes that grow it, which both engines
//! deliver as they happen.

use crate::{PointId, Refusal, packages::refs_of};
use axionomy_engine::{Change, Engine, PackagePath, Segment};
use axionomy_store::RefPath;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// One run of the history tree: the steps from `from`, the point it
/// branched at, or from the history's first state when `from` is none, up
/// to `tip`, its last point.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SegmentView {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<PointId>,
    pub tip: PointId,
}

impl From<Segment> for SegmentView {
    fn from(segment: Segment) -> Self {
        Self {
            from: segment.from.map(PointId::from),
            tip: segment.tip.into(),
        }
    }
}

/// A package's history tree: each history its refs are on, from its first
/// state, then every branch grown from a segment already listed, parents
/// first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct HistoriesView {
    pub segments: Vec<SegmentView>,
}

/// Something the store holds now that it did not before.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ChangeView {
    /// A run was recorded: a new segment of the history tree.
    Run { segment: SegmentView },
    /// A ref was made, moved or deleted: it names `point` now, or nothing.
    Ref {
        name: RefPath,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        point: Option<PointId>,
    },
    /// Definitions changed (a package loaded, an experiment's results
    /// recorded), or a client fell behind and missed changes: what it read,
    /// it reads again.
    Definitions,
}

impl From<&Change> for ChangeView {
    fn from(change: &Change) -> Self {
        match change {
            Change::Run(segment) => Self::Run {
                segment: (*segment).into(),
            },
            Change::Ref { name, point } => Self::Ref {
                name: name.clone(),
                point: point.map(PointId::from),
            },
            Change::Definitions => Self::Definitions,
        }
    }
}

/// The history tree of the package `name`.
pub fn histories(engine: &Engine, name: &PackagePath) -> Result<HistoriesView, Refusal> {
    let manifest = engine
        .manifests()?
        .into_iter()
        .find(|manifest| &manifest.name == name)
        .ok_or_else(|| Refusal::NotFound(format!("the store holds no package `{name}`")))?;
    let points: Vec<_> = refs_of(engine, &manifest)?
        .into_iter()
        .map(|reference| reference.point.point())
        .collect();
    Ok(HistoriesView {
        segments: engine
            .histories(&points)?
            .into_iter()
            .map(SegmentView::from)
            .collect(),
    })
}
