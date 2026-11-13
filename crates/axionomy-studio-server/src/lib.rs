//! Native HTTP adapter for the interface-neutral Axionomy study runtime, and
//! for the refs of the engine's store.

mod openapi;
mod refs;

pub use openapi::OpenApiDocument;
pub use refs::{
    ObjectiveList, PointId, PointIdError, RefAdvanceRequest, RefCreateRequest, RefFrame, RefFrames,
    RefFramesQuery, RefMoved, RefName, RefView,
};

use axionomy_engine::Engine;
use axionomy_study::{
    ProblemDescriptor, Progress, Registry, StudioEvent, StudioStudyId, Study, StudyControl,
    StudyError, StudyKey, StudyObserver, StudyRequest,
};
use axionomy_view::{DocumentPath, ExchangeFrame, SearchObservationView, ViewDocument};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderValue, StatusCode},
    response::{IntoResponse, Sse, sse::Event},
    routing::{delete, get, post},
};
use futures_util::StreamExt;
use openapi::Endpoint;
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
pub enum StudyStatus {
    Studying,
    Paused,
    Completed,
    Cancelled,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct StudySummary {
    pub id: StudioStudyId,
    pub request: StudyRequest,
    pub status: StudyStatus,
    pub completed: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<u64>,
    /// What the study was computed from, once it is published.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<StudyKey>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_document: Option<DocumentPath>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_event: Option<StudioEvent>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct FramePage {
    pub study_id: StudioStudyId,
    pub document: DocumentPath,
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
pub struct StudyPath {
    pub study_id: StudioStudyId,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, JsonSchema)]
pub struct StudyDocumentPath {
    pub study_id: StudioStudyId,
    pub document: DocumentPath,
}

const fn default_frame_limit() -> u64 {
    100
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ErrorResponse {
    pub error: String,
}

#[derive(Debug)]
struct StudyRecord {
    summary: RwLock<StudySummary>,
    published: RwLock<Option<Arc<Study>>>,
    history: RwLock<Vec<StudioEvent>>,
    events: broadcast::Sender<StudioEvent>,
    control: Arc<StudyControl>,
    generation: AtomicU64,
    next_sequence: AtomicU64,
}

enum WorkerUpdate {
    Progress(Progress),
    Observation(SearchObservationView),
    Frame {
        document: DocumentPath,
        frame: Box<axionomy_view::ExchangeFrame>,
    },
}

struct ChannelObserver {
    updates: mpsc::Sender<WorkerUpdate>,
}

impl StudyObserver for ChannelObserver {
    fn progress(&mut self, progress: Progress) {
        let _ = self.updates.blocking_send(WorkerUpdate::Progress(progress));
    }

    fn observation(&mut self, observation: SearchObservationView) {
        let _ = self
            .updates
            .blocking_send(WorkerUpdate::Observation(observation));
    }

    fn frame(&mut self, document: &DocumentPath, frame: axionomy_view::ExchangeFrame) {
        let _ = self.updates.blocking_send(WorkerUpdate::Frame {
            document: document.clone(),
            frame: Box::new(frame),
        });
    }
}

impl StudyRecord {
    fn sequence(&self) -> u64 {
        self.next_sequence.fetch_add(1, Ordering::Relaxed)
    }

    async fn emit(&self, event: StudioEvent) {
        self.summary.write().await.last_event = Some(event.clone());
        self.history.write().await.push(event.clone());
        let _ = self.events.send(event);
    }
}

#[derive(Clone)]
pub struct StudioState {
    registry: Arc<Registry>,
    studies: Arc<RwLock<HashMap<StudioStudyId, Arc<StudyRecord>>>>,
    next_study: Arc<AtomicU64>,
    /// The engine whose store's refs the API creates, moves and follows.
    engine: Engine,
}

impl StudioState {
    /// A Studio server over `engine`, whose store holds the points and refs
    /// it serves, studying the problems of `registry`, with no studies yet.
    pub fn new(engine: Engine, registry: Registry) -> Self {
        Self {
            registry: Arc::new(registry),
            studies: Arc::new(RwLock::new(HashMap::new())),
            next_study: Arc::new(AtomicU64::new(1)),
            engine,
        }
    }
}

#[derive(Debug)]
enum ApiError {
    Status {
        status: StatusCode,
        message: String,
    },
    /// A ref was not where a move expected it: 409 with where it is.
    Moved(RefMoved),
}

impl ApiError {
    fn not_found(message: impl Into<String>) -> Self {
        Self::Status {
            status: StatusCode::NOT_FOUND,
            message: message.into(),
        }
    }

    fn bad_request(message: impl Into<String>) -> Self {
        Self::Status {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
        }
    }

    fn internal(message: impl Into<String>) -> Self {
        Self::Status {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: message.into(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        match self {
            Self::Status { status, message } => {
                (status, Json(ErrorResponse { error: message })).into_response()
            }
            Self::Moved(moved) => (StatusCode::CONFLICT, Json(moved)).into_response(),
        }
    }
}

/// Every route, declared once with its handler and contract.
fn endpoints() -> Vec<Endpoint<StudioState>> {
    vec![
        Endpoint::new::<HealthResponse>("get", "/api/health", get(health)).describe(
            "getHealth",
            "Check whether the native Studio engine is reachable",
            "system",
        ),
        Endpoint::new::<ProblemList>("get", "/api/problems", get(list_problems)).describe(
            "listProblems",
            "List problems, strategies, and capabilities",
            "problems",
        ),
        Endpoint::new::<StudySummary>("post", "/api/studies", post(create_study))
            .describe(
                "createStudy",
                "Start a reproducible, cooperatively controlled study",
                "studies",
            )
            .json_body::<StudyRequest>()
            .fallible(),
        Endpoint::new::<StudySummary>("get", "/api/studies/{study_id}", get(get_study))
            .describe("getStudy", "Inspect a study's state", "studies")
            .path_parameters::<StudyPath>()
            .fallible(),
        Endpoint::new::<StudySummary>("delete", "/api/studies/{study_id}", delete(cancel_study))
            .describe("cancelStudy", "Cancel a study", "studies")
            .path_parameters::<StudyPath>()
            .fallible(),
        Endpoint::new::<StudySummary>("post", "/api/studies/{study_id}/pause", post(pause_study))
            .describe("pauseStudy", "Pause a study", "studies")
            .path_parameters::<StudyPath>()
            .fallible(),
        Endpoint::new::<StudySummary>("post", "/api/studies/{study_id}/resume", post(resume_study))
            .describe("resumeStudy", "Resume a paused study", "studies")
            .path_parameters::<StudyPath>()
            .fallible(),
        Endpoint::new::<StudioEvent>("get", "/api/studies/{study_id}/events", get(study_events))
            .describe(
                "streamStudyEvents",
                "Follow resumable ordered lifecycle events over SSE",
                "studies",
            )
            .path_parameters::<StudyPath>()
            .event_stream::<StudioEvent>()
            .fallible(),
        Endpoint::new::<Study>("get", "/api/studies/{study_id}/study", get(get_published))
            .describe(
                "getPublishedStudy",
                "Load the complete portable study once it is published",
                "studies",
            )
            .path_parameters::<StudyPath>()
            .fallible(),
        Endpoint::new::<ViewDocument>(
            "get",
            "/api/studies/{study_id}/study/documents/{document}",
            get(get_document),
        )
        .describe(
            "getViewDocument",
            "Load one replay-derived alternative",
            "studies",
        )
        .path_parameters::<StudyDocumentPath>()
        .fallible(),
        Endpoint::new::<FramePage>(
            "get",
            "/api/studies/{study_id}/study/documents/{document}/frames",
            get(get_frames),
        )
        .describe(
            "getTraceFrames",
            "Page through replay-derived exchange frames",
            "studies",
        )
        .path_parameters::<StudyDocumentPath>()
        .query::<FrameQuery>()
        .fallible(),
        Endpoint::new::<RefView>("post", "/api/refs", post(refs::create))
            .describe("createRef", "Name a point with a new ref", "refs")
            .json_body::<RefCreateRequest>()
            .conflict::<RefMoved>("A ref has the name already; it names `current`")
            .fallible(),
        Endpoint::new::<RefView>("get", "/api/refs/{name}", get(refs::read))
            .describe("getRef", "Read the point a ref names", "refs")
            .path_parameters::<RefName>()
            .fallible(),
        Endpoint::new::<RefView>("post", "/api/refs/{name}/advance", post(refs::advance))
            .describe(
                "advanceRef",
                "Move a ref forward from the point its mover saw it at",
                "refs",
            )
            .path_parameters::<RefName>()
            .json_body::<RefAdvanceRequest>()
            .conflict::<RefMoved>(
                "The ref is not at `expected`: another move came first, and it is at `current`",
            )
            .fallible(),
        Endpoint::new::<RefFrames>("get", "/api/refs/{name}/frames", get(refs::frames))
            .describe(
                "listRefFrames",
                "Follow the steps a ref was moved over after a point, waiting for the next",
                "refs",
            )
            .path_parameters::<RefName>()
            .query::<RefFramesQuery>()
            .fallible(),
    ]
}

/// Builds both the Axum router and its OpenAPI 3.1 contract from one endpoint
/// table, so a route cannot exist without its contract or the reverse.
pub fn api(state: StudioState) -> (Router, OpenApiDocument) {
    let endpoints = endpoints();
    let document = openapi::document(&endpoints);
    let routes = endpoints
        .into_iter()
        .fold(Router::new(), |router, endpoint| {
            let (path, route) = endpoint.into_route();
            router.route(path, route)
        })
        .with_state(state);
    let served = document.clone();
    let router = routes
        .route(
            "/api/openapi.json",
            get(move || std::future::ready(Json(served.clone()))),
        )
        .layer(CorsLayer::new().allow_origin(HeaderValue::from_static("*")))
        .layer(TraceLayer::new_for_http());
    (router, document)
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

async fn list_problems(State(state): State<StudioState>) -> Json<ProblemList> {
    Json(ProblemList {
        problems: state.registry.catalog(),
    })
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".into(),
        engine: "native".into(),
    })
}

async fn create_study(
    State(state): State<StudioState>,
    Json(request): Json<StudyRequest>,
) -> Result<Json<StudySummary>, ApiError> {
    let key = validate_request(&state.registry, &request)?;
    let study_id = StudioStudyId::new(state.next_study.fetch_add(1, Ordering::Relaxed));
    let (events, _) = broadcast::channel(256);
    let summary = StudySummary {
        id: study_id,
        request: request.clone(),
        status: StudyStatus::Studying,
        completed: 0,
        total: None,
        key: None,
        selected_document: None,
        message: Some("study queued".into()),
        last_event: None,
    };
    let record = Arc::new(StudyRecord {
        summary: RwLock::new(summary.clone()),
        published: RwLock::new(None),
        history: RwLock::new(Vec::new()),
        events,
        control: Arc::new(StudyControl::default()),
        generation: AtomicU64::new(1),
        next_sequence: AtomicU64::new(0),
    });
    state
        .studies
        .write()
        .await
        .insert(study_id, Arc::clone(&record));
    tokio::spawn(execute_study(
        Arc::clone(&state.registry),
        record,
        study_id,
        request,
        key,
        1,
    ));
    Ok(Json(summary))
}

/// Decides the request's instance and strategy, refusing what the problem
/// does not offer.
fn validate_request(registry: &Registry, request: &StudyRequest) -> Result<StudyKey, ApiError> {
    if request.budget == 0 {
        return Err(ApiError::bad_request("budget must be greater than zero"));
    }
    registry
        .resolve(request)
        .map_err(|error| ApiError::bad_request(error.to_string()))
}

/// Studies the request on a worker thread, relaying its progress, until the
/// study finishes or a cancellation moves it past `generation`.
async fn execute_study(
    registry: Arc<Registry>,
    record: Arc<StudyRecord>,
    study_id: StudioStudyId,
    request: StudyRequest,
    key: StudyKey,
    generation: u64,
) {
    record
        .emit(StudioEvent::StudyStarted {
            study_id,
            sequence: record.sequence(),
            problem: key.problem,
            strategy: key.strategy,
        })
        .await;

    let (updates_tx, mut updates_rx) = mpsc::channel::<WorkerUpdate>(64);
    let worker_control = Arc::clone(&record.control);
    let worker_request = request.clone();
    let worker = tokio::task::spawn_blocking(move || {
        let mut observer = ChannelObserver {
            updates: updates_tx,
        };
        registry.study_with(&worker_request, &worker_control, &mut observer)
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
                        study_id,
                        sequence: record.sequence(),
                        phase: progress.phase,
                        completed: progress.completed,
                        total: Some(progress.total),
                        message: progress.message,
                    })
                    .await;
            }
            WorkerUpdate::Observation(observation) => {
                record
                    .emit(StudioEvent::Thinking {
                        study_id,
                        sequence: record.sequence(),
                        observation,
                    })
                    .await;
            }
            WorkerUpdate::Frame { document, frame } => {
                let frame_index = frame.index;
                record
                    .emit(StudioEvent::FrameAppended {
                        study_id,
                        sequence: record.sequence(),
                        document,
                        frame_index,
                        frame,
                    })
                    .await;
            }
        }
    }

