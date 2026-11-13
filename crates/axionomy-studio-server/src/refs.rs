//! Refs over HTTP (P10, D-046): a ref names the point an application is at,
//! and moving it forward is what makes a step count. A ref moves only from
//! where its mover expects it, so of two movers from one point the second is
//! told where it went (409) and looks again; a follower pulls the steps a ref
//! was moved over after the last point it handled, waiting for the next.

use crate::{ApiError, StudioState};
use axionomy::{Quantity, paths};
use axionomy_engine::{Engine, EngineError, Following, Frame, GoalPath, ObjectivePath};
use axionomy_store::{Point, RefPath, RunId};
use axum::{
    Json,
    extract::{Path, Query, State},
};
use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{borrow::Cow, fmt, str::FromStr, time::Duration};

/// Frames one request returns at most.
const MAX_FRAMES: u64 = 1_000;
/// The longest a request for frames waits for the ref to move.
const MAX_WAIT_SECONDS: u64 = 30;

/// A point of the engine's store: the state after `step` steps of the
/// history of a run, written `run-`, 32 hexadecimal digits, `:` and the
/// step, such as `run-000000000000feed0000000000000007:3`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PointId(Point);

impl PointId {
    pub fn point(self) -> Point {
        self.0
    }
}

impl From<Point> for PointId {
    fn from(point: Point) -> Self {
        Self(point)
    }
}

/// Text that is not a point.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PointIdError(String);

impl fmt::Display for PointIdError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "`{}` is not a point such as `run-000000000000feed0000000000000007:3`",
            self.0
        )
    }
}

impl std::error::Error for PointIdError {}

impl FromStr for PointId {
    type Err = PointIdError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let malformed = || PointIdError(text.to_owned());
        let (run, step) = text.split_once(':').ok_or_else(malformed)?;
        // One spelling per point: decimal digits, without leading zeros.
        if step.is_empty()
            || !step.bytes().all(|byte| byte.is_ascii_digit())
            || (step.len() > 1 && step.starts_with('0'))
        {
            return Err(malformed());
        }
        Ok(Self(Point {
            run: run.parse::<RunId>().map_err(|_| malformed())?,
            step: step.parse().map_err(|_| malformed())?,
        }))
    }
}

impl fmt::Display for PointId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}:{}", self.0.run, self.0.step)
    }
}

impl Serialize for PointId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for PointId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Cow::<str>::deserialize(deserializer)?
            .parse()
            .map_err(serde::de::Error::custom)
    }
}

impl JsonSchema for PointId {
    fn schema_name() -> Cow<'static, str> {
        "PointId".into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "description": "A point of the engine's store: the state after `step` steps of a run's history, written `run-`, 32 hexadecimal digits, `:` and the step.",
            "type": "string",
            "pattern": "^run-[0-9a-f]{32}:(0|[1-9][0-9]*)$"
        })
    }
}

/// Objective paths separated by commas, as a query writes a list.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ObjectiveList(pub Vec<ObjectivePath>);

impl<'de> Deserialize<'de> for ObjectiveList {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = Cow::<str>::deserialize(deserializer)?;
        if text.is_empty() {
            return Ok(Self::default());
        }
        text.split(',')
            .map(str::parse)
            .collect::<Result<_, _>>()
            .map(Self)
            .map_err(serde::de::Error::custom)
    }
}

impl JsonSchema for ObjectiveList {
    fn schema_name() -> Cow<'static, str> {
        "ObjectiveList".into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "description": "Objective paths separated by commas, such as `paid,payments`.",
            "type": "string"
        })
    }
}

/// A ref's name in a URL. A name such as `live/game` is one path segment,
/// its `/` written `%2F`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, JsonSchema)]
pub struct RefName {
    pub name: RefPath,
}

/// A ref to make: its name, which no ref may have yet, and the point it
/// names.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RefCreateRequest {
    pub name: RefPath,
    pub point: PointId,
}

