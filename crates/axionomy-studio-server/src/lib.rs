//! Native HTTP adapter for the interface-neutral Axionomy service.

use aide::{
    IntoApi, UseApi,
    axum::{ApiRouter, routing},
    openapi::{Info, OpenApi},
};
use axionomy_service::{
    ProblemDescriptor, ReferenceService, RunArtifact, RunControl, RunObserver, RunRequest,
    ServiceError, ServiceProgress,
};
use axionomy_view::{ExchangeFrame, SearchObservationView, StudioEvent, ViewDocument};
use axum::{
    Extension, Json, Router,
    extract::{Path, Query, State},
    http::{HeaderValue, StatusCode},
    response::{IntoResponse, Sse, sse::Event},
    routing::get,
};
use futures_util::StreamExt;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    convert::Infallible,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};
use tokio::sync::{RwLock, broadcast, mpsc};
use tokio_stream::wrappers::BroadcastStream;
use tower_http::{
    cors::CorsLayer,
    services::{ServeDir, ServeFile},
    trace::TraceLayer,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ProblemList {
    pub problems: Vec<ProblemDescriptor>,
}

/// A deliberately small liveness contract. A successful response proves that
/// the native engine adapter is reachable now; loading a cached catalog does
/// not.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct HealthResponse {
    pub status: String,
    pub engine: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Running,
    Paused,
    Completed,
    Cancelled,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct RunSummary {
    pub id: String,
    pub request: RunRequest,
    pub status: RunStatus,
    pub completed: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artifact_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_document_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_event: Option<StudioEvent>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct FramePage {
    pub artifact_id: String,
    pub document_id: String,
    pub from: u64,
    pub total: u64,
    pub frames: Vec<ExchangeFrame>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, JsonSchema)]
pub struct FrameQuery {
    #[serde(default)]
    pub from: u64,
    #[serde(default = "default_frame_limit")]
    pub limit: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, JsonSchema)]
pub struct RunPath {
    pub run_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, JsonSchema)]
pub struct ArtifactPath {
    pub artifact_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, JsonSchema)]
pub struct ArtifactDocumentPath {
    pub artifact_id: String,
    pub document_id: String,
}

const fn default_frame_limit() -> u64 {
    100
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ErrorResponse {
    pub error: String,
}

struct EventStreamContract;

impl aide::OperationOutput for EventStreamContract {
    type Inner = StudioEvent;

    fn operation_response(
        context: &mut aide::generate::GenContext,
        operation: &mut aide::openapi::Operation,
    ) -> Option<aide::openapi::Response> {
        let mut response =
            <String as aide::OperationOutput>::operation_response(context, operation)?;
        let mut media = response.content.shift_remove("text/plain; charset=utf-8")?;
        media.schema = Some(aide::openapi::SchemaObject {
            json_schema: context.schema.subschema_for::<StudioEvent>(),
            example: None,
            external_docs: None,
        });
        response.content.insert("text/event-stream".into(), media);
        response.description = "Tagged StudioEvent values encoded in SSE data fields".into();
        Some(response)
    }

    fn inferred_responses(
        context: &mut aide::generate::GenContext,
        operation: &mut aide::openapi::Operation,
    ) -> Vec<(Option<aide::openapi::StatusCode>, aide::openapi::Response)> {
        Self::operation_response(context, operation)
            .into_iter()
            .map(|response| (Some(aide::openapi::StatusCode::Code(200)), response))
            .collect()
    }
}

#[derive(Debug)]
struct RunRecord {
    summary: RwLock<RunSummary>,
    history: RwLock<Vec<StudioEvent>>,
    events: broadcast::Sender<StudioEvent>,
    control: Arc<RunControl>,
    generation: AtomicU64,
    next_sequence: AtomicU64,
}

enum WorkerUpdate {
    Progress(ServiceProgress),
    Observation(SearchObservationView),
    Frame {
        document_id: String,
        frame: Box<axionomy_view::ExchangeFrame>,
    },
}

struct ChannelObserver {
    updates: mpsc::Sender<WorkerUpdate>,
}

impl RunObserver for ChannelObserver {
    fn progress(&mut self, progress: ServiceProgress) {
        let _ = self.updates.blocking_send(WorkerUpdate::Progress(progress));
    }

    fn observation(&mut self, observation: SearchObservationView) {
        let _ = self
            .updates
            .blocking_send(WorkerUpdate::Observation(observation));
    }

    fn frame(&mut self, document_id: &str, frame: axionomy_view::ExchangeFrame) {
        let _ = self.updates.blocking_send(WorkerUpdate::Frame {
            document_id: document_id.into(),
            frame: Box::new(frame),
        });
    }
}

impl RunRecord {
    fn sequence(&self) -> u64 {
        self.next_sequence.fetch_add(1, Ordering::Relaxed)
    }

    async fn emit(&self, event: StudioEvent) {
        self.summary.write().await.last_event = Some(event.clone());
        self.history.write().await.push(event.clone());
        let _ = self.events.send(event);
    }
}

#[derive(Debug, Clone)]
pub struct StudioState {
    runs: Arc<RwLock<HashMap<String, Arc<RunRecord>>>>,
    artifacts: Arc<RwLock<HashMap<String, Arc<RunArtifact>>>>,
    next_run: Arc<AtomicU64>,
}

impl Default for StudioState {
    fn default() -> Self {
        Self {
            runs: Arc::new(RwLock::new(HashMap::new())),
            artifacts: Arc::new(RwLock::new(HashMap::new())),
            next_run: Arc::new(AtomicU64::new(1)),
        }
    }
}

#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    fn not_found(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message: message.into(),
        }
    }

    fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        (
            self.status,
            Json(ErrorResponse {
                error: self.message,
            }),
        )
            .into_response()
    }
}

