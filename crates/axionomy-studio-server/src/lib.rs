//! Native HTTP adapter for the engine's operations Studio uses (P12): its
//! packages, points, refs, strategies and experiments, as
//! `axionomy-studio-api` defines them.

mod engine;
mod openapi;
mod refs;

pub use axionomy_studio_api::{
    ErrorResponse, ObjectiveList, PointId, PointIdError, RefAdvanceRequest, RefCreateRequest,
    RefDeleteQuery, RefFrame, RefFrames, RefFramesQuery, RefMoved, RefView,
};
pub use engine::{ExperimentName, PointName};
pub use openapi::OpenApiDocument;
pub use refs::RefName;

use axionomy_engine::Engine;
use axionomy_studio_api::{
    Ancestor, AncestorQuery, Applied, ApplyRequest, AssessRequest, Balances, Candidate, Candidates,
    CandidatesQuery, CompareRequest, ComparisonView, ExperimentEvent, LoadedView, PackageList,
    Refusal, RefusalBody, ResultsView, SamplePackages, StrategyEvent, StrategyRequest,
};
use axionomy_view::ViewDocument;
use axum::{
    Json, Router,
    extract::DefaultBodyLimit,
    http::{HeaderValue, StatusCode},
    response::IntoResponse,
    routing::{delete, get, post},
};
use openapi::Endpoint;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tower_http::{
    cors::CorsLayer,
    services::{ServeDir, ServeFile},
    trace::TraceLayer,
};

/// A deliberately small liveness contract. A successful response proves that
/// the native engine adapter is reachable now.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct HealthResponse {
    pub status: String,
    pub engine: String,
}

#[derive(Clone)]
pub struct StudioState {
    /// The engine whose store's packages, points and refs the API serves.
    engine: Engine,
}

impl StudioState {
    /// A Studio server over `engine`, whose store holds the packages,
    /// points and refs it serves.
    pub fn new(engine: Engine) -> Self {
        Self { engine }
    }
}

/// The largest package file the server loads.
const MAX_PACKAGE_BYTES: usize = 256 * 1024 * 1024;