/// A move of a ref forward: from `expected`, where its mover saw it, to `to`,
/// whose history passes through `expected`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RefAdvanceRequest {
    pub expected: PointId,
    pub to: PointId,
}

/// A ref and the point it names.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RefView {
    pub name: RefPath,
    pub point: PointId,
}

/// A move refused because the ref was not where the mover expected it, or a
/// ref made under a name a ref already has: where the ref is, to look again
/// from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RefMoved {
    pub error: String,
    pub name: RefPath,
    pub current: PointId,
}

/// Which steps to deliver, and what to measure at each.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, JsonSchema)]
pub struct RefFramesQuery {
    /// The last point the follower handled: the ref's point when it last
    /// looked, or the last frame's.
    pub after: PointId,
    /// The most frames to return, from 1 to 1000.
    #[serde(default = "default_limit")]
    #[schemars(range(min = 1, max = 1_000))]
    pub limit: u64,
    /// How long to wait, up to 30 seconds, for the ref to move when it is
    /// still at `after`. Zero, the default, returns at once.
    #[serde(default)]
    #[schemars(range(max = 30))]
    pub wait_seconds: u64,
    /// A goal the store keeps; each frame says whether it holds there.
    #[serde(default)]
    pub goal: Option<GoalPath>,
    /// Objectives the store keeps, measured at each frame in this order.
    #[serde(default)]
    pub objectives: Option<ObjectiveList>,
}

const fn default_limit() -> u64 {
    100
}

/// The steps a ref was moved over after a point, and where the ref is.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RefFrames {
    pub name: RefPath,
    /// Where the ref is. While it is past the last frame, more follow.
    pub point: PointId,
    /// The steps, earliest first; empty when the ref has not moved.
    pub frames: Vec<RefFrame>,
}

/// One step as delivered: the point it led to, its exchange and receipt, and
/// what the follower measures there.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RefFrame {
    pub point: PointId,
    pub exchange: paths::Exchange,
    pub receipt: paths::Receipt,
    /// The follower's objectives at `point`, in the order asked.
    pub values: Vec<Quantity>,
    /// Whether the follower's goal holds at `point`, when it follows one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub holds: Option<bool>,
}

impl From<Frame> for RefFrame {
    fn from(frame: Frame) -> Self {
        Self {
            point: frame.point.into(),
            exchange: frame.exchange,
            receipt: frame.receipt,
            values: frame.values,
            holds: frame.holds,
        }
    }
}

pub(crate) async fn create(
    State(state): State<StudioState>,
    Json(request): Json<RefCreateRequest>,
) -> Result<Json<RefView>, ApiError> {
    let (name, at) = (request.name.clone(), request.point.point());
    let made = blocking(&state.engine, move |engine| engine.refer(&name, at)).await;
    match made {
        Ok(()) => Ok(Json(RefView {
            name: request.name,
            point: request.point,
        })),
        Err(EngineError::Moved {
            name,
            current: Some(current),
        }) => Err(ApiError::Moved(RefMoved {
            error: format!("a ref `{name}` names `{}` already", PointId(current)),
            name,
            current: current.into(),
        })),
        Err(error) => Err(error.into()),
    }
}

pub(crate) async fn read(
    State(state): State<StudioState>,
    Path(RefName { name }): Path<RefName>,
) -> Result<Json<RefView>, ApiError> {
    let referred = name.clone();
    let point = blocking(&state.engine, move |engine| engine.referred(&referred)).await?;
    Ok(Json(RefView {
        name,
        point: point.into(),
    }))
}

pub(crate) async fn advance(
    State(state): State<StudioState>,
    Path(RefName { name }): Path<RefName>,
    Json(request): Json<RefAdvanceRequest>,
) -> Result<Json<RefView>, ApiError> {
    let (moved, expected, to) = (name.clone(), request.expected.point(), request.to.point());
    blocking(&state.engine, move |engine| {
        engine.advance(&moved, expected, to)
    })
    .await?;
    Ok(Json(RefView {
        name,
        point: request.to,
    }))
}