impl aide::OperationOutput for ApiError {
    type Inner = ErrorResponse;

    fn operation_response(
        context: &mut aide::generate::GenContext,
        operation: &mut aide::openapi::Operation,
    ) -> Option<aide::openapi::Response> {
        <Json<ErrorResponse> as aide::OperationOutput>::operation_response(context, operation)
    }

    fn inferred_responses(
        context: &mut aide::generate::GenContext,
        operation: &mut aide::openapi::Operation,
    ) -> Vec<(Option<aide::openapi::StatusCode>, aide::openapi::Response)> {
        Self::operation_response(context, operation)
            .into_iter()
            .map(|response| (None, response))
            .collect()
    }
}

pub fn problem_catalog() -> Vec<ProblemDescriptor> {
    ReferenceService.catalog()
}

/// Builds both the Axum router and its OpenAPI 3.1 contract from the same Rust
/// handlers and Schemars types.
pub fn api(state: StudioState) -> (Router, OpenApi) {
    let mut openapi = OpenApi {
        info: Info {
            title: "Axionomy Service API".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            description: Some(
                "Interface-neutral problem commands, replay artifacts, lifecycle control, and Studio data."
                    .into(),
            ),
            ..Info::default()
        },
        ..OpenApi::default()
    };

    let api = ApiRouter::new()
        .api_route(
            "/api/health",
            routing::get_with(health, |operation| {
                operation
                    .id("getHealth")
                    .summary("Check whether the native Studio engine is reachable")
                    .tag("system")
            }),
        )
        .api_route(
            "/api/problems",
            routing::get_with(list_problems, |operation| {
                operation
                    .id("listProblems")
                    .summary("List problems, strategies, and capabilities")
                    .tag("problems")
            }),
        )
        .api_route(
            "/api/runs",
            routing::post_with(create_run, |operation| {
                operation
                    .id("createRun")
                    .summary("Start a reproducible, cooperatively controlled run")
                    .tag("runs")
            }),
        )
        .api_route(
            "/api/runs/{run_id}",
            routing::get_with(get_run, |operation| {
                operation
                    .id("getRun")
                    .summary("Inspect run state")
                    .tag("runs")
            })
            .delete_with(cancel_run, |operation| {
                operation
                    .id("cancelRun")
                    .summary("Cancel a run")
                    .tag("runs")
            }),
        )
        .api_route(
            "/api/runs/{run_id}/pause",
            routing::post_with(pause_run, |operation| {
                operation.id("pauseRun").summary("Pause a run").tag("runs")
            }),
        )
        .api_route(
            "/api/runs/{run_id}/resume",
            routing::post_with(resume_run, |operation| {
                operation
                    .id("resumeRun")
                    .summary("Resume a paused run")
                    .tag("runs")
            }),
        )
        .api_route(
            "/api/runs/{run_id}/events",
            routing::get_with(run_events, |operation| {
                operation
                    .id("streamRunEvents")
                    .summary("Follow resumable ordered lifecycle events over SSE")
                    .tag("runs")
            }),
        )
        .api_route(
            "/api/artifacts/{artifact_id}",
            routing::get_with(get_artifact, |operation| {
                operation
                    .id("getRunArtifact")
                    .summary("Load a complete portable problem artifact")
                    .tag("artifacts")
            }),
        )
        .api_route(
            "/api/artifacts/{artifact_id}/documents/{document_id}",
            routing::get_with(get_document, |operation| {
                operation
                    .id("getViewDocument")
                    .summary("Load one replay-derived alternative")
                    .tag("artifacts")
            }),
        )
        .api_route(
            "/api/artifacts/{artifact_id}/documents/{document_id}/frames",
            routing::get_with(get_frames, |operation| {
                operation
                    .id("getTraceFrames")
                    .summary("Page through replay-derived exchange frames")
                    .tag("artifacts")
            }),
        )
        .with_state(state)
        .finish_api(&mut openapi)
        .layer(Extension(Arc::new(openapi.clone())))
        .route("/api/openapi.json", get(serve_openapi))
        .layer(CorsLayer::new().allow_origin(HeaderValue::from_static("*")))
        .layer(TraceLayer::new_for_http());

    (api, openapi)
}

