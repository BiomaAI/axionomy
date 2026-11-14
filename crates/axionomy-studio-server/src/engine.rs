//! The engine's operations over HTTP (P12): packages, a point's history, the
//! candidates at a point, assessing and applying exchanges, running a
//! strategy or an experiment while its progress streams, and comparing
//! points. Each handler calls the operation `axionomy-studio-api` defines,
//! the same one the browser Worker calls, where blocking is allowed.

use crate::{ApiError, StudioState};
use axionomy_engine::{Engine, ExperimentPath, PackagePath};
use axionomy_search::GraphSearchProgress;
use axionomy_studio_api::{
    Ancestor, AncestorQuery, Applied, ApplyRequest, AssessRequest, Balances, Candidate, Candidates,
    CandidatesQuery, ChangeView, CompareRequest, ComparisonView, ExperimentEvent,
    ExperimentProgress, HistoriesView, LoadedView, PackageList, PointId, Refusal, ResultsView,
    StrategyEvent, StrategyRequest,
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
use tokio_stream::wrappers::{BroadcastStream, ReceiverStream};

/// The least time between two reports of a run's progress, unless the
/// later one begins another part of it.
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

/// A package's name in a URL. A name such as `acme/maze` is one path
/// segment, its `/` written `%2F`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, JsonSchema)]
pub struct PackageName {
    pub name: PackagePath,
}

pub(crate) async fn histories(
    State(state): State<StudioState>,
    Path(PackageName { name }): Path<PackageName>,
) -> Result<Json<HistoriesView>, ApiError> {
    blocking(&state.engine, move |engine| {
        axionomy_studio_api::histories(engine, &name)
    })
    .await
    .map(Json)
}

/// Every change to the store from now on, each an event named `change`. A
/// client that falls behind is told [`ChangeView::Definitions`], to read
/// everything again.
pub(crate) async fn changes(
    State(state): State<StudioState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let events = BroadcastStream::new(state.changes.subscribe()).map(|change| {
        let change = change.unwrap_or(ChangeView::Definitions);
        Ok(Event::default()
            .event("change")
            .json_data(&change)
            .expect("a change always serializes"))
    });
    Sse::new(events).keep_alive(KeepAlive::default())
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

pub(crate) async fn balances(
    State(state): State<StudioState>,
    Path(PointName { point }): Path<PointName>,
) -> Result<Json<Balances>, ApiError> {
    blocking(&state.engine, move |engine| {
        axionomy_studio_api::balances(engine, point)
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

/// What a running operation streams: its progress, then its end.
trait Streamed: Serialize + Send + 'static {
    /// How far the operation is.
    type Progress: Copy + Send;
    fn progress(progress: Self::Progress) -> Self;
    /// Whether `progress` begins another part of the operation than `last`
    /// was of, so it is reported however soon after `last` it comes.
    fn begins(progress: &Self::Progress, last: &Self::Progress) -> bool;
    /// The event's name in the stream.
    fn kind(&self) -> &'static str;
}

impl Streamed for StrategyEvent {
    type Progress = GraphSearchProgress;

    fn progress(progress: GraphSearchProgress) -> Self {
        Self::Progress { progress }
    }

    fn begins(_: &GraphSearchProgress, _: &GraphSearchProgress) -> bool {
        false
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
    type Progress = ExperimentProgress;

    fn progress(progress: ExperimentProgress) -> Self {
        Self::Progress { progress }
    }

    /// Each trial is reported as it begins.
    fn begins(progress: &ExperimentProgress, last: &ExperimentProgress) -> bool {
        progress.another_trial(last)
    }

    fn kind(&self) -> &'static str {
        match self {
            Self::Progress { .. } => "progress",
            Self::Results { .. } => "results",
            Self::Failed { .. } => "failed",
        }
    }
}

/// Runs `work` where blocking is allowed, streaming its progress
/// now and then and its last event once it ends. A client that closes the
/// stream stops the run, whatever its kind, at its next look at its progress.
fn streamed<E: Streamed>(
    engine: Engine,
    work: impl FnOnce(&Engine, &mut dyn FnMut(&E::Progress) -> ControlFlow<()>) -> E + Send + 'static,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let (sender, receiver) = mpsc::channel::<E>(16);
    tokio::task::spawn_blocking(move || {
        let mut reported: Option<(Instant, E::Progress)> = None;
        let mut watch = |progress: &E::Progress| {
            if sender.is_closed() {
                return ControlFlow::Break(());
            }
            if reported.is_none_or(|(at, last)| {
                at.elapsed() >= PROGRESS_EVERY || E::begins(progress, &last)
            }) {
                reported = Some((Instant::now(), *progress));
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