pub(crate) async fn frames(
    State(state): State<StudioState>,
    Path(RefName { name }): Path<RefName>,
    Query(query): Query<RefFramesQuery>,
) -> Result<Json<RefFrames>, ApiError> {
    if !(1..=MAX_FRAMES).contains(&query.limit) {
        return Err(ApiError::bad_request(format!(
            "frame limit must be between 1 and {MAX_FRAMES}"
        )));
    }
    if query.wait_seconds > MAX_WAIT_SECONDS {
        return Err(ApiError::bad_request(format!(
            "wait_seconds must be at most {MAX_WAIT_SECONDS}; ask again to wait longer"
        )));
    }
    let following = Following {
        objectives: query.objectives.unwrap_or_default().0,
        goal: query.goal,
    };
    let (followed, after) = (name.clone(), query.after.point());
    let wait = Duration::from_secs(query.wait_seconds);
    let limit = usize::try_from(query.limit).unwrap_or(usize::MAX);
    let (frames, current) = blocking(&state.engine, move |engine| {
        engine.wait(&followed, after, wait)?;
        let frames = engine.frames(&followed, after, &following, limit)?;
        Ok((frames, engine.referred(&followed)?))
    })
    .await?;
    Ok(Json(RefFrames {
        name,
        point: current.into(),
        frames: frames.into_iter().map(RefFrame::from).collect(),
    }))
}

/// Runs the engine's synchronous `work`, which may wait or replay, where
/// blocking is allowed.
async fn blocking<T: Send + 'static>(
    engine: &Engine,
    work: impl FnOnce(&Engine) -> Result<T, EngineError> + Send + 'static,
) -> Result<T, EngineError> {
    let engine = engine.clone();
    tokio::task::spawn_blocking(move || work(&engine))
        .await
        .map_err(|_| EngineError::Store(axionomy_store::StoreError::Closed))?
}

impl From<EngineError> for ApiError {
    fn from(error: EngineError) -> Self {
        match error {
            EngineError::Moved {
                name,
                current: Some(current),
            } => Self::Moved(RefMoved {
                error: format!(
                    "ref `{name}` is not where the move expected it: another move came first, and it is at `{}`; look again from there",
                    PointId(current)
                ),
                name,
                current: current.into(),
            }),
            // A ref that names nothing is not a ref of this store.
            EngineError::Moved {
                name,
                current: None,
            } => Self::not_found(format!("no ref `{name}` names a point")),
            EngineError::UnknownRef(_)
            | EngineError::UnknownPoint(_)
            | EngineError::Undefined(_)
            | EngineError::UnknownScript(_) => Self::not_found(error.to_string()),
            EngineError::Backward { .. } => Self::bad_request(format!(
                "{error}: a ref moves only forward, to a point whose history passes through where it is"
            )),
            EngineError::NotOnHistory { .. } => Self::bad_request(format!(
                "{error}: follow from a point the ref was at or moved over"
            )),
            EngineError::NeedsHistory(_) => Self::bad_request(error.to_string()),
            error => Self::internal(error.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn points_read_back_as_written() {
        let run: RunId = "run-000000000000feed0000000000000007".parse().unwrap();
        let point = PointId(Point { run, step: 12 });
        assert_eq!(point.to_string(), "run-000000000000feed0000000000000007:12");
        assert_eq!(point.to_string().parse::<PointId>(), Ok(point));
        for forged in [
            "",
            "run-000000000000feed0000000000000007",
            "run-000000000000feed0000000000000007:",
            "run-000000000000feed0000000000000007:012",
            "run-000000000000feed0000000000000007:+1",
            "run-000000000000feed0000000000000007:-1",
            "run-feed:1",
        ] {
            assert!(forged.parse::<PointId>().is_err(), "{forged}");
        }
    }
}
