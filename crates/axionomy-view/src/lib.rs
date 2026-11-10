//! Runtime-neutral playback and presentation contracts for Axionomy.
//!
//! This crate deliberately contains no world model of its own. A view document
//! is derived by replaying exchanges through [`axionomy::Economy`], and optional
//! scenes are read-only projections supplied by the caller.

use axionomy::{
    AccountAssessment, AccountDelta, ApplyError, Basket, ConditionPath, Economy, Exchange,
    ExchangeAssessment, Goal, InvariantPath, Ontology, OntologyPath, ParameterPath, PathKind,
    Quantity, QuantityComparison, QuantityExpression, Receipt, Rounding, Trace,
    path::{Path, slug_segment},
};

pub use axionomy::path::slug;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{BTreeMap, HashSet},
    fmt::Debug,
    marker::PhantomData,
};
use thiserror::Error;

/// The kinds of identifiers in a view. Each is a distinct path type, so a
/// metric's key can never stand where an account's is expected.
pub mod kind {
    use super::PathKind;

    /// Anything a view shows and joins on: accounts, assets, rates, roles,
    /// scene entities, graph nodes and edges, lanes, rows, columns, paths,
    /// annotations, spans and document sources.
    pub enum Item {}
    /// A measured value: scene and observation metrics, objectives, Pareto
    /// axes and leaderboards.
    pub enum Measure {}
    /// A semantic class a renderer may style, such as `location` or `current`.
    pub enum Class {}
    /// One document of a study, such as one strategy's outcome.
    pub enum Document {}
    /// A proposal assessed against a snapshot.
    pub enum Proposal {}
    /// A phase of solver work, such as `a-star-frontier`.
    pub enum Phase {}
    /// The algorithm doing solver work, such as `a-star`.
    pub enum Algorithm {}

    impl PathKind for Item {
        const SCHEMA: &'static str = "ItemPath";
    }
    impl PathKind for Measure {
        const SCHEMA: &'static str = "MeasurePath";
    }
    impl PathKind for Class {
        const SCHEMA: &'static str = "ClassPath";
    }
    impl PathKind for Document {
        const SCHEMA: &'static str = "DocumentPath";
    }
    impl PathKind for Proposal {
        const SCHEMA: &'static str = "ProposalPath";
    }
    impl PathKind for Phase {
        const SCHEMA: &'static str = "PhasePath";
    }
    impl PathKind for Algorithm {
        const SCHEMA: &'static str = "AlgorithmPath";
    }
}

/// What a view shows, such as `maze/account/agent` or `node/start`.
pub type ItemPath = Path<kind::Item>;
/// A measured value, such as `energy` or `states-expanded`.
pub type MeasurePath = Path<kind::Measure>;
/// A semantic class, such as `location` or `search-explored`.
pub type ClassPath = Path<kind::Class>;
/// A document of a study, such as `a-star` or `deadlock`.
pub type DocumentPath = Path<kind::Document>;
/// A proposal, such as `locked-door`.
pub type ProposalPath = Path<kind::Proposal>;
/// A phase of solver work, such as `monte-carlo` or `a-star-frontier`.
pub type PhasePath = Path<kind::Phase>;
/// An algorithm, such as `a-star` or `exact-pareto-search`.
pub type AlgorithmPath = Path<kind::Algorithm>;

/// How people read quantities of one asset. The kernel counts atoms; a format
/// shows `atoms ÷ 10^scale` with a unit, so a balance of 1,250 cent atoms
/// reads `12.50 USD` and a WAD-scaled rate of 5 × 10^16 reads `5 %`.
///
/// A format never rounds. It writes every nonzero digit of the exact value,
/// trimming trailing zeros but keeping at least `digits` digits after the
/// point. It is presentation only: two assets with different formats are
/// still two assets, and equal atoms are equal whatever their format.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct QuantityFormat {
    /// The power of ten one displayed unit is made of, in atoms.
    pub scale: u8,
    /// Digits always written after the point, padding with zeros.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub digits: u8,
    /// The unit people read after the number, such as `kg`, `USD` or `%`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
}

fn is_zero(value: &u8) -> bool {
    *value == 0
}

impl QuantityFormat {
    /// A count of atoms, written as it is: the format of every asset that
    /// declares none.
    pub const COUNT: Self = Self {
        scale: 0,
        digits: 0,
        unit: None,
    };

    /// `atoms ÷ 10^scale`.
    pub const fn decimal(scale: u8) -> Self {
        Self {
            scale,
            digits: 0,
            unit: None,
        }
    }

    /// Writes at least `digits` digits after the point.
    pub const fn digits(mut self, digits: u8) -> Self {
        self.digits = digits;
        self
    }

    /// Writes `unit` after the number.
    pub fn unit(mut self, unit: impl Into<String>) -> Self {
        self.unit = Some(unit.into());
        self
    }

    /// The exact quantity as people read it, unit included.
    pub fn write(&self, quantity: Quantity) -> String {
        let number = self.number(quantity);
        match &self.unit {
            Some(unit) => format!("{number} {unit}"),
            None => number,
        }
    }

    /// The exact quantity as people read it, without the unit: for a view
    /// that shows the unit on its own.
    pub fn number(&self, quantity: Quantity) -> String {
        let atoms = quantity.to_string();
        let scale = usize::from(self.scale);
        let atoms = if atoms.len() <= scale {
            format!("{atoms:0>width$}", width = scale + 1)
        } else {
            atoms
        };
        let (whole, fraction) = atoms.split_at(atoms.len() - scale);
        let fraction = fraction.trim_end_matches('0');
        let digits = usize::from(self.digits).max(fraction.len());
        if digits == 0 {
            whole.to_owned()
        } else {
            format!("{whole}.{fraction:0<digits$}")
        }
    }
}

/// A browser-safe identity for an arbitrary user-defined ontology value.
///
/// `key` is used for joins, `label` is for people, and `value` is optional
/// diagnostic context. None of these fields replaces the underlying Rust value
/// as semantic authority.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ViewId {
    pub key: ItemPath,
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<Value>,
}

impl ViewId {
    pub fn new(key: ItemPath, label: impl Into<String>) -> Self {
        Self {
            key,
            label: label.into(),
            value: None,
        }
    }

    pub fn with_value(mut self, value: impl Serialize) -> Result<Self, serde_json::Error> {
        self.value = Some(serde_json::to_value(value)?);
        Ok(self)
    }
}

/// An exact, non-negative quantity encoded as text for JavaScript safety.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct ExactQuantity(pub String);

impl From<Quantity> for ExactQuantity {
    fn from(value: Quantity) -> Self {
        Self(value.to_string())
    }
}