    let result = match worker.await {
        Ok(result) => result,
        Err(error) => Err(StudyError::Problem {
            problem: request.problem.clone(),
            message: format!("worker failed: {error}"),
        }),
    };
    if record.generation.load(Ordering::Acquire) != generation {
        return;
    }
    match result {
        Ok(study) => publish_study(&record, study_id, study).await,
        Err(StudyError::Cancelled) => finish_cancelled(&record, study_id).await,
        Err(StudyError::Paused) => {}
        Err(error) => finish_failed(&record, study_id, error.to_string()).await,
    }
}

async fn publish_study(record: &StudyRecord, study_id: StudioStudyId, study: Study) {
    let key = study.key.clone();
    let document = study.selected_document.clone();
    let documents = study.documents.len() as u64;
    let total = study
        .documents
        .iter()
        .map(|document| document.frames.len() as u64)
        .sum();
    *record.published.write().await = Some(Arc::new(study));
    record
        .emit(StudioEvent::StudyPublished {
            study_id,
            sequence: record.sequence(),
            key: key.clone(),
            documents,
        })
        .await;
    {
        let mut summary = record.summary.write().await;
        summary.status = StudyStatus::Completed;
        summary.completed = total;
        summary.total = Some(total);
        summary.key = Some(key);
        summary.selected_document = Some(document.clone());
        summary.message = Some("every replay verified".into());
    }
    record
        .emit(StudioEvent::StudyCompleted {
            study_id,
            sequence: record.sequence(),
            document,
        })
        .await;
}