pub fn with_studio_frontend(router: Router, directory: impl Into<PathBuf>) -> Router {
    let directory = directory.into();
    let index = directory.join("index.html");
    if index.exists() {
        router.fallback_service(ServeDir::new(directory).not_found_service(ServeFile::new(index)))
    } else {
        router
    }
}

async fn serve_openapi(Extension(api): Extension<Arc<OpenApi>>) -> Json<OpenApi> {
    Json((*api).clone())
}

async fn list_problems() -> Json<ProblemList> {
    Json(ProblemList {
        problems: problem_catalog(),
    })
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".into(),
        engine: "native".into(),
    })
}

async fn create_run(
    State(state): State<StudioState>,
    Json(request): Json<RunRequest>,
) -> Result<Json<RunSummary>, ApiError> {
    validate_request(&request)?;
    let run_id = format!("run-{}", state.next_run.fetch_add(1, Ordering::Relaxed));
    let (events, _) = broadcast::channel(256);
    let summary = RunSummary {
        id: run_id.clone(),
        request: request.clone(),
        status: RunStatus::Running,
        completed: 0,
        total: None,
        artifact_id: None,
        selected_document_id: None,
        message: Some("run queued".into()),
        last_event: None,
    };
    let record = Arc::new(RunRecord {
        summary: RwLock::new(summary.clone()),
        history: RwLock::new(Vec::new()),
        events,
        control: Arc::new(RunControl::default()),
        generation: AtomicU64::new(1),
        next_sequence: AtomicU64::new(0),
    });
    state
        .runs
        .write()
        .await
        .insert(run_id.clone(), Arc::clone(&record));
    spawn_run(state, record, run_id, request, 1);
    Ok(Json(summary))
}

fn validate_request(request: &RunRequest) -> Result<(), ApiError> {
    let service = ReferenceService;
    let problem = service
        .problem(&request.problem)
        .ok_or_else(|| ApiError::bad_request(format!("unknown problem `{}`", request.problem)))?;
    if let Some(strategy) = &request.strategy
        && !problem
            .strategies
            .iter()
            .any(|known| &known.key == strategy)
    {
        return Err(ApiError::bad_request(format!(
            "unknown strategy `{strategy}` for problem `{}`",
            request.problem
        )));
    }
    if let Some(instance) = &request.instance
        && !problem.instances.iter().any(|known| &known.key == instance)
    {
        return Err(ApiError::bad_request(format!(
            "unknown instance `{instance}` for problem `{}`",
            request.problem
        )));
    }
    if request.budget == 0 {
        return Err(ApiError::bad_request("budget must be greater than zero"));
    }
    Ok(())
}

fn spawn_run(
    state: StudioState,
    record: Arc<RunRecord>,
    run_id: String,
    request: RunRequest,
    generation: u64,
) {
    tokio::spawn(execute_run(state, record, run_id, request, generation));
}

