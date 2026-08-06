//! Native HTTP reference server for Axionomy Studio.

use aide::{
    IntoApi, UseApi,
    axum::{ApiRouter, routing},
    openapi::{Info, OpenApi},
};
use axionomy_problems::maze_view::{self, MazeStrategy};
use axionomy_view::{ExchangeFrame, StudioEvent, ViewDocument};
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
use tokio::sync::{RwLock, broadcast};
use tokio_stream::wrappers::BroadcastStream;
use tokio_util::sync::CancellationToken;
use tower_http::{
    cors::CorsLayer,
    services::{ServeDir, ServeFile},
    trace::TraceLayer,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ExampleSummary {
    pub key: String,
    pub title: String,
    pub description: String,
    pub domain: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ExampleList {
    pub examples: Vec<ExampleSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateRunRequest {
    pub example: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Running,
    Completed,
    Cancelled,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct RunSummary {
    pub id: String,
    pub example: String,
    pub status: RunStatus,
    pub completed: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// Carries the tagged event union into the generated browser contract and
    /// also lets reconnecting clients inspect the latest lifecycle event.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_event: Option<StudioEvent>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct FramePage {
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
pub struct DocumentPath {
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
    cancellation: CancellationToken,
}

impl RunRecord {
    async fn emit(&self, event: StudioEvent) {
        self.summary.write().await.last_event = Some(event.clone());
        self.history.write().await.push(event.clone());
        let _ = self.events.send(event);
    }
}

#[derive(Debug, Clone)]
pub struct StudioState {
    runs: Arc<RwLock<HashMap<String, Arc<RunRecord>>>>,
    documents: Arc<RwLock<HashMap<String, Arc<ViewDocument>>>>,
    next_run: Arc<AtomicU64>,
}

impl Default for StudioState {
    fn default() -> Self {
        Self {
            runs: Arc::new(RwLock::new(HashMap::new())),
            documents: Arc::new(RwLock::new(HashMap::new())),
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

pub fn example_catalog() -> Vec<ExampleSummary> {
    MazeStrategy::ALL
        .into_iter()
        .map(|strategy| ExampleSummary {
            key: strategy.key().into(),
            title: strategy.label().into(),
            description: strategy.description().into(),
            domain: "maze".into(),
        })
        .collect()
}

/// Builds both the Axum router and its OpenAPI 3.1 contract from the same Rust
/// handlers and Schemars types.
pub fn api(state: StudioState) -> (Router, OpenApi) {
    let mut openapi = OpenApi {
        info: Info {
            title: "Axionomy Studio API".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            description: Some(
                "Replay-derived economic inspection, run lifecycle, and visualization data.".into(),
            ),
            ..Info::default()
        },
        ..OpenApi::default()
    };

    let api = ApiRouter::new()
        .api_route(
            "/api/examples",
            routing::get_with(list_examples, |operation| {
                operation
                    .id("listExamples")
                    .summary("List runnable reference examples")
                    .tag("examples")
            }),
        )
        .api_route(
            "/api/runs",
            routing::post_with(create_run, |operation| {
                operation
                    .id("createRun")
                    .summary("Start an interruptible example run")
                    .tag("runs")
            }),
        )
        .api_route(
            "/api/runs/{run_id}",
            routing::get_with(get_run, |operation| {
                operation
                    .id("getRun")
                    .summary("Inspect run lifecycle and latest event")
                    .tag("runs")
            })
            .delete_with(cancel_run, |operation| {
                operation
                    .id("cancelRun")
                    .summary("Request cooperative cancellation")
                    .tag("runs")
            }),
        )
        .api_route(
            "/api/runs/{run_id}/events",
            routing::get_with(run_events, |operation| {
                operation
                    .id("streamRunEvents")
                    .summary("Follow ordered lifecycle events over SSE")
                    .description(
                        "Each named Server-Sent Event carries one tagged StudioEvent JSON value. Sequence numbers are transport order, not economic time.",
                    )
                    .tag("runs")
            }),
        )
        .api_route(
            "/api/traces/{document_id}",
            routing::get_with(get_document, |operation| {
                operation
                    .id("getViewDocument")
                    .summary("Load a portable replay-derived document")
                    .tag("traces")
            }),
        )
        .api_route(
            "/api/traces/{document_id}/frames",
            routing::get_with(get_frames, |operation| {
                operation
                    .id("getTraceFrames")
                    .summary("Page through replay-derived exchange frames")
                    .tag("traces")
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

/// Adds a static Vite build with SPA fallback when the directory exists.
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

async fn list_examples() -> Json<ExampleList> {
    Json(ExampleList {
        examples: example_catalog(),
    })
}

async fn create_run(
    State(state): State<StudioState>,
    Json(request): Json<CreateRunRequest>,
) -> Result<Json<RunSummary>, ApiError> {
    let Some(strategy) = MazeStrategy::from_key(&request.example) else {
        return Err(ApiError::bad_request(format!(
            "unknown example `{}`",
            request.example
        )));
    };
    let run_id = format!("run-{}", state.next_run.fetch_add(1, Ordering::Relaxed));
    let (events, _) = broadcast::channel(128);
    let summary = RunSummary {
        id: run_id.clone(),
        example: request.example,
        status: RunStatus::Running,
        completed: 0,
        total: None,
        document_id: None,
        message: Some("search queued".into()),
        last_event: None,
    };
    let record = Arc::new(RunRecord {
        summary: RwLock::new(summary.clone()),
        history: RwLock::new(Vec::new()),
        events,
        cancellation: CancellationToken::new(),
    });
    state
        .runs
        .write()
        .await
        .insert(run_id.clone(), Arc::clone(&record));

    tokio::spawn(execute_run(state, Arc::clone(&record), run_id, strategy));
    Ok(Json(summary))
}

async fn execute_run(
    state: StudioState,
    record: Arc<RunRecord>,
    run_id: String,
    strategy: MazeStrategy,
) {
    let mut sequence = 0;
    record
        .emit(StudioEvent::RunStarted {
            run_id: run_id.clone(),
            sequence,
            example: strategy.key().into(),
        })
        .await;
    sequence += 1;
    record
        .emit(StudioEvent::Progress {
            run_id: run_id.clone(),
            sequence,
            completed: 0,
            total: None,
            message: "solving encoded economy".into(),
        })
        .await;
    sequence += 1;
    tokio::task::yield_now().await;

    if record.cancellation.is_cancelled() {
        finish_cancelled(&record, &run_id, sequence).await;
        return;
    }

    let mut document = match maze_view::document(strategy) {
        Ok(document) => document,
        Err(error) => {
            let message = error.to_string();
            let event = StudioEvent::RunFailed {
                run_id: run_id.clone(),
                sequence,
                message: message.clone(),
            };
            {
                let mut summary = record.summary.write().await;
                summary.status = RunStatus::Failed;
                summary.message = Some(message);
            }
            record.emit(event).await;
            return;
        }
    };
    document.id.clone_from(&run_id);
    let total = document.frames.len() as u64;
    {
        let mut summary = record.summary.write().await;
        summary.total = Some(total);
        summary.message = Some("replaying accepted exchanges".into());
    }

    for frame in &document.frames {
        if record.cancellation.is_cancelled() {
            finish_cancelled(&record, &run_id, sequence).await;
            return;
        }
        record
            .emit(StudioEvent::FrameAppended {
                run_id: run_id.clone(),
                sequence,
                frame_index: frame.index,
            })
            .await;
        sequence += 1;
        {
            let mut summary = record.summary.write().await;
            summary.completed = frame.index + 1;
        }
        tokio::task::yield_now().await;
    }

    state
        .documents
        .write()
        .await
        .insert(run_id.clone(), Arc::new(document));
    {
        let mut summary = record.summary.write().await;
        summary.status = RunStatus::Completed;
        summary.document_id = Some(run_id.clone());
        summary.message = Some("replay verified".into());
    }
    record
        .emit(StudioEvent::RunCompleted {
            run_id: run_id.clone(),
            sequence,
            document_id: run_id,
        })
        .await;
}

async fn finish_cancelled(record: &RunRecord, run_id: &str, sequence: u64) {
    {
        let mut summary = record.summary.write().await;
        summary.status = RunStatus::Cancelled;
        summary.message = Some("cancelled by caller".into());
    }
    record
        .emit(StudioEvent::RunCancelled {
            run_id: run_id.into(),
            sequence,
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
    record.cancellation.cancel();
    tokio::task::yield_now().await;
    let summary = record.summary.read().await.clone();
    Ok(Json(summary))
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

async fn get_document(
    State(state): State<StudioState>,
    Path(path): Path<DocumentPath>,
) -> Result<Json<ViewDocument>, ApiError> {
    let document_id = path.document_id;
    let document = state
        .documents
        .read()
        .await
        .get(&document_id)
        .cloned()
        .ok_or_else(|| ApiError::not_found(format!("document `{document_id}` was not found")))?;
    Ok(Json((*document).clone()))
}

async fn get_frames(
    State(state): State<StudioState>,
    Path(path): Path<DocumentPath>,
    Query(query): Query<FrameQuery>,
) -> Result<Json<FramePage>, ApiError> {
    let document_id = path.document_id;
    if query.limit == 0 || query.limit > 1_000 {
        return Err(ApiError::bad_request(
            "frame limit must be between 1 and 1000",
        ));
    }
    let document = state
        .documents
        .read()
        .await
        .get(&document_id)
        .cloned()
        .ok_or_else(|| ApiError::not_found(format!("document `{document_id}` was not found")))?;
    let total = document.frames.len() as u64;
    let from = query.from.min(total);
    let end = from.saturating_add(query.limit).min(total);
    Ok(Json(FramePage {
        document_id,
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
    let history = record.history.read().await.clone();
    let live = BroadcastStream::new(record.events.subscribe())
        .filter_map(|event| async move { event.ok() });
    let stream = tokio_stream::iter(history).chain(live).map(|event| {
        let kind = match &event {
            StudioEvent::RunStarted { .. } => "run_started",
            StudioEvent::Progress { .. } => "progress",
            StudioEvent::FrameAppended { .. } => "frame_appended",
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
    async fn run_completes_and_serves_replay_document_and_frames() {
        let (app, _) = api(StudioState::default());
        let create = app
            .clone()
            .oneshot(
                Request::post("/api/runs")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"example":"maze_pareto_energy"}"#))
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
        assert!(matches!(
            completed.last_event,
            Some(StudioEvent::RunCompleted { .. })
        ));

        let document_id = completed.document_id.unwrap();
        let document_response = app
            .clone()
            .oneshot(
                Request::get(format!("/api/traces/{document_id}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let document: ViewDocument = response_json(document_response).await;
        assert_eq!(document.id, document_id);
        assert!(!document.frames.is_empty());

        let page_response = app
            .oneshot(
                Request::get(format!("/api/traces/{document_id}/frames?from=1&limit=2"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let page: FramePage = response_json(page_response).await;
        assert_eq!(page.from, 1);
        assert!(page.frames.len() <= 2);
    }

    #[tokio::test]
    async fn unknown_examples_are_rejected() {
        let (app, _) = api(StudioState::default());
        let response = app
            .oneshot(
                Request::post("/api/runs")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"example":"outside-the-economy"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn openapi_contains_each_json_contract_route_and_tagged_events() {
        let (_, openapi) = api(StudioState::default());
        let value = serde_json::to_value(openapi).unwrap();
        let paths = value["paths"].as_object().unwrap();
        for path in [
            "/api/examples",
            "/api/runs",
            "/api/runs/{run_id}",
            "/api/runs/{run_id}/events",
            "/api/traces/{document_id}",
            "/api/traces/{document_id}/frames",
        ] {
            assert!(paths.contains_key(path), "missing {path}");
        }
        let schemas = value["components"]["schemas"].as_object().unwrap();
        assert!(schemas.contains_key("StudioEvent"));
        assert!(schemas.contains_key("ViewDocument"));
    }
}