/// A refused request, answered with its status and body.
#[derive(Debug)]
struct ApiError(Refusal);

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let status =
            StatusCode::from_u16(self.0.status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        match self.0.body() {
            RefusalBody::Moved(moved) => (status, Json(moved)).into_response(),
            RefusalBody::Error(error) => (status, Json(error)).into_response(),
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
        Endpoint::new::<PackageList>("get", "/api/packages", get(engine::packages))
            .describe(
                "listPackages",
                "List the packages the store holds: their refs, experiments, strategies, objectives, goals and candidate sets",
                "packages",
            )
            .fallible(),
        Endpoint::new::<LoadedView>(
            "post",
            "/api/packages",
            post(engine::load).layer(DefaultBodyLimit::max(MAX_PACKAGE_BYTES)),
        )
        .describe(
            "loadPackage",
            "Load a package file into the store, its manifests checked first",
            "packages",
        )
        .bytes_body("A package file: a store file, or a zip holding one")
        .fallible(),
        Endpoint::new::<ViewDocument>(
            "get",
            "/api/points/{point}/document",
            get(engine::draw),
        )
        .describe(
            "drawPoint",
            "Draw a point's history with the view of its run's ontology",
            "points",
        )
        .path_parameters::<PointName>()
        .fallible(),
        Endpoint::new::<Ancestor>("get", "/api/points/{point}/ancestor", get(engine::ancestor))
            .describe(
                "getAncestor",
                "Name the point at an earlier step of a point's history",
                "points",
            )
            .path_parameters::<PointName>()
            .query::<AncestorQuery>()
            .fallible(),
        Endpoint::new::<Balances>("get", "/api/points/{point}/balances", get(engine::balances))
            .describe(
                "getBalances",
                "List a point's accounts and what each holds there",
                "points",
            )
            .path_parameters::<PointName>()
            .fallible(),
        Endpoint::new::<Candidates>(
            "get",
            "/api/points/{point}/candidates",
            get(engine::candidates),
        )
        .describe(
            "listCandidates",
            "List the exchanges a candidate set offers at a point, each assessed there",
            "points",
        )
        .path_parameters::<PointName>()
        .query::<CandidatesQuery>()
        .fallible(),
        Endpoint::new::<Candidate>("post", "/api/points/{point}/assess", post(engine::assess))
            .describe(
                "assessExchange",
                "Assess an exchange at a point without recording anything",
                "points",
            )
            .path_parameters::<PointName>()
            .json_body::<AssessRequest>()
            .fallible(),
        Endpoint::new::<Applied>("post", "/api/points/{point}/apply", post(engine::apply))
            .describe(
                "applyExchanges",
                "Apply exchanges at a point, all of them or none, reaching a new point",
                "points",
            )
            .path_parameters::<PointName>()
            .json_body::<ApplyRequest>()
            .fallible(),
        Endpoint::new::<StrategyEvent>(
            "post",
            "/api/points/{point}/run",
            post(engine::run_strategy),
        )
        .describe(
            "runStrategy",
            "Run a strategy from a point, streaming its progress; closing the stream stops it",
            "points",
        )
        .path_parameters::<PointName>()
        .json_body::<StrategyRequest>()
        .event_stream::<StrategyEvent>()
        .fallible(),
        Endpoint::new::<ResultsView>(
            "get",
            "/api/experiments/{experiment}/results",
            get(engine::results),
        )
        .describe(
            "getResults",
            "Read the results an experiment last recorded",
            "experiments",
        )
        .path_parameters::<ExperimentName>()
        .fallible(),
        Endpoint::new::<ExperimentEvent>(
            "post",
            "/api/experiments/{experiment}/run",
            post(engine::run_experiment),
        )
        .describe(
            "runExperiment",
            "Run an experiment's trials, streaming their progress; closing the stream stops it and keeps no results",
            "experiments",
        )
        .path_parameters::<ExperimentName>()
        .event_stream::<ExperimentEvent>()
        .fallible(),
        Endpoint::new::<ComparisonView>("post", "/api/compare", post(engine::compare))
            .describe(
                "comparePoints",
                "Compare points on objectives: their values, the front and the best in order",
                "experiments",
            )
            .json_body::<CompareRequest>()
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
            .forbidden("The ref is one a package ships, which is fixed: name a ref of your own at its point and move that")
            .conflict::<RefMoved>(
                "The ref is at neither `expected` nor `to`: another move came first, and it is at `current`",
            )
            .fallible(),
        Endpoint::new::<RefView>("delete", "/api/refs/{name}", delete(refs::delete))
            .describe(
                "deleteRef",
                "Delete a ref from the point its deleter saw it at; the points it named stay",
                "refs",
            )
            .path_parameters::<RefName>()
            .query::<RefDeleteQuery>()
            .forbidden("The ref is one a package ships, which is fixed")
            .conflict::<RefMoved>(
                "The ref is not at `expected`: it moved since, and it is at `current`",
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
    let document = openapi::document(&endpoints, &[openapi::schema_of::<SamplePackages>]);
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

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".into(),
        engine: "native".into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> StudioState {
        StudioState::new(Engine::new(axionomy_store::Store::memory().unwrap()).unwrap())
    }

    #[test]
    fn openapi_contains_full_service_surface() {
        let (_, openapi) = api(state());
        let value = serde_json::to_value(openapi).unwrap();
        let paths = value["paths"].as_object().unwrap();
        for path in [
            "/api/health",
            "/api/refs",
            "/api/refs/{name}",
            "/api/refs/{name}/advance",
            "/api/refs/{name}/frames",
            "/api/packages",
            "/api/points/{point}/document",
            "/api/points/{point}/ancestor",
            "/api/points/{point}/candidates",
            "/api/points/{point}/assess",
            "/api/points/{point}/apply",
            "/api/points/{point}/run",
            "/api/experiments/{experiment}/results",
            "/api/experiments/{experiment}/run",
            "/api/compare",
        ] {
            assert!(paths.contains_key(path), "missing {path}");
        }
        // A package loads from its bytes; a run streams its progress.
        assert!(
            paths["/api/packages"]["post"]["requestBody"]["content"]
                .get("application/octet-stream")
                .is_some()
        );
        assert_eq!(
            paths["/api/points/{point}/run"]["post"]["responses"]["200"]["content"]["text/event-stream"]
                ["schema"]["$ref"],
            "#/components/schemas/StrategyEvent"
        );
        // A move that finds the ref elsewhere says where it is.
        assert_eq!(
            paths["/api/refs/{name}/advance"]["post"]["responses"]["409"]["content"]["application/json"]
                ["schema"]["$ref"],
            "#/components/schemas/RefMoved"
        );
        // A ref is deleted from where its deleter saw it; a package's is fixed.
        let deleted = &paths["/api/refs/{name}"]["delete"];
        assert_eq!(deleted["operationId"], "deleteRef");
        assert_eq!(
            deleted["responses"]["409"]["content"]["application/json"]["schema"]["$ref"],
            "#/components/schemas/RefMoved"
        );
        for operation in [deleted, &paths["/api/refs/{name}/advance"]["post"]] {
            assert_eq!(
                operation["responses"]["403"]["content"]["application/json"]["schema"]["$ref"],
                "#/components/schemas/ErrorResponse"
            );
        }
        let schemas = value["components"]["schemas"].as_object().unwrap();
        assert!(schemas.contains_key("PointId"));
        // Read by Studio from a static site, though no endpoint serves it.
        assert!(schemas.contains_key("SamplePackages"));
    }
}