async fn execute_run(
    state: StudioState,
    record: Arc<RunRecord>,
    run_id: String,
    request: RunRequest,
    generation: u64,
) {
    let strategy = request
        .strategy
        .clone()
        .or_else(|| {
            ReferenceService
                .problem(&request.problem)
                .map(|item| item.default_strategy)
        })
        .unwrap_or_default();
    record
        .emit(StudioEvent::RunStarted {
            run_id: run_id.clone(),
            sequence: record.sequence(),
            problem: request.problem.clone(),
            strategy,
        })
        .await;

    let (updates_tx, mut updates_rx) = mpsc::channel::<WorkerUpdate>(64);
    let worker_control = Arc::clone(&record.control);
    let worker_request = request.clone();
    let worker = tokio::task::spawn_blocking(move || {
        let mut observer = ChannelObserver {
            updates: updates_tx,
        };
        ReferenceService.run_with(&worker_request, &worker_control, &mut observer)
    });

    while let Some(update) = updates_rx.recv().await {
        if record.generation.load(Ordering::Acquire) != generation {
            return;
        }
        match update {
            WorkerUpdate::Progress(progress) => {
                {
                    let mut summary = record.summary.write().await;
                    summary.completed = progress.completed;
                    summary.total = Some(progress.total);
                    summary.message = Some(progress.message.clone());
                }
                record
                    .emit(StudioEvent::Progress {
                        run_id: run_id.clone(),
                        sequence: record.sequence(),
                        completed: progress.completed,
                        total: Some(progress.total),
                        message: format!("{}: {}", progress.phase, progress.message),
                    })
                    .await;
            }
            WorkerUpdate::Observation(observation) => {
                record
                    .emit(StudioEvent::SearchObservation {
                        run_id: run_id.clone(),
                        sequence: record.sequence(),
                        observation,
                    })
                    .await;
            }
            WorkerUpdate::Frame { document_id, frame } => {
                let frame_index = frame.index;
                record
                    .emit(StudioEvent::FrameAppended {
                        run_id: run_id.clone(),
                        sequence: record.sequence(),
                        document_id,
                        frame_index,
                        frame,
                    })
                    .await;
            }
        }
    }

    let result = match worker.await {
        Ok(result) => result,
        Err(error) => Err(ServiceError::Problem {
            problem: request.problem.clone(),
            message: format!("worker failed: {error}"),
        }),
    };
    if record.generation.load(Ordering::Acquire) != generation {
        return;
    }
    match result {
        Ok(artifact) => publish_artifact(&state, &record, &run_id, artifact).await,
        Err(ServiceError::Cancelled) => finish_cancelled(&record, &run_id).await,
        Err(ServiceError::Paused) => {}
        Err(error) => finish_failed(&record, &run_id, error.to_string()).await,
    }
}

async fn publish_artifact(
    state: &StudioState,
    record: &RunRecord,
    run_id: &str,
    artifact: RunArtifact,
) {
    let artifact_id = artifact.id.clone();
    let document_id = artifact.selected_document_id.clone();
    let total = artifact
        .documents
        .iter()
        .map(|document| document.frames.len() as u64)
        .sum();
    state
        .artifacts
        .write()
        .await
        .insert(artifact_id.clone(), Arc::new(artifact.clone()));
    record
        .emit(StudioEvent::ArtifactPublished {
            run_id: run_id.into(),
            sequence: record.sequence(),
            artifact_id: artifact_id.clone(),
            documents: artifact.documents.len() as u64,
        })
        .await;
    {
        let mut summary = record.summary.write().await;
        summary.status = RunStatus::Completed;
        summary.completed = total;
        summary.total = Some(total);
        summary.artifact_id = Some(artifact_id.clone());
        summary.selected_document_id = Some(document_id.clone());
        summary.message = Some("all replay artifacts verified".into());
    }
    record
        .emit(StudioEvent::RunCompleted {
            run_id: run_id.into(),
            sequence: record.sequence(),
            artifact_id,
            document_id,
        })
        .await;
}

async fn finish_cancelled(record: &RunRecord, run_id: &str) {
    {
        let mut summary = record.summary.write().await;
        summary.status = RunStatus::Cancelled;
        summary.message = Some("cancelled by caller".into());
    }
    record
        .emit(StudioEvent::RunCancelled {
            run_id: run_id.into(),
            sequence: record.sequence(),
        })
        .await;
}

async fn finish_failed(record: &RunRecord, run_id: &str, message: String) {
    {
        let mut summary = record.summary.write().await;
        summary.status = RunStatus::Failed;
        summary.message = Some(message.clone());
    }
    record
        .emit(StudioEvent::RunFailed {
            run_id: run_id.into(),
            sequence: record.sequence(),
            message,
        })
        .await;
}

