//! Runtime-neutral playback and presentation contracts for Axionomy.
//!
//! This crate deliberately contains no world model of its own. A view document
//! is derived by replaying exchanges through [`axionomy::Economy`], and optional
//! scenes are read-only projections supplied by the caller.

use axionomy::{
    AccountAssessment, AccountDelta, ApplyError, Basket, Economy, Exchange, ExchangeAssessment,
    Goal, Quantity, QuantityScalar, Receipt, Trace,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::HashSet, fmt::Debug, hash::Hash, marker::PhantomData};
use thiserror::Error;

/// A browser-safe identity for an arbitrary user-defined ontology value.
///
/// `key` is used for joins, `label` is for people, and `value` is optional
/// diagnostic context. None of these fields replaces the underlying Rust value
/// as semantic authority.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ViewId {
    pub key: String,
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<Value>,
}

impl ViewId {
    pub fn new(key: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            key: key.into(),
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

impl<N> From<&Quantity<N>> for ExactQuantity
where
    N: QuantityScalar,
{
    fn from(value: &Quantity<N>) -> Self {
        Self(value.to_string())
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
    RateOverflow,
    Infeasible,
    BalanceOverflow,
    InvariantOverflow,
    InvariantViolation,
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

/// One immutable transition rule in the closed model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RateView {
    pub rate: ViewId,
    pub roles: Vec<RateRoleView>,
    pub distinct_roles: Vec<[ViewId; 2]>,
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
    pub name: String,
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
    pub classes: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub x: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub y: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct GraphEdgeView {
    pub id: String,
    pub source: String,
    pub target: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default)]
    pub classes: Vec<String>,
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SceneAnchorView {
    GraphNode {
        node: String,
    },
    GraphEdge {
        edge: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        progress: Option<f64>,
    },
    GridCell {
        x: u32,
        y: u32,
    },
    MatrixCell {
        row: String,
        column: String,
    },
    Timeline {
        lane: String,
        at: u64,
    },
    Unanchored,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SceneMetricView {
    pub key: String,
    pub label: String,
    /// Exact text preserves arbitrarily large quantities and caller units.
    pub value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SceneEntityView {
    pub id: ViewId,
    pub glyph: SceneGlyphView,
    pub anchor: SceneAnchorView,
    pub tone: SceneToneView,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account: Option<String>,
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
    pub id: String,
    pub label: String,
    pub anchors: Vec<SceneAnchorView>,
    pub status: ScenePathStatusView,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SceneAnnotationView {
    pub id: String,
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
        focus: Option<String>,
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
}

/// Optional domain projection derived from authoritative economic state.
/// Scenes explain and animate; they never decide whether an exchange applies.
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
    pub label: String,
    #[serde(default)]
    pub classes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct MatrixCellView {
    pub row: String,
    pub column: String,
    pub label: String,
    #[serde(default)]
    pub classes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TimelineLaneView {
    pub id: ViewId,
    #[serde(default)]
    pub classes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TimelineSpanView {
    pub id: String,
    pub lane: String,
    pub start: u64,
    pub end: u64,
    pub label: String,
    #[serde(default)]
    pub classes: Vec<String>,
}

impl Scene {
    pub fn graph(
        title: impl Into<String>,
        nodes: Vec<GraphNodeView>,
        edges: Vec<GraphEdgeView>,
        focus: Option<String>,
    ) -> Self {
        let entities = nodes
            .iter()
            .map(|node| SceneEntityView {
                id: node.id.clone(),
                glyph: inferred_glyph(&node.id.label, &node.classes),
                anchor: SceneAnchorView::GraphNode {
                    node: node.id.key.clone(),
                },
                tone: inferred_tone(&node.classes),
                status: node.classes.first().cloned(),
                account: None,
                metrics: Vec::new(),
            })
            .collect::<Vec<_>>();
        let paths = edges
            .iter()
            .map(|edge| ScenePathView {
                id: edge.id.clone(),
                label: edge.label.clone().unwrap_or_else(|| edge.id.clone()),
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
        let entities = cells
            .iter()
            .filter(|cell| {
                !cell.label.trim().is_empty()
                    && cell.label != "·"
                    && !cell.label.eq_ignore_ascii_case("empty")
            })
            .map(|cell| SceneEntityView {
                id: ViewId::new(
                    format!("cell:{}:{}:{}", cell.x, cell.y, debug_key(&cell.label)),
                    cell.label.clone(),
                ),
                glyph: inferred_glyph(&cell.label, &cell.classes),
                anchor: SceneAnchorView::GridCell {
                    x: cell.x,
                    y: cell.y,
                },
                tone: inferred_tone(&cell.classes),
                status: cell.classes.first().cloned(),
                account: None,
                metrics: Vec::new(),
            })
            .collect::<Vec<_>>();
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
                    format!("matrix:{}:{}", cell.row, cell.column),
                    cell.label.clone(),
                ),
                glyph: SceneGlyphView::Constraint,
                anchor: SceneAnchorView::MatrixCell {
                    row: cell.row.clone(),
                    column: cell.column.clone(),
                },
                tone: inferred_tone(&cell.classes),
                status: cell.classes.first().cloned(),
                account: None,
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
                id: ViewId::new(&span.id, &span.label),
                glyph: inferred_glyph(&span.label, &span.classes),
                anchor: SceneAnchorView::Timeline {
                    lane: span.lane.clone(),
                    at: span.start,
                },
                tone: inferred_tone(&span.classes),
                status: span.classes.first().cloned(),
                account: None,
                metrics: vec![SceneMetricView {
                    key: "duration".into(),
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
            .map(|entity| entity.id.key.as_str())
            .collect::<HashSet<_>>();
        if entity_ids.len() != self.entities.len() {
            return Err(SceneValidationError::DuplicateEntity);
        }
        match &self.surface {
            SceneSurfaceView::Graph { nodes, edges, .. } => {
                let nodes = nodes
                    .iter()
                    .map(|node| node.id.key.as_str())
                    .collect::<HashSet<_>>();
                for edge in edges {
                    if !nodes.contains(edge.source.as_str())
                        || !nodes.contains(edge.target.as_str())
                    {
                        return Err(SceneValidationError::UnknownAnchor(edge.id.clone()));
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
                let rows = rows
                    .iter()
                    .map(|row| row.key.as_str())
                    .collect::<HashSet<_>>();
                let columns = columns
                    .iter()
                    .map(|column| column.key.as_str())
                    .collect::<HashSet<_>>();
                if cells.iter().any(|cell| {
                    !rows.contains(cell.row.as_str()) || !columns.contains(cell.column.as_str())
                }) {
                    return Err(SceneValidationError::UnknownAnchor("matrix cell".into()));
                }
            }
            SceneSurfaceView::Timeline { lanes, spans, .. } => {
                let lanes = lanes
                    .iter()
                    .map(|lane| lane.id.key.as_str())
                    .collect::<HashSet<_>>();
                if spans.iter().any(|span| !lanes.contains(span.lane.as_str())) {
                    return Err(SceneValidationError::UnknownAnchor("timeline span".into()));
                }
            }
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
}

fn inferred_glyph(label: &str, classes: &[String]) -> SceneGlyphView {
    let words = format!("{} {}", label, classes.join(" ")).to_ascii_lowercase();
    if words.contains("vehicle") || words.contains("truck") || words.contains("carrier") {
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

fn inferred_tone(classes: &[String]) -> SceneToneView {
    if classes
        .iter()
        .any(|class| class == "current" || class == "active")
    {
        SceneToneView::Active
    } else if classes.iter().any(|class| class == "goal") {
        SceneToneView::Goal
    } else if classes
        .iter()
        .any(|class| class == "success" || class == "selected")
    {
        SceneToneView::Success
    } else if classes
        .iter()
        .any(|class| class == "danger" || class == "invalid")
    {
        SceneToneView::Danger
    } else if classes
        .iter()
        .any(|class| class == "warning" || class == "blocked")
    {
        SceneToneView::Warning
    } else if classes.iter().any(|class| class == "uncertain") {
        SceneToneView::Uncertain
    } else if classes
        .iter()
        .any(|class| class == "muted" || class == "inactive")
    {
        SceneToneView::Muted
    } else {
        SceneToneView::Neutral
    }
}

fn inferred_path_status(classes: &[String]) -> ScenePathStatusView {
    if classes.iter().any(|class| class == "current") {
        ScenePathStatusView::Current
    } else if classes
        .iter()
        .any(|class| class == "selected" || class == "incumbent")
    {
        ScenePathStatusView::Incumbent
    } else if classes.iter().any(|class| class == "traversed") {
        ScenePathStatusView::Traversed
    } else if classes
        .iter()
        .any(|class| class == "rejected" || class == "invalid")
    {
        ScenePathStatusView::Rejected
    } else if classes.iter().any(|class| class == "blocked") {
        ScenePathStatusView::Blocked
    } else if classes.iter().any(|class| class == "uncertain") {
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ExchangeFrame {
    pub index: u64,
    pub exchange: ExchangeView,
    pub assessment: AssessmentView,
    pub receipt: ReceiptView,
    pub before: ViewSnapshot,
    pub after: ViewSnapshot,
    #[serde(default)]
    pub cues: Vec<FrameCueView>,
    #[serde(default)]
    pub observations: Vec<ObservationView>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ObjectiveView {
    pub key: String,
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
    pub key: String,
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
    pub id: String,
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
/// HTTP, WASM, saved artifacts, and Studio use identical evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SearchObservationView {
    pub sequence: u64,
    pub phase: String,
    pub algorithm: String,
    pub kind: SearchObservationKindView,
    pub label: String,
    pub completed: u64,
    pub total: u64,
    #[serde(default)]
    pub metrics: Vec<SceneMetricView>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ViewDocument {
    pub id: String,
    pub title: String,
    pub description: String,
    pub source: ViewSource,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<ModelView>,
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
    /// Bounded solver history retained so a fast native run or a static Pages
    /// artifact is as inspectable as a live stream.
    #[serde(default)]
    pub solve_observations: Vec<SearchObservationView>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ViewSource {
    pub key: String,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ViewDocumentMetadata {
    pub id: String,
    pub title: String,
    pub description: String,
    pub source: ViewSource,
}

/// Lifecycle events emitted by a Studio run. `sequence` orders transport
/// events; it is not economic time and has no effect on replay.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum StudioEvent {
    RunStarted {
        run_id: String,
        sequence: u64,
        problem: String,
        strategy: String,
    },
    Progress {
        run_id: String,
        sequence: u64,
        completed: u64,
        #[serde(skip_serializing_if = "Option::is_none")]
        total: Option<u64>,
        message: String,
    },
    SearchObservation {
        run_id: String,
        sequence: u64,
        observation: SearchObservationView,
    },
    FrameAppended {
        run_id: String,
        sequence: u64,
        frame_index: u64,
    },
    ArtifactPublished {
        run_id: String,
        sequence: u64,
        artifact_id: String,
        documents: u64,
    },
    RunPaused {
        run_id: String,
        sequence: u64,
    },
    RunResumed {
        run_id: String,
        sequence: u64,
    },
    RunCompleted {
        run_id: String,
        sequence: u64,
        artifact_id: String,
        document_id: String,
    },
    RunCancelled {
        run_id: String,
        sequence: u64,
    },
    RunFailed {
        run_id: String,
        sequence: u64,
        message: String,
    },
}

impl StudioEvent {
    pub const fn sequence(&self) -> u64 {
        match self {
            Self::RunStarted { sequence, .. }
            | Self::Progress { sequence, .. }
            | Self::SearchObservation { sequence, .. }
            | Self::FrameAppended { sequence, .. }
            | Self::ArtifactPublished { sequence, .. }
            | Self::RunPaused { sequence, .. }
            | Self::RunResumed { sequence, .. }
            | Self::RunCompleted { sequence, .. }
            | Self::RunCancelled { sequence, .. }
            | Self::RunFailed { sequence, .. } => *sequence,
        }
    }

    pub fn run_id(&self) -> &str {
        match self {
            Self::RunStarted { run_id, .. }
            | Self::Progress { run_id, .. }
            | Self::SearchObservation { run_id, .. }
            | Self::FrameAppended { run_id, .. }
            | Self::ArtifactPublished { run_id, .. }
            | Self::RunPaused { run_id, .. }
            | Self::RunResumed { run_id, .. }
            | Self::RunCompleted { run_id, .. }
            | Self::RunCancelled { run_id, .. }
            | Self::RunFailed { run_id, .. } => run_id,
        }
    }
}

/// Converts user ontology values into stable, browser-facing identities and
/// may derive a scene from each replayed economy snapshot.
pub trait ViewOntology<AccountId, A, RateId, Role, N = u64> {
    fn account(&self, id: &AccountId) -> ViewId;
    fn asset(&self, id: &A) -> ViewId;
    fn rate(&self, id: &RateId) -> ViewId;
    fn role(&self, id: &Role) -> ViewId;

    fn scene(
        &self,
        _index: u64,
        _economy: &Economy<AccountId, A, RateId, Role, N>,
    ) -> Option<Scene> {
        None
    }
}

/// A useful default for user ontologies and reference fixtures. It preserves
/// exact typed engine values internally while deriving stable diagnostic IDs
/// from `Debug` output at the presentation boundary.
type OntologyMarker<AccountId, A, RateId, Role, N> =
    PhantomData<fn() -> (AccountId, A, RateId, Role, N)>;

pub struct DebugOntology<AccountId, A, RateId, Role, N = u64, SceneFn = NoScene> {
    namespace: String,
    scene: SceneFn,
    marker: OntologyMarker<AccountId, A, RateId, Role, N>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct NoScene;

impl<AccountId, A, RateId, Role, N> DebugOntology<AccountId, A, RateId, Role, N, NoScene> {
    pub fn new(namespace: impl Into<String>) -> Self {
        Self {
            namespace: namespace.into(),
            scene: NoScene,
            marker: PhantomData,
        }
    }
}

impl<AccountId, A, RateId, Role, N, CurrentScene>
    DebugOntology<AccountId, A, RateId, Role, N, CurrentScene>
{
    pub fn with_scene<SceneFn>(
        self,
        scene: SceneFn,
    ) -> DebugOntology<AccountId, A, RateId, Role, N, SceneFn> {
        DebugOntology {
            namespace: self.namespace,
            scene,
            marker: PhantomData,
        }
    }

    fn id<T: Debug>(&self, category: &str, value: &T) -> ViewId {
        let debug = format!("{value:?}");
        ViewId::new(
            format!("{}:{category}:{}", self.namespace, debug_key(&debug)),
            humanize_debug(&debug),
        )
    }
}

impl<AccountId, A, RateId, Role, N> ViewOntology<AccountId, A, RateId, Role, N>
    for DebugOntology<AccountId, A, RateId, Role, N, NoScene>
where
    AccountId: Debug,
    A: Debug,
    RateId: Debug,
    Role: Debug,
{
    fn account(&self, id: &AccountId) -> ViewId {
        self.id("account", id)
    }

    fn asset(&self, id: &A) -> ViewId {
        self.id("asset", id)
    }

    fn rate(&self, id: &RateId) -> ViewId {
        self.id("rate", id)
    }

    fn role(&self, id: &Role) -> ViewId {
        self.id("role", id)
    }
}

impl<AccountId, A, RateId, Role, N, SceneFn> ViewOntology<AccountId, A, RateId, Role, N>
    for DebugOntology<AccountId, A, RateId, Role, N, SceneFn>
where
    AccountId: Debug,
    A: Debug,
    RateId: Debug,
    Role: Debug,
    SceneFn: Fn(u64, &Economy<AccountId, A, RateId, Role, N>) -> Option<Scene>,
{
    fn account(&self, id: &AccountId) -> ViewId {
        self.id("account", id)
    }

    fn asset(&self, id: &A) -> ViewId {
        self.id("asset", id)
    }

    fn rate(&self, id: &RateId) -> ViewId {
        self.id("rate", id)
    }

    fn role(&self, id: &Role) -> ViewId {
        self.id("role", id)
    }

    fn scene(&self, index: u64, economy: &Economy<AccountId, A, RateId, Role, N>) -> Option<Scene> {
        (self.scene)(index, economy)
    }
}

fn debug_key(debug: &str) -> String {
    debug
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-")
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
pub fn derive_document<AccountId, A, RateId, Role, N, O>(
    metadata: ViewDocumentMetadata,
    initial: &Economy<AccountId, A, RateId, Role, N>,
    trace: &Trace<RateId, Role, AccountId, N>,
    ontology: &O,
    objectives: Vec<ObjectiveView>,
) -> Result<ViewDocument, PlaybackError>
where
    AccountId: Clone + Eq + Hash + Ord,
    A: Clone + Eq + Hash + Ord,
    RateId: Clone + Eq + Hash + Ord,
    Role: Clone + Ord,
    N: QuantityScalar,
    O: ViewOntology<AccountId, A, RateId, Role, N>,
{
    let mut economy = initial.fork();
    let initial = snapshot(0, &economy, ontology);
    let mut frames = Vec::with_capacity(trace.exchanges().len());

    for (offset, exchange) in trace.exchanges().iter().enumerate() {
        let index = offset as u64;
        let before = snapshot(index, &economy, ontology);
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
        frames.push(ExchangeFrame {
            index,
            exchange,
            assessment,
            receipt,
            before,
            after,
            cues,
            observations: Vec::new(),
        });
    }

    Ok(ViewDocument {
        id: metadata.id,
        title: metadata.title,
        description: metadata.description,
        source: metadata.source,
        model: None,
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

fn frame_cues(exchange: &ExchangeView, receipt: &ReceiptView) -> Vec<FrameCueView> {
    let mut cues = vec![FrameCueView {
        kind: FrameCueKindView::AtomicExchange,
        label: format!("{} applied atomically", exchange.rate.label),
        details: vec![format!(
            "{} role bindings · {} account deltas",
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
            (FrameCueKindView::Preserved, "verified", &delta.preserved),
        ] {
            if assets.is_empty() {
                continue;
            }
            cues.push(FrameCueView {
                kind,
                label: format!("{} {verb} {}", delta.account.label, assets.len()),
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
pub fn derive_model<AccountId, A, RateId, Role, N, O>(
    economy: &Economy<AccountId, A, RateId, Role, N>,
    goal: &Goal<AccountId, A, N>,
    ontology: &O,
) -> ModelView
where
    AccountId: Clone + Eq + Hash + Ord,
    A: Clone + Eq + Hash + Ord,
    RateId: Clone + Eq + Hash + Ord,
    Role: Clone + Ord,
    N: QuantityScalar,
    O: ViewOntology<AccountId, A, RateId, Role, N>,
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
            Some(RateView {
                rate: ontology.rate(id),
                roles,
                distinct_roles,
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
                name: invariant.name().into(),
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

/// Projects a snapshot without mutating or replaying it.
pub fn derive_snapshot<AccountId, A, RateId, Role, N, O>(
    index: u64,
    economy: &Economy<AccountId, A, RateId, Role, N>,
    ontology: &O,
) -> ViewSnapshot
where
    AccountId: Clone + Eq + Hash + Ord,
    A: Clone + Eq + Hash + Ord,
    RateId: Clone + Eq + Hash + Ord,
    Role: Clone + Ord,
    N: QuantityScalar,
    O: ViewOntology<AccountId, A, RateId, Role, N>,
{
    snapshot(index, economy, ontology)
}

/// Projects an exchange and its non-mutating feasibility assessment.
pub fn derive_proposal<AccountId, A, RateId, Role, N, O>(
    id: impl Into<String>,
    label: impl Into<String>,
    description: impl Into<String>,
    snapshot_index: u64,
    economy: &Economy<AccountId, A, RateId, Role, N>,
    exchange: &Exchange<RateId, Role, AccountId, N>,
    ontology: &O,
) -> ProposalView
where
    AccountId: Clone + Eq + Hash + Ord,
    A: Clone + Eq + Hash + Ord,
    RateId: Clone + Eq + Hash + Ord,
    Role: Clone + Ord,
    N: QuantityScalar,
    O: ViewOntology<AccountId, A, RateId, Role, N>,
{
    ProposalView {
        id: id.into(),
        label: label.into(),
        description: description.into(),
        snapshot_index,
        exchange: exchange_view(exchange, ontology),
        assessment: assessment_view(&economy.assess(exchange), ontology),
    }
}

fn snapshot<AccountId, A, RateId, Role, N, O>(
    index: u64,
    economy: &Economy<AccountId, A, RateId, Role, N>,
    ontology: &O,
) -> ViewSnapshot
where
    AccountId: Clone + Eq + Hash + Ord,
    A: Clone + Eq + Hash + Ord,
    RateId: Clone + Eq + Hash + Ord,
    Role: Clone + Ord,
    N: QuantityScalar,
    O: ViewOntology<AccountId, A, RateId, Role, N>,
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
        scene.metrics.extend([
            SceneMetricView {
                key: "snapshot".into(),
                label: "Economic step".into(),
                value: index.to_string(),
                unit: Some("exchanges".into()),
                previous: index.checked_sub(1).map(|value| value.to_string()),
            },
            SceneMetricView {
                key: "accounts".into(),
                label: "Accounts".into(),
                value: accounts.len().to_string(),
                unit: None,
                previous: None,
            },
            SceneMetricView {
                key: "balances".into(),
                label: "Non-zero balances".into(),
                value: balance_entries.to_string(),
                unit: None,
                previous: None,
            },
        ]);
        scene
    });

    ViewSnapshot {
        index,
        accounts,
        scene,
    }
}

fn basket_view<AccountId, A, RateId, Role, N, O>(
    basket: &Basket<A, N>,
    ontology: &O,
) -> Vec<AssetQuantityView>
where
    N: QuantityScalar,
    O: ViewOntology<AccountId, A, RateId, Role, N>,
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

fn exchange_view<AccountId, A, RateId, Role, N, O>(
    exchange: &Exchange<RateId, Role, AccountId, N>,
    ontology: &O,
) -> ExchangeView
where
    Role: Ord,
    N: QuantityScalar,
    O: ViewOntology<AccountId, A, RateId, Role, N>,
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
    }
}

fn delta_view<AccountId, A, RateId, Role, N, O>(
    delta: &AccountDelta<AccountId, A, N>,
    ontology: &O,
) -> AccountDeltaView
where
    N: QuantityScalar,
    O: ViewOntology<AccountId, A, RateId, Role, N>,
{
    AccountDeltaView {
        account: ontology.account(delta.account()),
        consumed: basket_view(delta.consumed(), ontology),
        produced: basket_view(delta.produced(), ontology),
        preserved: basket_view(delta.preserved(), ontology),
    }
}

fn receipt_view<AccountId, A, RateId, Role, N, O>(
    receipt: &Receipt<RateId, Role, AccountId, A, N>,
    ontology: &O,
) -> ReceiptView
where
    N: QuantityScalar,
    O: ViewOntology<AccountId, A, RateId, Role, N>,
{
    ReceiptView {
        deltas: receipt
            .deltas()
            .iter()
            .map(|delta| delta_view(delta, ontology))
            .collect(),
    }
}

fn account_assessment_view<AccountId, A, RateId, Role, N, O>(
    assessment: &AccountAssessment<AccountId, A, N>,
    ontology: &O,
) -> AccountAssessmentView
where
    N: QuantityScalar,
    O: ViewOntology<AccountId, A, RateId, Role, N>,
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

fn assessment_view<AccountId, A, RateId, Role, N, O>(
    assessment: &ExchangeAssessment<AccountId, A, RateId, Role, N>,
    ontology: &O,
) -> AssessmentView
where
    N: QuantityScalar,
    O: ViewOntology<AccountId, A, RateId, Role, N>,
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

fn assessment_issue_view<AccountId, A, RateId, Role, N, O>(
    issue: &ApplyError<RateId, Role, AccountId, A, N>,
    ontology: &O,
) -> AssessmentIssueView
where
    N: QuantityScalar,
    O: ViewOntology<AccountId, A, RateId, Role, N>,
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
        ApplyError::InvariantOverflow { invariant } => (
            AssessmentIssueKindView::InvariantOverflow,
            format!("Invariant `{invariant}` overflowed during evaluation."),
            Vec::new(),
        ),
        ApplyError::InvariantViolation { invariant, .. } => (
            AssessmentIssueKindView::InvariantViolation,
            format!("Declared invariant `{invariant}` would be violated."),
            Vec::new(),
        ),
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

    struct Ontology;

    impl ViewOntology<AccountId, Asset, RateId, Role> for Ontology {
        fn account(&self, _: &AccountId) -> ViewId {
            ViewId::new("account:agent", "Agent")
        }

        fn asset(&self, asset: &Asset) -> ViewId {
            match asset {
                Asset::Ready => ViewId::new("asset:ready", "Ready"),
                Asset::Done => ViewId::new("asset:done", "Done"),
            }
        }

        fn rate(&self, _: &RateId) -> ViewId {
            ViewId::new("rate:finish", "Finish")
        }

        fn role(&self, _: &Role) -> ViewId {
            ViewId::new("role:actor", "Actor")
        }
    }

    #[test]
    fn replayed_frames_expose_exact_balances_and_receipt_deltas() {
        let economy = EconomyBuilder::new()
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
                id: "test".into(),
                title: "Test".into(),
                description: "A test trace".into(),
                source: ViewSource {
                    key: "test".into(),
                    label: "Test".into(),
                },
            },
            &economy,
            &trace,
            &Ontology,
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
        let economy = EconomyBuilder::new()
            .account(AccountId::Agent, Account::from(basket([(Asset::Ready, 1)])))
            .rate(
                RateId::Finish,
                Rate::new().consume(Role::Actor, basket([(Asset::Ready, 1)])),
            )
            .build()
            .unwrap();
        let proposal = derive_proposal(
            "missing-actor",
            "Missing actor",
            "An intentionally unbound exchange",
            0,
            &economy,
            &Exchange::new(RateId::Finish, Quantity::new(1)),
            &Ontology,
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
        assert_eq!(proposal.assessment.issues[0].subjects[0].key, "role:actor");
    }

    #[test]
    fn studio_events_are_discriminated_unions() {
        let event = StudioEvent::Progress {
            run_id: "run-1".into(),
            sequence: 2,
            completed: 3,
            total: Some(5),
            message: "replaying".into(),
        };
        let value = serde_json::to_value(event).unwrap();
        assert_eq!(value["kind"], "progress");
        assert_eq!(value["completed"], 3);

        let decoded: StudioEvent = serde_json::from_value(value).unwrap();
        assert_eq!(decoded.run_id(), "run-1");
        assert_eq!(decoded.sequence(), 2);
    }

    #[test]
    fn view_document_has_a_json_schema() {
        let schema = schemars::schema_for!(ViewDocument);
        let value = serde_json::to_value(schema).unwrap();
        assert_eq!(value["title"], "ViewDocument");
    }
}
