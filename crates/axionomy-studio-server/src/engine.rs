//! The engine's operations over HTTP (P12): packages, a point's history, the
//! candidates at a point, assessing and applying exchanges, running a
//! strategy or an experiment while its progress streams, and comparing
//! points. Each handler calls the operation `axionomy-studio-api` defines,
//! the same one the browser Worker calls, where blocking is allowed.

use crate::{ApiError, StudioState};
use axionomy_engine::{Engine, ExperimentPath};
use axionomy_search::GraphSearchProgress;
use axionomy_studio_api::{
    Ancestor, AncestorQuery, Applied, ApplyRequest, AssessRequest, Candidate, Candidates,
    CandidatesQuery, CompareRequest, ComparisonView, ExperimentEvent, LoadedView, PackageList,
    PointId, Refusal, ResultsView, StrategyEvent, StrategyRequest,
};
use axionomy_view::ViewDocument;
use axum::{
    Json,
    body::Bytes,
    extract::{Path, Query, State},
    response::{
        Sse,
        sse::{Event, KeepAlive},
    },
};
use futures_util::{Stream, StreamExt};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::{
    convert::Infallible,
    ops::ControlFlow,
    time::{Duration, Instant},
};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

/// The longest a run goes between two reports of its search's progress.
const PROGRESS_EVERY: Duration = Duration::from_millis(100);

/// A point in a URL, such as `run-000000000000feed0000000000000007:3`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, JsonSchema)]
pub struct PointName {
    pub point: PointId,
}

/// An experiment's path in a URL. A path such as `maze/micro` is one path
/// segment, its `/` written `%2F`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, JsonSchema)]
pub struct ExperimentName {
    pub experiment: ExperimentPath,
}

pub(crate) async fn packages(
    State(state): State<StudioState>,
) -> Result<Json<PackageList>, ApiError> {
    blocking(&state.engine, axionomy_studio_api::packages)
        .await
        .map(Json)
}

pub(crate) async fn load(
    State(state): State<StudioState>,
    bytes: Bytes,
) -> Result<Json<LoadedView>, ApiError> {
    blocking(&state.engine, move |engine| {
        axionomy_studio_api::load(engine, &bytes)
    })
    .await
    .map(Json)
}

pub(crate) async fn draw(
    State(state): State<StudioState>,
    Path(PointName { point }): Path<PointName>,
) -> Result<Json<ViewDocument>, ApiError> {
    blocking(&state.engine, move |engine| {
        axionomy_studio_api::draw(engine, point)
    })
    .await
    .map(Json)
}

pub(crate) async fn ancestor(
    State(state): State<StudioState>,
    Path(PointName { point }): Path<PointName>,
    Query(query): Query<AncestorQuery>,
) -> Result<Json<Ancestor>, ApiError> {
    blocking(&state.engine, move |engine| {
        axionomy_studio_api::ancestor(engine, point, query)
    })
    .await
    .map(Json)
}

pub(crate) async fn candidates(
    State(state): State<StudioState>,
    Path(PointName { point }): Path<PointName>,
    Query(query): Query<CandidatesQuery>,
) -> Result<Json<Candidates>, ApiError> {
    blocking(&state.engine, move |engine| {
        axionomy_studio_api::candidates(engine, point, query)
    })
    .await
    .map(Json)
}

pub(crate) async fn assess(
    State(state): State<StudioState>,
    Path(PointName { point }): Path<PointName>,
    Json(request): Json<AssessRequest>,
) -> Result<Json<Candidate>, ApiError> {
    blocking(&state.engine, move |engine| {
        axionomy_studio_api::assess(engine, point, request)
    })
    .await
    .map(Json)
}

pub(crate) async fn apply(
    State(state): State<StudioState>,
    Path(PointName { point }): Path<PointName>,
    Json(request): Json<ApplyRequest>,
) -> Result<Json<Applied>, ApiError> {
    blocking(&state.engine, move |engine| {
        axionomy_studio_api::apply(engine, point, request)
    })
    .await
    .map(Json)
}