async fn get_run(
    State(state): State<StudioState>,
    Path(path): Path<RunPath>,
) -> Result<Json<RunSummary>, ApiError> {
    let record = find_run(&state, &path.run_id).await?;
    let summary = record.summary.read().await.clone();
    Ok(Json(summary))
}

async fn cancel_run(
    State(state): State<StudioState>,
    Path(path): Path<RunPath>,
) -> Result<Json<RunSummary>, ApiError> {
    let record = find_run(&state, &path.run_id).await?;
    record.generation.fetch_add(1, Ordering::AcqRel);
    record.control.cancel();
    finish_cancelled(&record, &path.run_id).await;
    Ok(Json(record.summary.read().await.clone()))
}

async fn pause_run(
    State(state): State<StudioState>,
    Path(path): Path<RunPath>,
) -> Result<Json<RunSummary>, ApiError> {
    let record = find_run(&state, &path.run_id).await?;
    {
        let summary = record.summary.read().await;
        if summary.status != RunStatus::Running {
            return Err(ApiError::bad_request("only a running run can be paused"));
        }
    }
    record.control.pause();
    {
        let mut summary = record.summary.write().await;
        summary.status = RunStatus::Paused;
        summary.message = Some("paused by caller".into());
    }
    record
        .emit(StudioEvent::RunPaused {
            run_id: path.run_id,
            sequence: record.sequence(),
        })
        .await;
    Ok(Json(record.summary.read().await.clone()))
}

async fn resume_run(
    State(state): State<StudioState>,
    Path(path): Path<RunPath>,
) -> Result<Json<RunSummary>, ApiError> {
    let record = find_run(&state, &path.run_id).await?;
    {
        let mut summary = record.summary.write().await;
        if summary.status != RunStatus::Paused {
            return Err(ApiError::bad_request("only a paused run can be resumed"));
        }
        summary.status = RunStatus::Running;
        summary.message = Some("resumed by caller".into());
    }
    record.control.resume();
    record
        .emit(StudioEvent::RunResumed {
            run_id: path.run_id.clone(),
            sequence: record.sequence(),
        })
        .await;
    drop(state);
    Ok(Json(record.summary.read().await.clone()))
}

async fn find_run(state: &StudioState, run_id: &str) -> Result<Arc<RunRecord>, ApiError> {
    state
        .runs
        .read()
        .await
        .get(run_id)
        .cloned()
        .ok_or_else(|| ApiError::not_found(format!("run `{run_id}` was not found")))
}

async fn find_artifact(
    state: &StudioState,
    artifact_id: &str,
) -> Result<Arc<RunArtifact>, ApiError> {
    state
        .artifacts
        .read()
        .await
        .get(artifact_id)
        .cloned()
        .ok_or_else(|| ApiError::not_found(format!("artifact `{artifact_id}` was not found")))
}

async fn get_artifact(
    State(state): State<StudioState>,
    Path(path): Path<ArtifactPath>,
) -> Result<Json<RunArtifact>, ApiError> {
    Ok(Json(
        (*find_artifact(&state, &path.artifact_id).await?).clone(),
    ))
}

async fn get_document(
    State(state): State<StudioState>,
    Path(path): Path<ArtifactDocumentPath>,
) -> Result<Json<ViewDocument>, ApiError> {
    let artifact = find_artifact(&state, &path.artifact_id).await?;
    let document = artifact
        .documents
        .iter()
        .find(|document| document.id == path.document_id)
        .cloned()
        .ok_or_else(|| {
            ApiError::not_found(format!("document `{}` was not found", path.document_id))
        })?;
    Ok(Json(document))
}

async fn get_frames(
    State(state): State<StudioState>,
    Path(path): Path<ArtifactDocumentPath>,
    Query(query): Query<FrameQuery>,
) -> Result<Json<FramePage>, ApiError> {
    if query.limit == 0 || query.limit > 1_000 {
        return Err(ApiError::bad_request(
            "frame limit must be between 1 and 1000",
        ));
    }
    let artifact = find_artifact(&state, &path.artifact_id).await?;
    let document = artifact
        .documents
        .iter()
        .find(|document| document.id == path.document_id)
        .ok_or_else(|| {
            ApiError::not_found(format!("document `{}` was not found", path.document_id))
        })?;
    let total = document.frames.len() as u64;
    let from = query.from.min(total);
    let end = from.saturating_add(query.limit).min(total);
    Ok(Json(FramePage {
        artifact_id: path.artifact_id,
        document_id: path.document_id,
        from,
        total,
        frames: document.frames[from as usize..end as usize].to_vec(),
    }))
}