impl From<&Quantity> for ExactQuantity {
    fn from(value: &Quantity) -> Self {
        Self::from(*value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AssetQuantityView {
    pub asset: ViewId,
    pub quantity: ExactQuantity,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AccountView {
    pub account: ViewId,
    pub balances: Vec<AssetQuantityView>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RoleBindingView {
    pub role: ViewId,
    pub account: ViewId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ExchangeView {
    pub rate: ViewId,
    pub units: ExactQuantity,
    pub bindings: Vec<RoleBindingView>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parameters: Vec<ExchangeParameterView>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ExchangeParameterView {
    pub name: ParameterPath,
    pub value: ExactQuantity,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AccountDeltaView {
    pub account: ViewId,
    pub consumed: Vec<AssetQuantityView>,
    pub produced: Vec<AssetQuantityView>,
    pub preserved: Vec<AssetQuantityView>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AccountAssessmentView {
    pub account: ViewId,
    pub available: Vec<AssetQuantityView>,
    pub required: Vec<AssetQuantityView>,
    pub consumed: Vec<AssetQuantityView>,
    pub produced: Vec<AssetQuantityView>,
    pub preserved: Vec<AssetQuantityView>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AssessmentStatusView {
    Applicable,
    Infeasible,
    Invalid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AssessmentIssueKindView {
    MissingRate,
    MissingBinding,
    UnknownBinding,
    RolesMustDiffer,
    MissingAccount,
    ZeroUnits,
    MissingParameter,
    UnknownParameter,
    RateOverflow,
    RateLawEvaluation,
    RateConditionFailed,
    Infeasible,
    BalanceOverflow,
    InvariantViolation,
    AccountExists,
    OpeningShared,
    ClosedNotEmpty,
}

/// One structured reason why a proposal is invalid.
///
/// Subjects retain the browser-facing identities of the roles, accounts,
/// rates, and assets involved instead of flattening distinct failures into the
/// same generic error string.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AssessmentIssueView {
    pub kind: AssessmentIssueKindView,
    pub message: String,
    pub subjects: Vec<ViewId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AssessmentView {
    pub status: AssessmentStatusView,
    pub accounts: Vec<AccountAssessmentView>,
    pub projected_deltas: Vec<AccountDeltaView>,
    pub shortfalls: Vec<AccountShortfallView>,
    pub issues: Vec<AssessmentIssueView>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AccountShortfallView {
    pub account: ViewId,
    pub missing: Vec<AssetQuantityView>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ReceiptView {
    pub deltas: Vec<AccountDeltaView>,
}

/// A rate role's complete consume/produce/preserve contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RateRoleView {
    pub role: ViewId,
    pub consumed: Vec<AssetQuantityView>,
    pub produced: Vec<AssetQuantityView>,
    pub preserved: Vec<AssetQuantityView>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ComputedRateAmountView {
    pub role: ViewId,
    pub asset: ViewId,
    pub quantity: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RateConditionView {
    pub name: ConditionPath,
    pub left: String,
    pub comparison: String,
    pub right: String,
}

/// One immutable transition rule in the closed model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RateView {
    pub rate: ViewId,
    pub roles: Vec<RateRoleView>,
    pub distinct_roles: Vec<[ViewId; 2]>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub computed_consumed: Vec<ComputedRateAmountView>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub computed_produced: Vec<ComputedRateAmountView>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub computed_preserved: Vec<ComputedRateAmountView>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conditions: Vec<RateConditionView>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct GoalRequirementView {
    pub account: ViewId,
    pub required: Vec<AssetQuantityView>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct InvariantTermView {
    pub asset: ViewId,
    pub weight: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct InvariantView {
    pub name: InvariantPath,
    pub terms: Vec<InvariantTermView>,
}

/// Read-only explanation of the authoritative economy definition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ModelView {
    pub rates: Vec<RateView>,
    pub goal: Vec<GoalRequirementView>,
    pub invariants: Vec<InvariantView>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct GraphNodeView {
    pub id: ViewId,
    #[serde(default)]
    pub classes: Vec<ClassPath>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub x: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub y: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct GraphEdgeView {
    pub id: ItemPath,
    pub source: ItemPath,
    pub target: ItemPath,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default)]
    pub classes: Vec<ClassPath>,
}

/// A semantic icon key owned by the portable Rust contract. The browser maps
/// these keys to its chosen icon implementation; documents never contain SVG,
/// CSS class names masquerading as meaning, or React component names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SceneGlyphView {
    Agent,
    Robot,
    Vehicle,
    Package,
    Location,
    Goal,
    Key,
    Door,
    Fuel,
    Money,
    Product,
    Person,
    Organization,
    Tool,
    Material,
    Food,
    Temperature,
    Energy,
    Clock,
    Hazard,
    Weather,
    Repair,
    Sensor,
    Information,
    Move,
    Constraint,
    Task,
    Machine,
    Token,
    Shield,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SceneToneView {
    Neutral,
    Active,
    Goal,
    Success,
    Warning,
    Danger,
    Uncertain,
    Muted,
}

/// How a semantic entity relates to the geometric surface it is anchored to.
///
/// This is presentation metadata only. It lets generic viewers distinguish a
/// place from something occupying it, a task attached to it, state describing
/// it, or scenario context that should remain outside the topology.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SceneEntityRoleView {
    Structure,
    Occupant,
    Attachment,
    State,
    Context,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SceneAnchorView {
    GraphNode {
        node: ItemPath,
    },
    GraphEdge {
        edge: ItemPath,
        #[serde(skip_serializing_if = "Option::is_none")]
        progress: Option<f64>,
    },
    GridCell {
        x: u32,
        y: u32,
    },
    MatrixCell {
        row: ItemPath,
        column: ItemPath,
    },
    Timeline {
        lane: ItemPath,
        at: u64,
    },
    /// Attach presentation state to another stable scene entity. This is used
    /// for inventory, cargo, equipment, and similar ownership relationships
    /// that would be misleading if attached only to a shared location.
    Entity {
        entity: ItemPath,
    },
    Unanchored,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SceneMetricView {
    pub key: MeasurePath,
    pub label: String,
    /// Exact text preserves arbitrarily large quantities and caller units.
    pub value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous: Option<String>,
}

/// Exact economic evidence illustrated by a disposable scene entity.
///
/// These references never create state or authorize a transition. They let a
/// renderer connect replay-derived motion and emphasis back to the accounts
/// and balances that prove why the picture changed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SceneEvidenceRefView {
    Account { account: ItemPath },
    Balance { account: ItemPath, asset: ItemPath },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SceneEntityView {
    pub id: ViewId,
    pub glyph: SceneGlyphView,
    pub anchor: SceneAnchorView,
    pub role: SceneEntityRoleView,
    pub tone: SceneToneView,
    /// A short status line for people, such as `2 crates aboard`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account: Option<ItemPath>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence: Vec<SceneEvidenceRefView>,
    #[serde(default)]
    pub metrics: Vec<SceneMetricView>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ScenePathStatusView {
    Available,
    Candidate,
    Explored,
    Traversed,
    Current,
    Incumbent,
    Rejected,
    Blocked,
    Uncertain,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ScenePathView {
    pub id: ItemPath,
    pub label: String,
    pub anchors: Vec<SceneAnchorView>,
    pub status: ScenePathStatusView,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SceneAnnotationView {
    pub id: ItemPath,
    pub label: String,
    pub anchor: SceneAnchorView,
    pub tone: SceneToneView,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SceneLegendView {
    pub label: String,
    pub glyph: SceneGlyphView,
    pub tone: SceneToneView,
}

/// One constant-product pool projected from an authoritative replay snapshot.
/// Values remain exact text so renderers cannot silently round economic state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct MarketPoolView {
    pub id: ViewId,
    pub base_asset: ViewId,
    pub quote_asset: ViewId,
    pub base_reserve: ExactQuantity,
    pub quote_reserve: ExactQuantity,
    /// Quote per base, scaled by 1,000.
    pub price_milli: ExactQuantity,
    pub product: ExactQuantity,
    pub issued_liquidity: ExactQuantity,
    pub fee_numerator: u64,
    pub fee_denominator: u64,
    pub account: ItemPath,
}

/// One participant positioned around a market pool. Balances and status are
/// replay-derived presentation data linked back to the represented account.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct MarketActorView {
    pub id: ViewId,
    pub account: ItemPath,
    pub glyph: SceneGlyphView,
    pub tone: SceneToneView,
    pub x: f64,
    pub y: f64,
    pub energy: ExactQuantity,
    pub credit: ExactQuantity,
    pub liquidity: ExactQuantity,
    pub utility: ExactQuantity,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
}

/// The geometric substrate for a derived scene. Surface geometry and semantic
/// entities are deliberately separate so one vehicle can move through a graph,
/// grid, matrix, or timeline without inventing four domain models.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SceneSurfaceView {
    Graph {
        nodes: Vec<GraphNodeView>,
        edges: Vec<GraphEdgeView>,
        #[serde(skip_serializing_if = "Option::is_none")]
        focus: Option<ItemPath>,
    },
    Grid {
        width: u32,
        height: u32,
        cells: Vec<GridCellView>,
    },
    Matrix {
        rows: Vec<ViewId>,
        columns: Vec<ViewId>,
        cells: Vec<MatrixCellView>,
    },
    Timeline {
        lanes: Vec<TimelineLaneView>,
        spans: Vec<TimelineSpanView>,
        #[serde(skip_serializing_if = "Option::is_none")]
        cursor: Option<u64>,
    },
    Market {
        pool: Box<MarketPoolView>,
        actors: Vec<MarketActorView>,
    },
}

/// Optional problem picture derived from the economic state.
/// Pictures explain and animate; they never decide whether an exchange applies.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Scene {
    pub title: String,
    pub surface: SceneSurfaceView,
    #[serde(default)]
    pub entities: Vec<SceneEntityView>,
    #[serde(default)]
    pub paths: Vec<ScenePathView>,
    #[serde(default)]
    pub annotations: Vec<SceneAnnotationView>,
    #[serde(default)]
    pub metrics: Vec<SceneMetricView>,
    #[serde(default)]
    pub legend: Vec<SceneLegendView>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct GridCellView {
    pub x: u32,
    pub y: u32,
    /// Human-readable terrain or cell state. Renderers may keep repetitive
    /// labels (for example, `Floor`) accessible without drawing them in every
    /// cell.
    pub label: String,
    #[serde(default)]
    pub classes: Vec<ClassPath>,
    /// Optional authoritative account represented by this cell. This makes
    /// receipt-derived cell emphasis and account inspection portable without
    /// turning the grid projection into state of its own.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account: Option<ItemPath>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct MatrixCellView {
    pub row: ItemPath,
    pub column: ItemPath,
    pub label: String,
    #[serde(default)]
    pub classes: Vec<ClassPath>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TimelineLaneView {
    pub id: ViewId,
    #[serde(default)]
    pub classes: Vec<ClassPath>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TimelineSpanView {
    pub id: ItemPath,
    pub lane: ItemPath,
    pub start: u64,
    pub end: u64,
    pub label: String,
    #[serde(default)]
    pub classes: Vec<ClassPath>,
}

impl Scene {
    pub fn graph(
        title: impl Into<String>,
        nodes: Vec<GraphNodeView>,
        edges: Vec<GraphEdgeView>,
        focus: Option<ItemPath>,
    ) -> Self {
        let entities = nodes
            .iter()
            .map(|node| SceneEntityView {
                id: node.id.clone(),
                glyph: inferred_glyph(&node.id.label, &node.classes),
                anchor: SceneAnchorView::GraphNode {
                    node: node.id.key.clone(),
                },
                role: SceneEntityRoleView::Structure,
                tone: inferred_tone(&node.classes),
                status: node.classes.first().map(ToString::to_string),
                account: None,
                evidence: Vec::new(),
                metrics: Vec::new(),
            })
            .collect::<Vec<_>>();
        let paths = edges
            .iter()
            .map(|edge| ScenePathView {
                id: edge.id.clone(),
                label: edge.label.clone().unwrap_or_else(|| edge.id.to_string()),
                anchors: vec![
                    SceneAnchorView::GraphNode {
                        node: edge.source.clone(),
                    },
                    SceneAnchorView::GraphNode {
                        node: edge.target.clone(),
                    },
                ],
                status: inferred_path_status(&edge.classes),
            })
            .collect();
        Self {
            title: title.into(),
            surface: SceneSurfaceView::Graph {
                nodes,
                edges,
                focus,
            },
            legend: inferred_legend(&entities),
            entities,
            paths,
            annotations: Vec::new(),
            metrics: Vec::new(),
        }
    }

    pub fn grid(
        title: impl Into<String>,
        width: u32,
        height: u32,
        cells: Vec<GridCellView>,
    ) -> Self {
        // Cells describe stable terrain. Occupants are explicit entities with
        // caller-owned identities so a renderer can track one thing moving
        // between anchors instead of seeing unrelated coordinate-derived IDs.
        let entities = Vec::new();
        Self {
            title: title.into(),
            surface: SceneSurfaceView::Grid {
                width,
                height,
                cells,
            },
            legend: inferred_legend(&entities),
            entities,
            paths: Vec::new(),
            annotations: Vec::new(),
            metrics: Vec::new(),
        }
    }

    pub fn matrix(
        title: impl Into<String>,
        rows: Vec<ViewId>,
        columns: Vec<ViewId>,
        cells: Vec<MatrixCellView>,
    ) -> Self {
        let entities = cells
            .iter()
            .filter(|cell| !cell.label.trim().is_empty() && cell.label != "·")
            .map(|cell| SceneEntityView {
                id: ViewId::new(
                    slug(format!("matrix/{}/{}", cell.row, cell.column)),
                    cell.label.clone(),
                ),
                glyph: SceneGlyphView::Constraint,
                anchor: SceneAnchorView::MatrixCell {
                    row: cell.row.clone(),
                    column: cell.column.clone(),
                },
                role: SceneEntityRoleView::Structure,
                tone: inferred_tone(&cell.classes),
                status: cell.classes.first().map(ToString::to_string),
                account: None,
                evidence: Vec::new(),
                metrics: Vec::new(),
            })
            .collect::<Vec<_>>();
        Self {
            title: title.into(),
            surface: SceneSurfaceView::Matrix {
                rows,
                columns,
                cells,
            },
            legend: inferred_legend(&entities),
            entities,
            paths: Vec::new(),
            annotations: Vec::new(),
            metrics: Vec::new(),
        }
    }

    pub fn timeline(
        title: impl Into<String>,
        lanes: Vec<TimelineLaneView>,
        spans: Vec<TimelineSpanView>,
        cursor: Option<u64>,
    ) -> Self {
        let entities = spans
            .iter()
            .map(|span| SceneEntityView {
                id: ViewId::new(span.id.clone(), &span.label),
                glyph: inferred_glyph(&span.label, &span.classes),
                anchor: SceneAnchorView::Timeline {
                    lane: span.lane.clone(),
                    at: span.start,
                },
                role: SceneEntityRoleView::Structure,
                tone: inferred_tone(&span.classes),
                status: span.classes.first().map(ToString::to_string),
                account: None,
                evidence: Vec::new(),
                metrics: vec![SceneMetricView {
                    key: slug("duration"),
                    label: "Duration".into(),
                    value: span.end.saturating_sub(span.start).to_string(),
                    unit: None,
                    previous: None,
                }],
            })
            .collect::<Vec<_>>();
        Self {
            title: title.into(),
            surface: SceneSurfaceView::Timeline {
                lanes,
                spans,
                cursor,
            },
            legend: inferred_legend(&entities),
            entities,
            paths: Vec::new(),
            annotations: Vec::new(),
            metrics: Vec::new(),
        }
    }

    pub fn with_entities(mut self, entities: impl IntoIterator<Item = SceneEntityView>) -> Self {
        self.entities.extend(entities);
        self.legend = inferred_legend(&self.entities);
        self
    }

    pub fn with_annotations(
        mut self,
        annotations: impl IntoIterator<Item = SceneAnnotationView>,
    ) -> Self {
        self.annotations.extend(annotations);
        self
    }

    pub fn with_metrics(mut self, metrics: impl IntoIterator<Item = SceneMetricView>) -> Self {
        self.metrics.extend(metrics);
        self
    }

    /// Verifies that a disposable projection is internally joinable. This does
    /// not validate the economy—the replay already did that—but it prevents a
    /// broken renderer contract from silently hiding valid economic evidence.
    pub fn validate(&self) -> Result<(), SceneValidationError> {
        let entity_ids = self
            .entities
            .iter()
            .map(|entity| &entity.id.key)
            .collect::<HashSet<_>>();
        if entity_ids.len() != self.entities.len() {
            return Err(SceneValidationError::DuplicateEntity);
        }
        match &self.surface {
            SceneSurfaceView::Graph { nodes, edges, .. } => {
                let nodes = nodes
                    .iter()
                    .map(|node| &node.id.key)
                    .collect::<HashSet<_>>();
                for edge in edges {
                    if !nodes.contains(&edge.source) || !nodes.contains(&edge.target) {
                        return Err(SceneValidationError::UnknownAnchor(edge.id.to_string()));
                    }
                }
            }
            SceneSurfaceView::Grid {
                width,
                height,
                cells,
            } => {
                if cells
                    .iter()
                    .any(|cell| cell.x >= *width || cell.y >= *height)
                {
                    return Err(SceneValidationError::OutOfBounds);
                }
            }
            SceneSurfaceView::Matrix {
                rows,
                columns,
                cells,
            } => {
                let rows = rows.iter().map(|row| &row.key).collect::<HashSet<_>>();
                let columns = columns
                    .iter()
                    .map(|column| &column.key)
                    .collect::<HashSet<_>>();
                if cells
                    .iter()
                    .any(|cell| !rows.contains(&cell.row) || !columns.contains(&cell.column))
                {
                    return Err(SceneValidationError::UnknownAnchor("matrix cell".into()));
                }
            }
            SceneSurfaceView::Timeline { lanes, spans, .. } => {
                let lanes = lanes
                    .iter()
                    .map(|lane| &lane.id.key)
                    .collect::<HashSet<_>>();
                if spans.iter().any(|span| !lanes.contains(&span.lane)) {
                    return Err(SceneValidationError::UnknownAnchor("timeline span".into()));
                }
            }
            SceneSurfaceView::Market { .. } => {}
        }
        for anchor in self
            .entities
            .iter()
            .map(|entity| &entity.anchor)
            .chain(self.annotations.iter().map(|annotation| &annotation.anchor))
            .chain(self.paths.iter().flat_map(|path| path.anchors.iter()))
        {
            self.validate_anchor(anchor)?;
        }
        Ok(())
    }

    /// Verifies that every presentation-only evidence link resolves into the
    /// replayed snapshot that owns this scene.
    pub fn validate_evidence(&self, accounts: &[AccountView]) -> Result<(), SceneValidationError> {
        for reference in self.entities.iter().flat_map(|entity| &entity.evidence) {
            match reference {
                SceneEvidenceRefView::Account { account } => {
                    if !accounts
                        .iter()
                        .any(|candidate| candidate.account.key == *account)
                    {
                        return Err(SceneValidationError::UnknownEvidence(account.to_string()));
                    }
                }
                SceneEvidenceRefView::Balance { account, asset } => {
                    let Some(owner) = accounts
                        .iter()
                        .find(|candidate| candidate.account.key == *account)
                    else {
                        return Err(SceneValidationError::UnknownEvidence(account.to_string()));
                    };
                    if !owner
                        .balances
                        .iter()
                        .any(|balance| balance.asset.key == *asset)
                    {
                        return Err(SceneValidationError::UnknownEvidence(format!(
                            "{asset} in {account}"
                        )));
                    }
                }
            }
        }
        Ok(())
    }

    fn validate_anchor(&self, anchor: &SceneAnchorView) -> Result<(), SceneValidationError> {
        let valid = match (anchor, &self.surface) {
            (SceneAnchorView::Unanchored, _) => true,
            (SceneAnchorView::GraphNode { node }, SceneSurfaceView::Graph { nodes, .. }) => {
                nodes.iter().any(|candidate| candidate.id.key == *node)
            }
            (
                SceneAnchorView::GraphEdge { edge, progress },
                SceneSurfaceView::Graph { edges, .. },
            ) => {
                edges.iter().any(|candidate| candidate.id == *edge)
                    && progress.is_none_or(|value| (0.0..=1.0).contains(&value))
            }
            (SceneAnchorView::GridCell { x, y }, SceneSurfaceView::Grid { width, height, .. }) => {
                x < width && y < height
            }
            (
                SceneAnchorView::MatrixCell { row, column },
                SceneSurfaceView::Matrix { rows, columns, .. },
            ) => {
                rows.iter().any(|candidate| candidate.key == *row)
                    && columns.iter().any(|candidate| candidate.key == *column)
            }
            (SceneAnchorView::Timeline { lane, .. }, SceneSurfaceView::Timeline { lanes, .. }) => {
                lanes.iter().any(|candidate| candidate.id.key == *lane)
            }
            (SceneAnchorView::Entity { entity }, _) => self
                .entities
                .iter()
                .any(|candidate| candidate.id.key == *entity),
            _ => false,
        };
        if valid {
            Ok(())
        } else {
            Err(SceneValidationError::UnknownAnchor(format!("{anchor:?}")))
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SceneValidationError {
    #[error("scene contains duplicate entity ids")]
    DuplicateEntity,
    #[error("scene contains an unknown or incompatible anchor: {0}")]
    UnknownAnchor(String),
    #[error("scene contains an out-of-bounds coordinate")]
    OutOfBounds,
    #[error("scene contains an unknown economic evidence reference: {0}")]
    UnknownEvidence(String),
}

fn inferred_glyph(label: &str, classes: &[ClassPath]) -> SceneGlyphView {
    let words = std::iter::once(label)
        .chain(classes.iter().map(ClassPath::as_str))
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase();
    // Explicit Rust-owned semantic classes win over label heuristics. A room
    // called "Key Room" is still a location; the key itself must be a distinct
    // asset-backed entity rather than an icon inferred from prose.
    if has_class(classes, &["location"]) {
        SceneGlyphView::Location
    } else if has_class(classes, &["facility"])
        && (words.contains("door") || words.contains("gate"))
    {
        SceneGlyphView::Door
    } else if words.contains("vehicle") || words.contains("truck") || words.contains("carrier") {
        SceneGlyphView::Vehicle
    } else if words.contains("robot") {
        SceneGlyphView::Robot
    } else if words.contains("agent") || words.contains("player") || words.contains("scout") {
        SceneGlyphView::Agent
    } else if words.contains("package") || words.contains("order") || words.contains("crate") {
        SceneGlyphView::Package
    } else if words.contains("goal") || words.contains("exit") || words.contains("customer") {
        SceneGlyphView::Goal
    } else if words.contains("key") {
        SceneGlyphView::Key
    } else if words.contains("door") || words.contains("gate") {
        SceneGlyphView::Door
    } else if words.contains("fuel") {
        SceneGlyphView::Fuel
    } else if words.contains("money") || words.contains("buyer") || words.contains("seller") {
        SceneGlyphView::Money
    } else if words.contains("food") || words.contains("fruit") || words.contains("cohort") {
        SceneGlyphView::Food
    } else if words.contains("machine") {
        SceneGlyphView::Machine
    } else if words.contains("task") || words.contains("job") {
        SceneGlyphView::Task
    } else if words.contains("hazard") || words.contains("danger") {
        SceneGlyphView::Hazard
    } else if words.contains("sensor") || words.contains("observe") {
        SceneGlyphView::Sensor
    } else if words.contains("tool") {
        SceneGlyphView::Tool
    } else if words.contains("material") || words.contains("input") {
        SceneGlyphView::Material
    } else if words.contains("time") || words.contains("wait") {
        SceneGlyphView::Clock
    } else if words.contains("energy") {
        SceneGlyphView::Energy
    } else if words.contains("shield") || words.contains("safe") {
        SceneGlyphView::Shield
    } else if words.contains("location") || words.contains("room") || words.contains("depot") {
        SceneGlyphView::Location
    } else {
        SceneGlyphView::Token
    }
}

fn has_class(classes: &[ClassPath], names: &[&str]) -> bool {
    classes.iter().any(|class| names.contains(&class.as_str()))
}

fn inferred_tone(classes: &[ClassPath]) -> SceneToneView {
    if has_class(classes, &["current", "active"]) {
        SceneToneView::Active
    } else if has_class(classes, &["goal"]) {
        SceneToneView::Goal
    } else if has_class(classes, &["success", "selected"]) {
        SceneToneView::Success
    } else if has_class(classes, &["danger", "invalid"]) {
        SceneToneView::Danger
    } else if has_class(classes, &["warning", "blocked"]) {
        SceneToneView::Warning
    } else if has_class(classes, &["uncertain"]) {
        SceneToneView::Uncertain
    } else if has_class(classes, &["muted", "inactive"]) {
        SceneToneView::Muted
    } else {
        SceneToneView::Neutral
    }
}

fn inferred_path_status(classes: &[ClassPath]) -> ScenePathStatusView {
    if has_class(classes, &["current"]) {
        ScenePathStatusView::Current
    } else if has_class(classes, &["selected", "incumbent"]) {
        ScenePathStatusView::Incumbent
    } else if has_class(classes, &["traversed"]) {
        ScenePathStatusView::Traversed
    } else if has_class(classes, &["rejected", "invalid"]) {
        ScenePathStatusView::Rejected
    } else if has_class(classes, &["blocked"]) {
        ScenePathStatusView::Blocked
    } else if has_class(classes, &["uncertain"]) {
        ScenePathStatusView::Uncertain
    } else {
        ScenePathStatusView::Available
    }
}

fn inferred_legend(entities: &[SceneEntityView]) -> Vec<SceneLegendView> {
    let mut entries = Vec::new();
    for entity in entities {
        if !entries
            .iter()
            .any(|known: &SceneLegendView| known.glyph == entity.glyph && known.tone == entity.tone)
        {
            entries.push(SceneLegendView {
                label: entity.id.label.clone(),
                glyph: entity.glyph,
                tone: entity.tone,
            });
        }
    }
    entries
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ViewSnapshot {
    pub index: u64,
    pub accounts: Vec<AccountView>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scene: Option<Scene>,
    /// Disposable rankings derived from this exact replay-verified state.
    /// They explain comparative outcomes; they never authorize an exchange.
    #[serde(default)]
    pub leaderboards: Vec<LeaderboardView>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LeaderboardView {
    pub key: MeasurePath,
    pub label: String,
    pub description: String,
    pub direction: ObjectiveDirectionView,
    #[serde(default)]
    pub entries: Vec<LeaderboardEntryView>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LeaderboardEntryView {
    /// Eligible entries use one-based competition ranking. Ineligible entries
    /// remain visible but deliberately have no rank.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rank: Option<u64>,
    pub participant: ViewId,
    /// Exact display text. Ratios should be reduced before projection rather
    /// than rounded into semantic state.
    pub value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    pub eligible: bool,
    #[serde(default)]
    pub components: Vec<SceneMetricView>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FrameCueKindView {
    AtomicExchange,
    Consumed,
    Produced,
    Preserved,
    Movement,
    Information,
    Chance,
    Completion,
}

/// A readable explanation derived from the applied exchange and its receipt.
/// Cues make economically meaningful transitions visible even when a domain
/// projection chooses identical geometry before and after the exchange.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct FrameCueView {
    pub kind: FrameCueKindView,
    pub label: String,
    #[serde(default)]
    pub details: Vec<String>,
    #[serde(default)]
    pub subjects: Vec<ViewId>,
}

/// One applied exchange and the state it led to. The state before it is the
/// previous frame's `after`, or the document's `initial` for the first frame,
/// so a replay holds each state once.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ExchangeFrame {
    pub index: u64,
    pub exchange: ExchangeView,
    pub assessment: AssessmentView,
    pub receipt: ReceiptView,
    pub after: ViewSnapshot,
    #[serde(default)]
    pub cues: Vec<FrameCueView>,
    #[serde(default)]
    pub observations: Vec<ObservationView>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ObjectiveView {
    pub key: MeasurePath,
    pub label: String,
    pub direction: ObjectiveDirectionView,
    pub value: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ObjectiveDirectionView {
    Minimize,
    Maximize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FrontierCompletenessView {
    Exact,
    Approximate,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ObjectiveAxisView {
    pub key: MeasurePath,
    pub label: String,
    pub direction: ObjectiveDirectionView,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ParetoPointView {
    pub label: String,
    /// Ordered exactly like `ParetoFrontView.axes`; values remain exact text.
    pub values: Vec<String>,
    pub selected: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ParetoFrontView {
    pub title: String,
    pub completeness: FrontierCompletenessView,
    pub axes: Vec<ObjectiveAxisView>,
    pub points: Vec<ParetoPointView>,
}

/// A proposal assessed against an exact snapshot, including rejected actions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ProposalView {
    pub id: ProposalPath,
    pub label: String,
    pub description: String,
    pub snapshot_index: u64,
    pub exchange: ExchangeView,
    pub assessment: AssessmentView,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TelemetryKindView {
    Expanded,
    Generated,
    Iteration,
    Sample,
    InformationSet,
    Accounts,
    Rates,
    Transitions,
    Constraints,
    Alternatives,
    Message,
}

/// A transport-neutral progress observation. Counters are exact text so the
/// same contract remains safe in native and JavaScript consumers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TelemetryPointView {
    pub sequence: u64,
    pub kind: TelemetryKindView,
    pub value: String,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SearchTelemetryView {
    pub algorithm: String,
    pub exact: bool,
    #[serde(default)]
    pub points: Vec<TelemetryPointView>,
}

/// An actor-relative observation. Omitted accounts and assets are intentionally
/// not visible to that actor; this is not a filtered copy of authoritative
/// state pretending to be complete.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ObservationView {
    pub actor: ViewId,
    pub label: String,
    pub visible_accounts: Vec<AccountView>,
    #[serde(default)]
    pub facts: Vec<AssetQuantityView>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SearchObservationKindView {
    Phase,
    Frontier,
    Rollout,
    Tree,
    Belief,
    Candidate,
    Incumbent,
    Prune,
    Artifact,
}

/// A bounded, transport-neutral view of solver work. Search crates expose
/// typed session state; the service maps it into this presentation contract so
/// HTTP, WASM, saved studies, and Studio use identical evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SearchObservationView {
    pub sequence: u64,
    pub phase: PhasePath,
    pub algorithm: AlgorithmPath,
    pub kind: SearchObservationKindView,
    pub label: String,
    pub completed: u64,
    pub total: u64,
    /// Stable Rust-owned scene subjects affected by this observation. Generic
    /// viewers may emphasize matching nodes or entities without understanding
    /// the problem's domain model.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub subjects: Vec<ViewId>,
    #[serde(default)]
    pub metrics: Vec<SceneMetricView>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ViewDocument {
    pub id: DocumentPath,
    pub title: String,
    pub description: String,
    pub source: ViewSource,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<ModelView>,
    /// How to read quantities of each asset whose format is not a count, by
    /// the asset's key.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub formats: BTreeMap<ItemPath, QuantityFormat>,
    pub initial: ViewSnapshot,
    pub frames: Vec<ExchangeFrame>,
    #[serde(default)]
    pub objectives: Vec<ObjectiveView>,
    #[serde(default)]
    pub pareto_fronts: Vec<ParetoFrontView>,
    #[serde(default)]
    pub proposals: Vec<ProposalView>,
    #[serde(default)]
    pub telemetry: Vec<SearchTelemetryView>,
    #[serde(default)]
    pub observations: Vec<ObservationView>,
    /// Bounded solver history retained so a fast native study or a static Pages
    /// study is as inspectable as a live stream.
    #[serde(default)]
    pub solve_observations: Vec<SearchObservationView>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ViewSource {
    pub key: ItemPath,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ViewDocumentMetadata {
    pub id: DocumentPath,
    pub title: String,
    pub description: String,
    pub source: ViewSource,
}

/// How views show an ontology: the stable key and label of each of its
/// accounts, assets, rates and roles, and optionally a scene and leaderboards
/// drawn from each replayed state. Keys are paths under the ontology's name,
/// so a value has the same key in every document of its ontology and never
/// the key of another ontology's value.
pub trait ViewOntology<O: Ontology> {
    fn account(&self, id: &O::Account) -> ViewId;
    fn asset(&self, id: &O::Asset) -> ViewId;
    fn rate(&self, id: &O::Rate) -> ViewId;
    fn role(&self, id: &O::Role) -> ViewId;

    /// How people read quantities of `asset`; a count unless it says
    /// otherwise.
    fn format(&self, _asset: &O::Asset) -> QuantityFormat {
        QuantityFormat::COUNT
    }

    fn scene(&self, _index: u64, _economy: &Economy<O>) -> Option<Scene> {
        None
    }

    fn leaderboards(&self, _index: u64, _economy: &Economy<O>) -> Vec<LeaderboardView> {
        Vec::new()
    }
}

/// Shows an ontology's values by their `Debug` output, keyed under the
/// ontology's name: the account `Agent` of the ontology `maze` is keyed
/// `maze/account/agent` and labeled `Agent`. The engine keeps the exact typed
/// values; only their presentation is derived.
pub struct DebugOntology<O>(PhantomData<fn() -> O>);

impl<O: Ontology> Default for DebugOntology<O> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

impl<O: Ontology> DebugOntology<O> {
    fn id<T: Debug>(&self, category: &str, value: &T) -> ViewId {
        let debug = format!("{value:?}");
        // A value whose `Debug` output has no letter or digit, such as `()`,
        // still needs a segment.
        let value = slug_segment(&debug).unwrap_or_else(|| "value".into());
        ViewId::new(
            slug(format!("{}/{category}/{value}", OntologyPath::of::<O>())),
            humanize_debug(&debug),
        )
    }
}

impl<O: Ontology> ViewOntology<O> for DebugOntology<O> {
    fn account(&self, id: &O::Account) -> ViewId {
        self.id("account", id)
    }

    fn asset(&self, id: &O::Asset) -> ViewId {
        self.id("asset", id)
    }

    fn rate(&self, id: &O::Rate) -> ViewId {
        self.id("rate", id)
    }

    fn role(&self, id: &O::Role) -> ViewId {
        self.id("role", id)
    }
}

fn humanize_debug(debug: &str) -> String {
    let mut output = String::with_capacity(debug.len() + 4);
    let mut previous_lowercase = false;
    for character in debug.chars() {
        if matches!(character, '(' | ')' | '{' | '}' | '[' | ']') {
            output.push(' ');
            previous_lowercase = false;
        } else if character == ',' {
            output.push_str(", ");
            previous_lowercase = false;
        } else if character == '_' {
            output.push(' ');
            previous_lowercase = false;
        } else {
            if character.is_ascii_uppercase() && previous_lowercase {
                output.push(' ');
            }
            output.push(character);
            previous_lowercase = character.is_ascii_lowercase() || character.is_ascii_digit();
        }
    }
    output.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PlaybackError {
    #[error("trace exchange {index} failed replay: {message}")]
    Replay { index: u64, message: String },
}

/// Replays a trace and materializes a portable, self-contained view document.
pub fn derive_document<O: Ontology, V>(
    metadata: ViewDocumentMetadata,
    initial: &Economy<O>,
    trace: &Trace<O>,
    ontology: &V,
    objectives: Vec<ObjectiveView>,
) -> Result<ViewDocument, PlaybackError>
where
    V: ViewOntology<O>,
{
    derive_document_with_frames(metadata, initial, trace, ontology, objectives, |_| {})
}

/// Replays a trace and reports each verified frame as soon as it is derived.
/// The callback is transport-neutral and cannot influence economic state.
pub fn derive_document_with_frames<O: Ontology, V, F>(
    metadata: ViewDocumentMetadata,
    initial: &Economy<O>,
    trace: &Trace<O>,
    ontology: &V,
    objectives: Vec<ObjectiveView>,
    mut on_frame: F,
) -> Result<ViewDocument, PlaybackError>
where
    V: ViewOntology<O>,
    F: FnMut(&ExchangeFrame),
{
    let formats = formats(initial, ontology);
    let mut economy = initial.clone();
    let initial = snapshot(0, &economy, ontology);
    let mut frames = Vec::with_capacity(trace.exchanges().len());

    for (offset, exchange) in trace.exchanges().iter().enumerate() {
        let index = offset as u64;
        let assessment = assessment_view(&economy.assess(exchange), ontology);
        let receipt = economy
            .apply(exchange.clone())
            .map_err(|error| PlaybackError::Replay {
                index,
                message: error.to_string(),
            })?;
        let after = snapshot(index + 1, &economy, ontology);
        let exchange = exchange_view(exchange, ontology);
        let receipt = receipt_view(&receipt, ontology);
        let cues = frame_cues(&exchange, &receipt);
        let frame = ExchangeFrame {
            index,
            exchange,
            assessment,
            receipt,
            after,
            cues,
            observations: Vec::new(),
        };
        on_frame(&frame);
        frames.push(frame);
    }

    Ok(ViewDocument {
        id: metadata.id,
        title: metadata.title,
        description: metadata.description,
        source: metadata.source,
        model: None,
        formats,
        initial,
        frames,
        objectives,
        pareto_fronts: Vec::new(),
        proposals: Vec::new(),
        telemetry: Vec::new(),
        observations: Vec::new(),
        solve_observations: Vec::new(),
    })
}

/// The format of every asset the economy names, its balances, rates and
/// invariants, whose format is not a count.
fn formats<O: Ontology, V: ViewOntology<O>>(
    economy: &Economy<O>,
    ontology: &V,
) -> BTreeMap<ItemPath, QuantityFormat> {
    economy
        .asset_keys()
        .into_iter()
        .map(|asset| (ontology.asset(asset).key, ontology.format(asset)))
        .filter(|(_, format)| *format != QuantityFormat::COUNT)
        .collect()
}

fn frame_cues(exchange: &ExchangeView, receipt: &ReceiptView) -> Vec<FrameCueView> {
    let mut cues = vec![FrameCueView {
        kind: FrameCueKindView::AtomicExchange,
        label: exchange.rate.label.clone(),
        details: vec![format!(
            "applied atomically · {} roles bound · {} accounts changed",
            exchange.bindings.len(),
            receipt.deltas.len()
        )],
        subjects: std::iter::once(exchange.rate.clone())
            .chain(
                exchange
                    .bindings
                    .iter()
                    .flat_map(|binding| [binding.role.clone(), binding.account.clone()]),
            )
            .collect(),
    }];
    for delta in &receipt.deltas {
        for (kind, verb, assets) in [
            (FrameCueKindView::Consumed, "consumed", &delta.consumed),
            (FrameCueKindView::Produced, "produced", &delta.produced),
            (FrameCueKindView::Preserved, "preserved", &delta.preserved),
        ] {
            if assets.is_empty() {
                continue;
            }
            cues.push(FrameCueView {
                kind,
                label: format!(
                    "{} {verb} {} {}",
                    delta.account.label,
                    assets.len(),
                    if assets.len() == 1 { "asset" } else { "assets" }
                ),
                details: assets
                    .iter()
                    .map(|asset| format!("{} {}", asset.quantity.0, asset.asset.label))
                    .collect(),
                subjects: std::iter::once(delta.account.clone())
                    .chain(assets.iter().map(|asset| asset.asset.clone()))
                    .collect(),
            });
        }
    }
    cues
}

/// Derives the immutable model definition used by a view document.
pub fn derive_model<O: Ontology, V>(economy: &Economy<O>, goal: &Goal<O>, ontology: &V) -> ModelView
where
    V: ViewOntology<O>,
{
    let mut rates = economy
        .rate_ids()
        .filter_map(|id| {
            let rate = economy.rate(id)?;
            let roles = rate
                .roles()
                .map(|role| RateRoleView {
                    role: ontology.role(role),
                    consumed: rate
                        .consumed(role)
                        .map_or_else(Vec::new, |basket| basket_view(basket, ontology)),
                    produced: rate
                        .produced(role)
                        .map_or_else(Vec::new, |basket| basket_view(basket, ontology)),
                    preserved: rate
                        .preserved(role)
                        .map_or_else(Vec::new, |basket| basket_view(basket, ontology)),
                })
                .collect();
            let distinct_roles = rate
                .distinct_roles()
                .map(|(left, right)| [ontology.role(left), ontology.role(right)])
                .collect();
            let computed_amounts = |amounts: &[axionomy::ComputedAmount<O>]| {
                amounts
                    .iter()
                    .map(|amount| ComputedRateAmountView {
                        role: ontology.role(amount.role()),
                        asset: ontology.asset(amount.asset()),
                        quantity: expression_view(amount.quantity(), ontology),
                    })
                    .collect::<Vec<_>>()
            };
            Some(RateView {
                rate: ontology.rate(id),
                roles,
                distinct_roles,
                computed_consumed: computed_amounts(rate.computed_consumed()),
                computed_produced: computed_amounts(rate.computed_produced()),
                computed_preserved: computed_amounts(rate.computed_preserved()),
                conditions: rate
                    .conditions()
                    .iter()
                    .map(|condition| RateConditionView {
                        name: condition.name().clone(),
                        left: expression_view(condition.left(), ontology),
                        comparison: match condition.comparison() {
                            QuantityComparison::Equal => "=".into(),
                            QuantityComparison::GreaterThanOrEqual => "≥".into(),
                            QuantityComparison::LessThanOrEqual => "≤".into(),
                        },
                        right: expression_view(condition.right(), ontology),
                    })
                    .collect(),
            })
        })
        .collect::<Vec<_>>();
    rates.sort_by(|left, right| left.rate.key.cmp(&right.rate.key));

    let goal = goal
        .requirements()
        .iter()
        .map(|(account, required)| GoalRequirementView {
            account: ontology.account(account),
            required: basket_view(required, ontology),
        })
        .collect();

    let invariants = economy
        .invariants()
        .map(|invariant| {
            let mut terms = invariant
                .weights()
                .map(|(asset, weight)| InvariantTermView {
                    asset: ontology.asset(asset),
                    weight,
                })
                .collect::<Vec<_>>();
            terms.sort_by(|left, right| left.asset.key.cmp(&right.asset.key));
            InvariantView {
                name: invariant.name().clone(),
                terms,
            }
        })
        .collect();

    ModelView {
        rates,
        goal,
        invariants,
    }
}

fn expression_view<O: Ontology, V>(expression: &QuantityExpression<O>, ontology: &V) -> String
where
    V: ViewOntology<O>,
{
    match expression {
        QuantityExpression::Constant { value } => value.to_string(),
        QuantityExpression::Units => "exchange units".into(),
        QuantityExpression::Parameter { name } => format!("${name}"),
        QuantityExpression::Balance { role, asset } => {
            format!(
                "{}.{}",
                ontology.role(role).label,
                ontology.asset(asset).label
            )
        }
        QuantityExpression::Add { left, right } => format!(
            "({} + {})",
            expression_view(left, ontology),
            expression_view(right, ontology)
        ),
        QuantityExpression::Subtract { left, right } => format!(
            "({} − {})",
            expression_view(left, ontology),
            expression_view(right, ontology)
        ),
        QuantityExpression::Multiply { left, right } => format!(
            "({} × {})",
            expression_view(left, ontology),
            expression_view(right, ontology)
        ),
        QuantityExpression::Minimum { left, right } => format!(
            "min({}, {})",
            expression_view(left, ontology),
            expression_view(right, ontology)
        ),
        QuantityExpression::Maximum { left, right } => format!(
            "max({}, {})",
            expression_view(left, ontology),
            expression_view(right, ontology)
        ),
        QuantityExpression::MulDiv {
            left,
            right,
            denominator,
            rounding,
        } => {
            let direction = match rounding {
                Rounding::Floor => "floor",
                Rounding::Ceil => "ceil",
            };
            let numerator = match right.as_ref() {
                // Plain division is a mul-div by one; render it as division.
                QuantityExpression::Constant { value } if *value == Quantity::new(1) => {
                    expression_view(left, ontology)
                }
                _ => format!(
                    "{} × {}",
                    expression_view(left, ontology),
                    expression_view(right, ontology)
                ),
            };
            format!(
                "{direction}({numerator} ÷ {})",
                expression_view(denominator, ontology)
            )
        }
    }
}

/// Projects a snapshot without mutating or replaying it.
pub fn derive_snapshot<O: Ontology, V>(
    index: u64,
    economy: &Economy<O>,
    ontology: &V,
) -> ViewSnapshot
where
    V: ViewOntology<O>,
{
    snapshot(index, economy, ontology)
}

/// Projects an exchange and its non-mutating feasibility assessment.
pub fn derive_proposal<O: Ontology, V>(
    id: ProposalPath,
    label: impl Into<String>,
    description: impl Into<String>,
    snapshot_index: u64,
    economy: &Economy<O>,
    exchange: &Exchange<O>,
    ontology: &V,
) -> ProposalView
where
    V: ViewOntology<O>,
{
    ProposalView {
        id,
        label: label.into(),
        description: description.into(),
        snapshot_index,
        exchange: exchange_view(exchange, ontology),
        assessment: assessment_view(&economy.assess(exchange), ontology),
    }
}

fn snapshot<O: Ontology, V>(index: u64, economy: &Economy<O>, ontology: &V) -> ViewSnapshot
where
    V: ViewOntology<O>,
{
    let mut accounts = economy
        .accounts()
        .map(|(id, account)| AccountView {
            account: ontology.account(id),
            balances: basket_view(account.balances(), ontology),
        })
        .collect::<Vec<_>>();
    accounts.sort_by(|left, right| left.account.key.cmp(&right.account.key));
    let balance_entries = accounts
        .iter()
        .map(|account| account.balances.len())
        .sum::<usize>();
    let scene = ontology.scene(index, economy).map(|mut scene| {
        if !matches!(scene.surface, SceneSurfaceView::Market { .. }) {
            scene.metrics.extend([
                SceneMetricView {
                    key: slug("snapshot"),
                    label: "Economic step".into(),
                    value: index.to_string(),
                    unit: Some("exchanges".into()),
                    previous: index.checked_sub(1).map(|value| value.to_string()),
                },
                SceneMetricView {
                    key: slug("accounts"),
                    label: "Accounts".into(),
                    value: accounts.len().to_string(),
                    unit: None,
                    previous: None,
                },
                SceneMetricView {
                    key: slug("balances"),
                    label: "Non-zero balances".into(),
                    value: balance_entries.to_string(),
                    unit: None,
                    previous: None,
                },
            ]);
        }
        scene
    });

    ViewSnapshot {
        index,
        accounts,
        scene,
        leaderboards: ontology.leaderboards(index, economy),
    }
}

fn basket_view<O: Ontology, V>(basket: &Basket<O::Asset>, ontology: &V) -> Vec<AssetQuantityView>
where
    V: ViewOntology<O>,
{
    let mut entries = basket
        .iter()
        .map(|(asset, quantity)| AssetQuantityView {
            asset: ontology.asset(asset),
            quantity: quantity.into(),
        })
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| left.asset.key.cmp(&right.asset.key));
    entries
}

fn exchange_view<O: Ontology, V>(exchange: &Exchange<O>, ontology: &V) -> ExchangeView
where
    V: ViewOntology<O>,
{
    ExchangeView {
        rate: ontology.rate(exchange.rate()),
        units: exchange.units().into(),
        bindings: exchange
            .bindings()
            .iter()
            .map(|(role, account)| RoleBindingView {
                role: ontology.role(role),
                account: ontology.account(account),
            })
            .collect(),
        parameters: exchange
            .parameters()
            .iter()
            .map(|(name, value)| ExchangeParameterView {
                name: name.clone(),
                value: value.into(),
            })
            .collect(),
    }
}

fn delta_view<O: Ontology, V>(delta: &AccountDelta<O>, ontology: &V) -> AccountDeltaView
where
    V: ViewOntology<O>,
{
    AccountDeltaView {
        account: ontology.account(delta.account()),
        consumed: basket_view(delta.consumed(), ontology),
        produced: basket_view(delta.produced(), ontology),
        preserved: basket_view(delta.preserved(), ontology),
    }
}

fn receipt_view<O: Ontology, V>(receipt: &Receipt<O>, ontology: &V) -> ReceiptView
where
    V: ViewOntology<O>,
{
    ReceiptView {
        deltas: receipt
            .deltas()
            .iter()
            .map(|delta| delta_view(delta, ontology))
            .collect(),
    }
}

fn account_assessment_view<O: Ontology, V>(
    assessment: &AccountAssessment<O>,
    ontology: &V,
) -> AccountAssessmentView
where
    V: ViewOntology<O>,
{
    AccountAssessmentView {
        account: ontology.account(assessment.account()),
        available: basket_view(assessment.available(), ontology),
        required: basket_view(assessment.required(), ontology),
        consumed: basket_view(assessment.consumed(), ontology),
        produced: basket_view(assessment.produced(), ontology),
        preserved: basket_view(assessment.preserved(), ontology),
    }
}

fn assessment_view<O: Ontology, V>(
    assessment: &ExchangeAssessment<O>,
    ontology: &V,
) -> AssessmentView
where
    V: ViewOntology<O>,
{
    match assessment {
        ExchangeAssessment::Applicable {
            accounts,
            projected_deltas,
        } => AssessmentView {
            status: AssessmentStatusView::Applicable,
            accounts: accounts
                .iter()
                .map(|account| account_assessment_view(account, ontology))
                .collect(),
            projected_deltas: projected_deltas
                .iter()
                .map(|delta| delta_view(delta, ontology))
                .collect(),
            shortfalls: Vec::new(),
            issues: Vec::new(),
        },
        ExchangeAssessment::Infeasible {
            accounts,
            shortfalls,
        } => AssessmentView {
            status: AssessmentStatusView::Infeasible,
            accounts: accounts
                .iter()
                .map(|account| account_assessment_view(account, ontology))
                .collect(),
            projected_deltas: Vec::new(),
            shortfalls: shortfalls
                .iter()
                .map(|shortfall| AccountShortfallView {
                    account: ontology.account(shortfall.account()),
                    missing: basket_view(shortfall.missing(), ontology),
                })
                .collect(),
            issues: Vec::new(),
        },
        ExchangeAssessment::Invalid { issues } => AssessmentView {
            status: AssessmentStatusView::Invalid,
            accounts: Vec::new(),
            projected_deltas: Vec::new(),
            shortfalls: Vec::new(),
            issues: issues
                .iter()
                .map(|issue| assessment_issue_view(issue, ontology))
                .collect(),
        },
    }
}

fn assessment_issue_view<O: Ontology, V>(issue: &ApplyError<O>, ontology: &V) -> AssessmentIssueView
where
    V: ViewOntology<O>,
{
    let (kind, message, subjects) = match issue {
        ApplyError::MissingRate { rate } => {
            let rate = ontology.rate(rate);
            (
                AssessmentIssueKindView::MissingRate,
                format!("Rate `{}` does not exist.", rate.label),
                vec![rate],
            )
        }
        ApplyError::MissingBinding { role } => {
            let role = ontology.role(role);
            (
                AssessmentIssueKindView::MissingBinding,
                format!("Missing account binding for role `{}`.", role.label),
                vec![role],
            )
        }
        ApplyError::UnknownBinding { role } => {
            let role = ontology.role(role);
            (
                AssessmentIssueKindView::UnknownBinding,
                format!("Role `{}` is not declared by this rate.", role.label),
                vec![role],
            )
        }
        ApplyError::RolesMustDiffer { left, right } => {
            let left = ontology.role(left);
            let right = ontology.role(right);
            (
                AssessmentIssueKindView::RolesMustDiffer,
                format!(
                    "Roles `{}` and `{}` must bind to different accounts.",
                    left.label, right.label
                ),
                vec![left, right],
            )
        }
        ApplyError::MissingAccount { account } => {
            let account = ontology.account(account);
            (
                AssessmentIssueKindView::MissingAccount,
                format!("Bound account `{}` does not exist.", account.label),
                vec![account],
            )
        }
        ApplyError::ZeroUnits => (
            AssessmentIssueKindView::ZeroUnits,
            "Exchange units must be greater than zero.".into(),
            Vec::new(),
        ),
        ApplyError::MissingParameter { name } => (
            AssessmentIssueKindView::MissingParameter,
            format!("Missing required exchange parameter `{name}`."),
            Vec::new(),
        ),
        ApplyError::UnknownParameter { name } => (
            AssessmentIssueKindView::UnknownParameter,
            format!("Parameter `{name}` is not declared by this rate."),
            Vec::new(),
        ),
        ApplyError::RateOverflow { rate, asset } => {
            let rate = ontology.rate(rate);
            let asset = ontology.asset(asset);
            (
                AssessmentIssueKindView::RateOverflow,
                format!(
                    "Scaling rate `{}` overflowed asset `{}`.",
                    rate.label, asset.label
                ),
                vec![rate, asset],
            )
        }
        ApplyError::RateLawEvaluation { rate, message } => {
            let rate = ontology.rate(rate);
            (
                AssessmentIssueKindView::RateLawEvaluation,
                format!(
                    "Rate `{}` could not evaluate its exchange law: {message}.",
                    rate.label
                ),
                vec![rate],
            )
        }
        ApplyError::RateConditionFailed { rate, condition } => {
            let rate = ontology.rate(rate);
            (
                AssessmentIssueKindView::RateConditionFailed,
                format!("Rate `{}` refused condition `{condition}`.", rate.label),
                vec![rate],
            )
        }
        ApplyError::Infeasible { shortfalls } => {
            let accounts = shortfalls
                .iter()
                .map(|shortfall| ontology.account(shortfall.account()))
                .collect::<Vec<_>>();
            (
                AssessmentIssueKindView::Infeasible,
                "Exchange requirements are not currently feasible.".into(),
                accounts,
            )
        }
        ApplyError::BalanceOverflow { account, asset } => {
            let account = ontology.account(account);
            let asset = ontology.asset(asset);
            (
                AssessmentIssueKindView::BalanceOverflow,
                format!(
                    "Applying the exchange would overflow `{}` in account `{}`.",
                    asset.label, account.label
                ),
                vec![account, asset],
            )
        }
        ApplyError::InvariantViolation { invariant, .. } => (
            AssessmentIssueKindView::InvariantViolation,
            format!("Declared invariant `{invariant}` would be violated."),
            Vec::new(),
        ),
        ApplyError::AccountExists { account } => {
            let account = ontology.account(account);
            (
                AssessmentIssueKindView::AccountExists,
                format!(
                    "Account `{}` already exists, so the exchange cannot open it.",
                    account.label
                ),
                vec![account],
            )
        }
        ApplyError::OpeningShared { role } => {
            let role = ontology.role(role);
            (
                AssessmentIssueKindView::OpeningShared,
                format!(
                    "Another role binds the account that role `{}` opens.",
                    role.label
                ),
                vec![role],
            )
        }
        ApplyError::ClosedNotEmpty { account, .. } => {
            let account = ontology.account(account);
            (
                AssessmentIssueKindView::ClosedNotEmpty,
                format!(
                    "The exchange leaves assets in account `{}`, which it closes.",
                    account.label
                ),
                vec![account],
            )
        }
    };
    AssessmentIssueView {
        kind,
        message,
        subjects,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axionomy::{Account, EconomyBuilder, Exchange, Quantity, Rate, basket};
    use serde_json::json;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
    enum AccountId {
        Agent,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
    enum Asset {
        Ready,
        Done,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
    enum RateId {
        Finish,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
    enum Role {
        Actor,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
    enum Test {}

    impl Ontology for Test {
        const NAME: &'static str = "test";
        type Account = AccountId;
        type Asset = Asset;
        type Rate = RateId;
        type Role = Role;
    }

    struct Labels;

    impl ViewOntology<Test> for Labels {
        fn account(&self, _: &AccountId) -> ViewId {
            ViewId::new(slug("account/agent"), "Agent")
        }

        fn asset(&self, asset: &Asset) -> ViewId {
            match asset {
                Asset::Ready => ViewId::new(slug("asset/ready"), "Ready"),
                Asset::Done => ViewId::new(slug("asset/done"), "Done"),
            }
        }

        fn rate(&self, _: &RateId) -> ViewId {
            ViewId::new(slug("rate/finish"), "Finish")
        }

        fn role(&self, _: &Role) -> ViewId {
            ViewId::new(slug("role/actor"), "Actor")
        }
    }

    #[test]
    fn formats_write_exact_values_and_never_round() {
        let write = |format: &QuantityFormat, atoms: u64| format.write(Quantity::new(atoms));
        assert_eq!(write(&QuantityFormat::COUNT, 0), "0");
        assert_eq!(write(&QuantityFormat::COUNT, 1_250), "1250");
        let usd = QuantityFormat::decimal(2).digits(2).unit("USD");
        assert_eq!(write(&usd, 1_250), "12.50 USD");
        assert_eq!(write(&usd, 5), "0.05 USD");
        assert_eq!(write(&usd, 0), "0.00 USD");
        let kilograms = QuantityFormat::decimal(3).unit("kg");
        assert_eq!(write(&kilograms, 1_000), "1 kg");
        assert_eq!(write(&kilograms, 900), "0.9 kg");
        assert_eq!(write(&kilograms, 1_234_567), "1234.567 kg");
        // A WAD-scaled share read as a percentage: 10^16 atoms per percent.
        let percent = QuantityFormat::decimal(16).unit("%");
        assert_eq!(write(&percent, 50_000_000_000_000_000), "5 %");
        assert_eq!(write(&percent, 1), "0.0000000000000001 %");
        assert_eq!(
            QuantityFormat::decimal(18).write(Quantity::MAX),
            format!(
                "{}.{}",
                &Quantity::MAX.to_string()[..Quantity::MAX.to_string().len() - 18],
                Quantity::MAX.to_string()[Quantity::MAX.to_string().len() - 18..]
                    .trim_end_matches('0')
            )
        );
        // More digits than the scale pad with zeros; they never invent precision.
        assert_eq!(write(&QuantityFormat::COUNT.digits(2), 7), "7.00");
    }

    #[test]
    fn grid_cells_are_terrain_not_inferred_moving_entities() {
        let scene = Scene::grid(
            "Warehouse",
            2,
            1,
            vec![
                GridCellView {
                    x: 0,
                    y: 0,
                    label: "Floor with player".into(),
                    classes: vec![slug("floor")],
                    account: Some(slug("cell/0")),
                },
                GridCellView {
                    x: 1,
                    y: 0,
                    label: "Floor".into(),
                    classes: vec![slug("floor")],
                    account: Some(slug("cell/1")),
                },
            ],
        );

        assert!(scene.entities.is_empty());
        assert!(matches!(scene.surface, SceneSurfaceView::Grid { .. }));
    }

    #[test]
    fn replayed_frames_expose_exact_balances_and_receipt_deltas() {
        let economy = EconomyBuilder::<Test>::new()
            .account(AccountId::Agent, Account::from(basket([(Asset::Ready, 1)])))
            .rate(
                RateId::Finish,
                Rate::new()
                    .consume(Role::Actor, basket([(Asset::Ready, 1)]))
                    .produce(Role::Actor, basket([(Asset::Done, u64::MAX)])),
            )
            .build()
            .unwrap();
        let exchange =
            Exchange::new(RateId::Finish, Quantity::new(1)).bind(Role::Actor, AccountId::Agent);
        let mut trace = Trace::new();
        trace.push(exchange);

        let document = derive_document(
            ViewDocumentMetadata {
                id: slug("test"),
                title: "Test".into(),
                description: "A test trace".into(),
                source: ViewSource {
                    key: slug("test"),
                    label: "Test".into(),
                },
            },
            &economy,
            &trace,
            &Labels,
            Vec::new(),
        )
        .unwrap();

        assert_eq!(document.frames.len(), 1);
        assert_eq!(
            document.frames[0].after.accounts[0].balances[0].quantity.0,
            u64::MAX.to_string()
        );
        assert_eq!(
            serde_json::to_value(document.frames[0].assessment.status).unwrap(),
            json!("applicable")
        );
        assert_eq!(
            document.frames[0].assessment.projected_deltas,
            document.frames[0].receipt.deltas
        );
    }

    #[test]
    fn invalid_proposals_preserve_the_missing_role_identity() {
        let economy = EconomyBuilder::<Test>::new()
            .account(AccountId::Agent, Account::from(basket([(Asset::Ready, 1)])))
            .rate(
                RateId::Finish,
                Rate::new().consume(Role::Actor, basket([(Asset::Ready, 1)])),
            )
            .build()
            .unwrap();
        let proposal = derive_proposal(
            slug("missing-actor"),
            "Missing actor",
            "An intentionally unbound exchange",
            0,
            &economy,
            &Exchange::new(RateId::Finish, Quantity::new(1)),
            &Labels,
        );

        assert_eq!(proposal.assessment.issues.len(), 1);
        assert_eq!(
            proposal.assessment.issues[0].kind,
            AssessmentIssueKindView::MissingBinding
        );
        assert_eq!(
            proposal.assessment.issues[0].message,
            "Missing account binding for role `Actor`."
        );
        assert_eq!(
            proposal.assessment.issues[0].subjects[0].key.as_str(),
            "role/actor"
        );
    }

    #[test]
    fn scene_evidence_must_resolve_to_an_exact_snapshot_balance() {
        let accounts = vec![AccountView {
            account: ViewId::new(slug("account/agent"), "Agent"),
            balances: vec![AssetQuantityView {
                asset: ViewId::new(slug("asset/ready"), "Ready"),
                quantity: ExactQuantity("1".into()),
            }],
        }];
        let mut scene = Scene::graph(
            "Evidence",
            vec![GraphNodeView {
                id: ViewId::new(slug("node/agent"), "Agent"),
                classes: Vec::new(),
                x: Some(0.0),
                y: Some(0.0),
            }],
            Vec::new(),
            None,
        );
        scene.entities[0].evidence = vec![SceneEvidenceRefView::Balance {
            account: slug("account/agent"),
            asset: slug("asset/ready"),
        }];
        assert_eq!(scene.validate_evidence(&accounts), Ok(()));

        scene.entities[0].evidence = vec![SceneEvidenceRefView::Balance {
            account: slug("account/agent"),
            asset: slug("asset/missing"),
        }];
        assert_eq!(
            scene.validate_evidence(&accounts),
            Err(SceneValidationError::UnknownEvidence(
                "asset/missing in account/agent".into()
            ))
        );
    }

    #[test]
    fn scene_entity_roles_are_explicit_portable_presentation_metadata() {
        let scene = Scene::graph(
            "Roles",
            vec![GraphNodeView {
                id: ViewId::new(slug("node/depot"), "Depot"),
                classes: Vec::new(),
                x: Some(0.0),
                y: Some(0.0),
            }],
            Vec::new(),
            None,
        );

        assert_eq!(scene.entities[0].role, SceneEntityRoleView::Structure);
        assert_eq!(
            serde_json::to_value(scene.entities[0].role).unwrap(),
            json!("structure")
        );
    }

    #[test]
    fn view_document_has_a_json_schema() {
        let schema = schemars::schema_for!(ViewDocument);
        let value = serde_json::to_value(schema).unwrap();
        assert_eq!(value["title"], "ViewDocument");
    }
}
