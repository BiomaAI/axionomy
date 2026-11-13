//! Refs (P10, D-046): a ref names the point an application is at, and
//! moving it forward is what makes a step count. A ref moves only from where
//! its mover expects it, so of two movers from one point the second is told
//! where it went and looks again; a follower pulls the steps a ref was moved
//! over after the last point it handled.

use crate::{ObjectiveList, PointId, RefMoved, Refusal};
use axionomy::{Quantity, paths};
use axionomy_engine::{Engine, EngineError, Following, Frame, GoalPath};
use axionomy_store::{Point, RefPath};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Frames one request returns at most.
pub const MAX_FRAMES: u64 = 1_000;
/// The longest a request for frames waits for the ref to move.
pub const MAX_WAIT_SECONDS: u64 = 30;

/// A ref to make: its name, which no ref may have yet, and the point it
/// names.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RefCreateRequest {
    pub name: RefPath,
    pub point: PointId,
}

/// A move of a ref forward: from `expected`, where its mover saw it, to `to`,
/// whose history passes through `expected`. Asked again once made, with the
/// ref at `to`, it succeeds.
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

/// Which steps to deliver, and what to measure at each.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub goal: Option<GoalPath>,
    /// Objectives the store keeps, measured at each frame in this order.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub objectives: Option<ObjectiveList>,
}

const fn default_limit() -> u64 {
    100
}

impl RefFramesQuery {
    /// How long the request may wait for the ref to move, once its limits
    /// are checked.
    pub fn wait(&self) -> Result<Duration, Refusal> {
        if !(1..=MAX_FRAMES).contains(&self.limit) {
            return Err(Refusal::BadRequest(format!(
                "frame limit must be between 1 and {MAX_FRAMES}"
            )));
        }
        if self.wait_seconds > MAX_WAIT_SECONDS {
            return Err(Refusal::BadRequest(format!(
                "wait_seconds must be at most {MAX_WAIT_SECONDS}; ask again to wait longer"
            )));
        }
        Ok(Duration::from_secs(self.wait_seconds))
    }
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
    /// The point the step led to, named as its mover named it when it
    /// advanced the ref there or over it.
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

/// Names a point with a new ref.
pub fn create_ref(engine: &Engine, request: RefCreateRequest) -> Result<RefView, Refusal> {
    match engine.refer(&request.name, request.point.point()) {
        Ok(()) => Ok(RefView {
            name: request.name,
            point: request.point,
        }),
        Err(EngineError::Moved {
            name,
            current: Some(current),
        }) => Err(Refusal::Moved(RefMoved {
            error: format!("a ref `{name}` names `{}` already", PointId::from(current)),
            name,
            current: current.into(),
        })),
        Err(error) => Err(error.into()),
    }
}

/// The point a ref names.
pub fn read_ref(engine: &Engine, name: RefPath) -> Result<RefView, Refusal> {
    let point = engine.referred(&name)?;
    Ok(RefView {
        name,
        point: point.into(),
    })
}

/// Moves a ref forward from where its mover saw it, or finds it moved
/// there already.
pub fn advance_ref(
    engine: &Engine,
    name: RefPath,
    request: RefAdvanceRequest,
) -> Result<RefView, Refusal> {
    engine.advance(&name, request.expected.point(), request.to.point())?;
    Ok(RefView {
        name,
        point: request.to,
    })
}

/// The steps a ref was moved over after `query.after`, at once: a transport
/// that waits for the ref to move does so first, for at most
/// [`RefFramesQuery::wait`].
pub fn frames(engine: &Engine, name: RefPath, query: RefFramesQuery) -> Result<RefFrames, Refusal> {
    query.wait()?;
    let following = Following {
        objectives: query.objectives.unwrap_or_default().0,
        goal: query.goal,
    };
    let limit = usize::try_from(query.limit).unwrap_or(usize::MAX);
    let frames = engine.frames(&name, query.after.point(), &following, limit)?;
    let current: Point = engine.referred(&name)?;
    Ok(RefFrames {
        name,
        point: current.into(),
        frames: frames.into_iter().map(RefFrame::from).collect(),
    })
}