async fn finish_cancelled(record: &StudyRecord, study_id: StudioStudyId) {
    {
        let mut summary = record.summary.write().await;
        summary.status = StudyStatus::Cancelled;
        summary.message = Some("cancelled by caller".into());
    }
    record
        .emit(StudioEvent::StudyCancelled {
            study_id,
            sequence: record.sequence(),
        })
        .await;
}

async fn finish_failed(record: &StudyRecord, study_id: StudioStudyId, message: String) {
    {
        let mut summary = record.summary.write().await;
        summary.status = StudyStatus::Failed;
        summary.message = Some(message.clone());
    }
    record
        .emit(StudioEvent::StudyFailed {
            study_id,
            sequence: record.sequence(),
            message,
        })
        .await;
}

async fn get_study(
    State(state): State<StudioState>,
    Path(path): Path<StudyPath>,
) -> Result<Json<StudySummary>, ApiError> {
    let record = find_study(&state, path.study_id).await?;
    let summary = record.summary.read().await.clone();
    Ok(Json(summary))
}

async fn cancel_study(
    State(state): State<StudioState>,
    Path(path): Path<StudyPath>,
) -> Result<Json<StudySummary>, ApiError> {
    let record = find_study(&state, path.study_id).await?;
    record.generation.fetch_add(1, Ordering::AcqRel);
    record.control.cancel();
    finish_cancelled(&record, path.study_id).await;
    Ok(Json(record.summary.read().await.clone()))
}