pub(crate) async fn run_strategy(
    State(state): State<StudioState>,
    Path(PointName { point }): Path<PointName>,
    Json(request): Json<StrategyRequest>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, ApiError> {
    let checked = request.clone();
    blocking(&state.engine, move |engine| {
        axionomy_studio_api::check_strategy(engine, point, &checked)
    })
    .await?;
    Ok(streamed(state.engine, move |engine, mut watch| {
        StrategyEvent::from(axionomy_studio_api::run_strategy(
            engine, point, request, &mut watch,
        ))
    }))
}

pub(crate) async fn results(
    State(state): State<StudioState>,
    Path(ExperimentName { experiment }): Path<ExperimentName>,
) -> Result<Json<ResultsView>, ApiError> {
    blocking(&state.engine, move |engine| {
        axionomy_studio_api::results(engine, experiment)
    })
    .await
    .map(Json)
}

pub(crate) async fn run_experiment(
    State(state): State<StudioState>,
    Path(ExperimentName { experiment }): Path<ExperimentName>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, ApiError> {
    let checked = experiment.clone();
    blocking(&state.engine, move |engine| {
        axionomy_studio_api::check_experiment(engine, &checked)
    })
    .await?;
    Ok(streamed(state.engine, move |engine, mut watch| {
        ExperimentEvent::from(axionomy_studio_api::run_experiment(
            engine, experiment, &mut watch,
        ))
    }))
}

pub(crate) async fn compare(
    State(state): State<StudioState>,
    Json(request): Json<CompareRequest>,
) -> Result<Json<ComparisonView>, ApiError> {
    blocking(&state.engine, move |engine| {
        axionomy_studio_api::compare(engine, request)
    })
    .await
    .map(Json)
}

/// What a running operation streams: its search's progress, then its end.
trait Streamed: Serialize + Send + 'static {
    fn progress(progress: GraphSearchProgress) -> Self;
    /// The event's name in the stream.
    fn kind(&self) -> &'static str;
}

impl Streamed for StrategyEvent {
    fn progress(progress: GraphSearchProgress) -> Self {
        Self::Progress { progress }
    }

    fn kind(&self) -> &'static str {
        match self {
            Self::Progress { .. } => "progress",
            Self::Ran { .. } => "ran",
            Self::Failed { .. } => "failed",
        }
    }
}

impl Streamed for ExperimentEvent {
    fn progress(progress: GraphSearchProgress) -> Self {
        Self::Progress { progress }
    }

    fn kind(&self) -> &'static str {
        match self {
            Self::Progress { .. } => "progress",
            Self::Results { .. } => "results",
            Self::Failed { .. } => "failed",
        }
    }
}

/// Runs `work` where blocking is allowed, streaming its search's progress
/// now and then and its last event once it ends. A client that closes the
/// stream stops the search at its next look at its progress.
fn streamed<E: Streamed>(
    engine: Engine,
    work: impl FnOnce(&Engine, &mut dyn FnMut(&GraphSearchProgress) -> ControlFlow<()>) -> E
    + Send
    + 'static,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let (sender, receiver) = mpsc::channel::<E>(16);
    tokio::task::spawn_blocking(move || {
        let mut reported: Option<Instant> = None;
        let mut watch = |progress: &GraphSearchProgress| {
            if sender.is_closed() {
                return ControlFlow::Break(());
            }
            if reported.is_none_or(|at| at.elapsed() >= PROGRESS_EVERY) {
                reported = Some(Instant::now());
                // A full stream drops a report; a later one says more.
                let _ = sender.try_send(E::progress(*progress));
            }
            ControlFlow::Continue(())
        };
        let last = work(&engine, &mut watch);
        let _ = sender.blocking_send(last);
    });
    let events = ReceiverStream::new(receiver).map(|event| {
        Ok(Event::default()
            .event(event.kind())
            .json_data(&event)
            .expect("an event always serializes"))
    });
    Sse::new(events).keep_alive(KeepAlive::default())
}

/// Runs the engine's synchronous `work`, which may wait or replay, where
/// blocking is allowed.
pub(crate) async fn blocking<T: Send + 'static>(
    engine: &Engine,
    work: impl FnOnce(&Engine) -> Result<T, Refusal> + Send + 'static,
) -> Result<T, ApiError> {
    let engine = engine.clone();
    tokio::task::spawn_blocking(move || work(&engine))
        .await
        .map_err(|_| ApiError(Refusal::Failed("the engine's work was lost".into())))?
        .map_err(ApiError)
}
