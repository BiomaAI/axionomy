//! What a problem's scenes are built from: entities and metrics, keys
//! written from identifiers' paths, and the evidence that ties an entity to
//! the account or balance it shows.

use crate::{
    Format, ItemPath, MeasurePath, PathOntology, SceneAnchorView, SceneEntityRoleView,
    SceneEntityView, SceneEvidenceRefView, SceneGlyphView, SceneMetricView, SceneToneView, ViewId,
    ViewOntology, WrittenOntology,
};
use axionomy::{
    Quantity,
    path::{self, Path, Segments},
};

/// Keys written from identifiers' paths (D-044), so a scene, a metric or an
/// objective names a value the way storage, MCP and every view do.
pub trait Keyed {
    /// This path followed by `identifier`'s path: `route/direct-out` for
    /// `route` and `Route::DirectOut`.
    fn then(&self, identifier: &impl Segments) -> Self;
}

impl<K> Keyed for Path<K> {
    fn then(&self, identifier: &impl Segments) -> Self {
        self.join(&path::write(identifier))
    }
}

/// A scene entity citing no evidence yet and showing no metrics.
pub fn visual_entity(
    key: ItemPath,
    label: impl Into<String>,
    glyph: SceneGlyphView,
    anchor: SceneAnchorView,
    role: SceneEntityRoleView,
    tone: SceneToneView,
    status: Option<String>,
) -> SceneEntityView {
    SceneEntityView {
        id: ViewId::new(key, label),
        glyph,
        anchor,
        role,
        tone,
        status,
        account: None,
        evidence: Vec::new(),
        metrics: Vec::new(),
    }
}

/// A scene metric showing `value`, in `unit` if it has one.
pub fn visual_metric(
    key: MeasurePath,
    label: impl Into<String>,
    value: impl ToString,
    unit: Option<&str>,
) -> SceneMetricView {
    SceneMetricView {
        key,
        label: label.into(),
        value: value.to_string(),
        unit: unit.map(str::to_owned),
        previous: None,
    }
}

/// A scene metric showing a quantity of `asset`, written in the asset's
/// format, so a scene and the inspectors read it in the same unit.
pub fn asset_metric<A: Format>(
    key: MeasurePath,
    label: impl Into<String>,
    asset: &A,
    quantity: impl Into<Quantity>,
) -> SceneMetricView {
    let format = asset.format();
    SceneMetricView {
        key,
        label: label.into(),
        value: format.number(quantity.into()),
        unit: format.unit,
        previous: None,
    }
}

/// The key every view of `O` gives `account`, so a scene names the account
/// a frame shows.
pub fn account_key<O: WrittenOntology>(account: &O::Account) -> ItemPath {
    PathOntology::<O>::default().account(account).key
}

/// The key every view of `O` gives `asset`.
pub fn asset_key<O: WrittenOntology>(asset: &O::Asset) -> ItemPath {
    PathOntology::<O>::default().asset(asset).key
}

/// Shows `entity` as `account`, citing the account as its evidence.
pub fn link_account<O: WrittenOntology>(
    mut entity: SceneEntityView,
    account: &O::Account,
) -> SceneEntityView {
    let account = account_key::<O>(account);
    entity.account = Some(account.clone());
    entity
        .evidence
        .push(SceneEvidenceRefView::Account { account });
    entity
}

/// Shows `entity` as `account`, citing its balance of `asset` as evidence.
pub fn link_balance<O: WrittenOntology>(
    mut entity: SceneEntityView,
    account: &O::Account,
    asset: &O::Asset,
) -> SceneEntityView {
    let account = account_key::<O>(account);
    entity.account = Some(account.clone());
    entity.evidence.push(SceneEvidenceRefView::Balance {
        account,
        asset: asset_key::<O>(asset),
    });
    entity
}