async fn pause_study(
    State(state): State<StudioState>,
    Path(path): Path<StudyPath>,
) -> Result<Json<StudySummary>, ApiError> {
    let record = find_study(&state, path.study_id).await?;
    {
        let summary = record.summary.read().await;
        if summary.status != StudyStatus::Studying {
            return Err(ApiError::bad_request(
                "only a study in progress can be paused",
            ));
        }
    }
    record.control.pause();
    {
        let mut summary = record.summary.write().await;
        summary.status = StudyStatus::Paused;
        summary.message = Some("paused by caller".into());
    }
    record
        .emit(StudioEvent::StudyPaused {
            study_id: path.study_id,
            sequence: record.sequence(),
        })
        .await;
    Ok(Json(record.summary.read().await.clone()))
}

async fn resume_study(
    State(state): State<StudioState>,
    Path(path): Path<StudyPath>,
) -> Result<Json<StudySummary>, ApiError> {
    let record = find_study(&state, path.study_id).await?;
    {
        let mut summary = record.summary.write().await;
        if summary.status != StudyStatus::Paused {
            return Err(ApiError::bad_request("only a paused study can be resumed"));
        }
        summary.status = StudyStatus::Studying;
        summary.message = Some("resumed by caller".into());
    }
    record.control.resume();
    record
        .emit(StudioEvent::StudyResumed {
            study_id: path.study_id,
            sequence: record.sequence(),
        })
        .await;
    drop(state);
    Ok(Json(record.summary.read().await.clone()))
}