async fn run_events(
    State(state): State<StudioState>,
    Path(path): Path<RunPath>,
) -> Result<
    UseApi<
        Sse<impl futures_util::Stream<Item = Result<Event, Infallible>> + Send + 'static>,
        EventStreamContract,
    >,
    ApiError,
> {
    let record = find_run(&state, &path.run_id).await?;
    let receiver = record.events.subscribe();
    let history = record.history.read().await.clone();
    let last_history_sequence = history.last().map(StudioEvent::sequence);
    let live = BroadcastStream::new(receiver).filter_map(move |event| {
        let event = event
            .ok()
            .filter(|event| last_history_sequence.is_none_or(|last| event.sequence() > last));
        async move { event }
    });
    let stream = tokio_stream::iter(history).chain(live).map(|event| {
        let kind = match &event {
            StudioEvent::RunStarted { .. } => "run_started",
            StudioEvent::Progress { .. } => "progress",
            StudioEvent::SearchObservation { .. } => "search_observation",
            StudioEvent::FrameAppended { .. } => "frame_appended",
            StudioEvent::ArtifactPublished { .. } => "artifact_published",
            StudioEvent::RunPaused { .. } => "run_paused",
            StudioEvent::RunResumed { .. } => "run_resumed",
            StudioEvent::RunCompleted { .. } => "run_completed",
            StudioEvent::RunCancelled { .. } => "run_cancelled",
            StudioEvent::RunFailed { .. } => "run_failed",
        };
        Ok(Event::default()
            .event(kind)
            .json_data(event)
            .expect("StudioEvent always serializes"))
    });
    Ok(Sse::new(stream).into_api())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    async fn response_json<T: serde::de::DeserializeOwned>(
        response: axum::response::Response,
    ) -> T {
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[tokio::test]
    async fn service_artifact_is_identical_over_http() {
        let (app, _) = api(StudioState::default());
        let request = RunRequest::new("maze").with_strategy("a_star");
        let create = app
            .clone()
            .oneshot(
                Request::post("/api/runs")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&request).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(create.status(), StatusCode::OK);
        let created: RunSummary = response_json(create).await;
        let completed = loop {
            let response = app
                .clone()
                .oneshot(
                    Request::get(format!("/api/runs/{}", created.id))
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            let summary: RunSummary = response_json(response).await;
            if summary.status != RunStatus::Running {
                break summary;
            }
            tokio::task::yield_now().await;
        };
        assert_eq!(completed.status, RunStatus::Completed);
        let artifact_id = completed.artifact_id.unwrap();
        let response = app
            .oneshot(
                Request::get(format!("/api/artifacts/{artifact_id}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let http_artifact: RunArtifact = response_json(response).await;
        let service_artifact = ReferenceService.run(request).unwrap();
        assert_eq!(http_artifact, service_artifact);
    }

    #[tokio::test]
    async fn catalog_exposes_all_problems() {
        let (app, _) = api(StudioState::default());
        let response = app
            .oneshot(Request::get("/api/problems").body(Body::empty()).unwrap())
            .await
            .unwrap();
        let catalog: ProblemList = response_json(response).await;
        assert_eq!(catalog.problems.len(), 14);
    }

    #[test]
    fn openapi_contains_full_service_surface() {
        let (_, openapi) = api(StudioState::default());
        let value = serde_json::to_value(openapi).unwrap();
        let paths = value["paths"].as_object().unwrap();
        for path in [
            "/api/problems",
            "/api/runs",
            "/api/runs/{run_id}",
            "/api/runs/{run_id}/pause",
            "/api/runs/{run_id}/resume",
            "/api/runs/{run_id}/events",
            "/api/artifacts/{artifact_id}",
            "/api/artifacts/{artifact_id}/documents/{document_id}",
            "/api/artifacts/{artifact_id}/documents/{document_id}/frames",
        ] {
            assert!(paths.contains_key(path), "missing {path}");
        }
        let schemas = value["components"]["schemas"].as_object().unwrap();
        assert!(schemas.contains_key("StudioEvent"));
        assert!(schemas.contains_key("RunArtifact"));
    }
}