async fn find_study(
    state: &StudioState,
    study_id: StudioStudyId,
) -> Result<Arc<StudyRecord>, ApiError> {
    state
        .studies
        .read()
        .await
        .get(&study_id)
        .cloned()
        .ok_or_else(|| ApiError::not_found(format!("study `{study_id}` was not found")))
}

async fn find_published(
    state: &StudioState,
    study_id: StudioStudyId,
) -> Result<Arc<Study>, ApiError> {
    find_study(state, study_id)
        .await?
        .published
        .read()
        .await
        .clone()
        .ok_or_else(|| ApiError::not_found(format!("study `{study_id}` has not been published")))
}

async fn find_document(
    state: &StudioState,
    path: &StudyDocumentPath,
) -> Result<(Arc<Study>, usize), ApiError> {
    let study = find_published(state, path.study_id).await?;
    let index = study
        .documents
        .iter()
        .position(|document| document.id == path.document)
        .ok_or_else(|| {
            ApiError::not_found(format!("document `{}` was not found", path.document))
        })?;
    Ok((study, index))
}

async fn get_published(
    State(state): State<StudioState>,
    Path(path): Path<StudyPath>,
) -> Result<Json<Study>, ApiError> {
    Ok(Json(
        (*find_published(&state, path.study_id).await?).clone(),
    ))
}

async fn get_document(
    State(state): State<StudioState>,
    Path(path): Path<StudyDocumentPath>,
) -> Result<Json<ViewDocument>, ApiError> {
    let (study, index) = find_document(&state, &path).await?;
    Ok(Json(study.documents[index].clone()))
}

async fn get_frames(
    State(state): State<StudioState>,
    Path(path): Path<StudyDocumentPath>,
    Query(query): Query<FrameQuery>,
) -> Result<Json<FramePage>, ApiError> {
    if query.limit == 0 || query.limit > 1_000 {
        return Err(ApiError::bad_request(
            "frame limit must be between 1 and 1000",
        ));
    }
    let (study, index) = find_document(&state, &path).await?;
    let frames = &study.documents[index].frames;
    let total = frames.len() as u64;
    let from = query.from.min(total);
    let end = from.saturating_add(query.limit).min(total);
    Ok(Json(FramePage {
        study_id: path.study_id,
        document: path.document,
        from,
        total,
        frames: frames[from as usize..end as usize].to_vec(),
    }))
}

async fn study_events(
    State(state): State<StudioState>,
    Path(path): Path<StudyPath>,
) -> Result<
    Sse<impl futures_util::Stream<Item = Result<Event, Infallible>> + Send + 'static>,
    ApiError,
> {
    let record = find_study(&state, path.study_id).await?;
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
            StudioEvent::StudyStarted { .. } => "study_started",
            StudioEvent::Progress { .. } => "progress",
            StudioEvent::Thinking { .. } => "thinking",
            StudioEvent::FrameAppended { .. } => "frame_appended",
            StudioEvent::StudyPublished { .. } => "study_published",
            StudioEvent::StudyPaused { .. } => "study_paused",
            StudioEvent::StudyResumed { .. } => "study_resumed",
            StudioEvent::StudyCompleted { .. } => "study_completed",
            StudioEvent::StudyCancelled { .. } => "study_cancelled",
            StudioEvent::StudyFailed { .. } => "study_failed",
        };
        Ok(Event::default()
            .event(kind)
            .json_data(event)
            .expect("StudioEvent always serializes"))
    });
    Ok(Sse::new(stream))
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

    fn state() -> StudioState {
        let engine = Engine::new(axionomy_store::Store::memory().unwrap()).unwrap();
        StudioState::new(engine, axionomy_problems::registry())
    }

    #[tokio::test]
    async fn a_registry_study_is_identical_over_http() {
        let (app, _) = api(state());
        let request =
            StudyRequest::new("maze".parse().unwrap()).with_strategy("a-star".parse().unwrap());
        let create = app
            .clone()
            .oneshot(
                Request::post("/api/studies")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&request).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(create.status(), StatusCode::OK);
        let created: StudySummary = response_json(create).await;
        let completed = loop {
            let response = app
                .clone()
                .oneshot(
                    Request::get(format!("/api/studies/{}", created.id))
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            let summary: StudySummary = response_json(response).await;
            if summary.status != StudyStatus::Studying {
                break summary;
            }
            tokio::task::yield_now().await;
        };
        assert_eq!(completed.status, StudyStatus::Completed);
        let response = app
            .oneshot(
                Request::get(format!("/api/studies/{}/study", created.id))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let http_study: Study = response_json(response).await;
        let registry_study = axionomy_problems::registry().study(request).unwrap();
        assert_eq!(http_study, registry_study);
    }

    #[tokio::test]
    async fn catalog_exposes_all_problems() {
        let (app, _) = api(state());
        let response = app
            .oneshot(Request::get("/api/problems").body(Body::empty()).unwrap())
            .await
            .unwrap();
        let catalog: ProblemList = response_json(response).await;
        assert_eq!(catalog.problems.len(), 15);
    }

    #[test]
    fn openapi_contains_full_service_surface() {
        let (_, openapi) = api(state());
        let value = serde_json::to_value(openapi).unwrap();
        let paths = value["paths"].as_object().unwrap();
        for path in [
            "/api/problems",
            "/api/studies",
            "/api/studies/{study_id}",
            "/api/studies/{study_id}/pause",
            "/api/studies/{study_id}/resume",
            "/api/studies/{study_id}/events",
            "/api/studies/{study_id}/study",
            "/api/studies/{study_id}/study/documents/{document}",
            "/api/studies/{study_id}/study/documents/{document}/frames",
            "/api/refs",
            "/api/refs/{name}",
            "/api/refs/{name}/advance",
            "/api/refs/{name}/frames",
        ] {
            assert!(paths.contains_key(path), "missing {path}");
        }
        // A move that finds the ref elsewhere says where it is.
        assert_eq!(
            paths["/api/refs/{name}/advance"]["post"]["responses"]["409"]["content"]["application/json"]
                ["schema"]["$ref"],
            "#/components/schemas/RefMoved"
        );
        let schemas = value["components"]["schemas"].as_object().unwrap();
        assert!(schemas.contains_key("StudioEvent"));
        assert!(schemas.contains_key("Study"));
        assert!(schemas.contains_key("PointId"));
    }
}
