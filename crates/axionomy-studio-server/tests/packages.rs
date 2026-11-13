//! A package over HTTP, as Studio uses one: loaded from its bytes, listed
//! with its refs and what it defines, a point's history drawn, the
//! candidates at a point assessed, one applied and a ref of one's own moved
//! over it, the package's own refs fixed, a strategy continued from a point
//! and an experiment run while their progress streams, and their ends
//! compared.

use axionomy_engine::{Engine, Version, zipped};
use axionomy_store::Store;
use axionomy_studio_server::{PointId, StudioState, api};
use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

/// The maze sample's package file, zipped, as `axionomy package samples`
/// writes it.
fn maze() -> Vec<u8> {
    let name = "maze".parse().unwrap();
    let source = Engine::new(Store::memory().unwrap()).unwrap();
    axionomy_problems::maze::package::write(&source).unwrap();
    let file = std::env::temp_dir().join(format!(
        "axionomy-studio-server-maze-{}-{:?}.redb",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_file(&file);
    {
        let package = Store::open(&file).unwrap();
        let contents = source.named_under(&name).unwrap();
        source
            .export(
                &name,
                "0.1.0".parse::<Version>().unwrap(),
                contents,
                &package,
            )
            .unwrap();
    }
    let bytes = std::fs::read(&file).unwrap();
    std::fs::remove_file(&file).unwrap();
    zipped(&bytes).unwrap()
}

fn studio() -> Router {
    let engine = Engine::new(Store::memory().unwrap()).unwrap();
    api(StudioState::new(engine)).0
}

async fn send(router: &Router, request: Request<Body>) -> (StatusCode, Value) {
    let response = router.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body = serde_json::from_slice(&bytes)
        .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).into_owned()));
    (status, body)
}

async fn get(router: &Router, uri: &str) -> (StatusCode, Value) {
    send(router, Request::get(uri).body(Body::empty()).unwrap()).await
}

async fn delete(router: &Router, uri: &str) -> (StatusCode, Value) {
    send(router, Request::delete(uri).body(Body::empty()).unwrap()).await
}

async fn post(router: &Router, uri: &str, body: Value) -> (StatusCode, Value) {
    let request = Request::post(uri)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    send(router, request).await
}

fn ok((status, body): (StatusCode, Value)) -> Value {
    assert_eq!(status, StatusCode::OK, "{body}");
    body
}

/// The events of a stream, in order, each its JSON data.
async fn streamed(router: &Router, uri: &str, body: Value) -> Vec<Value> {
    let request = Request::post(uri)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    let response = router.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers()["content-type"].to_str().unwrap(),
        "text/event-stream"
    );
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    String::from_utf8(bytes.to_vec())
        .unwrap()
        .lines()
        .filter_map(|line| line.strip_prefix("data: "))
        .map(|data| serde_json::from_str(data).unwrap())
        .collect()
}

async fn load(router: &Router, bytes: Vec<u8>) -> (StatusCode, Value) {
    let request = Request::post("/api/packages")
        .header("content-type", "application/octet-stream")
        .body(Body::from(bytes))
        .unwrap();
    send(router, request).await
}

fn point(value: &Value) -> PointId {
    value.as_str().unwrap().parse().unwrap()
}

/// The point the maze's ref `name` names, as the package list gives it.
async fn referred(router: &Router, name: &str) -> PointId {
    let packages = ok(get(router, "/api/packages").await);
    let refs = packages["packages"][0]["refs"].as_array().unwrap();
    let named = refs.iter().find(|entry| entry["name"] == name).unwrap();
    point(&named["point"])
}

#[tokio::test]
async fn a_package_loads_from_its_bytes_and_lists_what_it_holds() {
    let router = studio();
    let bytes = maze();
    let loaded = ok(load(&router, bytes.clone()).await);
    assert_eq!(
        loaded,
        json!({ "packages": ["maze"], "kept": { "refs": [], "definitions": [] } })
    );

    let listed = ok(get(&router, "/api/packages").await);
    let listed = &listed["packages"][0];
    assert_eq!(listed["name"], "maze");
    assert_eq!(listed["version"], "0.1.0");
    let refs: Vec<&str> = listed["refs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["name"].as_str().unwrap())
        .collect();
    assert_eq!(refs, ["maze/micro", "maze/showcase", "maze/stress"]);
    // Each is the package's, so fixed.
    assert!(
        listed["refs"]
            .as_array()
            .unwrap()
            .iter()
            .all(|entry| entry["shipped_by"] == "maze")
    );
    assert_eq!(listed["goals"], json!(["maze/solved"]));
    assert_eq!(listed["candidates"], json!(["maze/moves"]));
    assert_eq!(listed["strategies"].as_array().unwrap().len(), 5);
    assert_eq!(
        listed["objectives"],
        json!([
            { "objective": "maze/energy", "direction": "minimize" },
            { "objective": "maze/time", "direction": "minimize" },
        ])
    );
    let micro = listed["experiments"]
        .as_array()
        .unwrap()
        .iter()
        .find(|experiment| experiment["experiment"] == "maze/micro")
        .unwrap();
    assert_eq!(micro["start"], "maze/micro");
    assert_eq!(micro["goal"], "maze/solved");
    // A seed, a budget and turns are written in decimal, exactly.
    assert_eq!(micro["seed"], "17");
    assert_eq!(micro["budget"], "10000000");
    assert_eq!(micro["turns"], "0");
    assert_eq!(micro["recorded"], false);

    // Loading it again keeps nothing of its own; a file that is no package is refused.
    let again = ok(load(&router, bytes).await);
    assert_eq!(again["kept"], json!({ "refs": [], "definitions": [] }));
    let (status, refused) = load(&router, b"no package".to_vec()).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{refused}");
    assert!(
        refused["error"]
            .as_str()
            .unwrap()
            .contains("not a package file")
    );
}

#[tokio::test]
async fn a_candidate_applied_at_a_refs_point_moves_the_ref_and_elsewhere_branches() {
    let router = studio();
    ok(load(&router, maze()).await);
    let start = referred(&router, "maze/micro").await;
    // A player names a ref of their own where the package's starts.
    let named = ok(post(
        &router,
        "/api/refs",
        json!({ "name": "maze/play", "point": start.to_string() }),
    )
    .await);
    assert_eq!(
        named,
        json!({ "name": "maze/play", "point": start.to_string() })
    );

    // The moves at the start, each assessed there and read with the maze's view.
    let offered = ok(get(
        &router,
        &format!("/api/points/{start}/candidates?set=maze%2Fmoves"),
    )
    .await);
    let candidates = offered["candidates"].as_array().unwrap();
    assert!(!candidates.is_empty());
    let applicable: Vec<&Value> = candidates
        .iter()
        .filter(|candidate| candidate["proposal"]["assessment"]["status"] == "applicable")
        .collect();
    assert!(!applicable.is_empty(), "{offered}");
    let first = applicable[0]["exchange"].clone();
    assert!(
        !applicable[0]["proposal"]["exchange"]["rate"]["label"]
            .as_str()
            .unwrap()
            .is_empty()
    );

    // Assessing records nothing; applying gives a new point a step on.
    let assessed = ok(post(
        &router,
        &format!("/api/points/{start}/assess"),
        json!({ "exchange": first }),
    )
    .await);
    assert_eq!(assessed["proposal"]["assessment"]["status"], "applicable");
    let applied = ok(post(
        &router,
        &format!("/api/points/{start}/apply"),
        json!({ "exchanges": [first] }),
    )
    .await);
    assert_eq!(point(&applied["from"]), start);
    let moved = point(&applied["point"]);
    assert_eq!(moved.point().step, 1);

    // The package's ref is fixed: it neither moves nor goes, and says
    // whose it is.
    for (status, refused) in [
        post(
            &router,
            "/api/refs/maze%2Fmicro/advance",
            json!({ "expected": start.to_string(), "to": moved.to_string() }),
        )
        .await,
        delete(&router, &format!("/api/refs/maze%2Fmicro?expected={start}")).await,
    ] {
        assert_eq!(status, StatusCode::FORBIDDEN, "{refused}");
        let error = refused["error"].as_str().unwrap();
        assert!(
            error.contains("`maze/micro`") && error.contains("package `maze`"),
            "{error}"
        );
    }
    let read = ok(get(&router, "/api/refs/maze%2Fmicro").await);
    assert_eq!(read["shipped_by"], "maze");
    assert_eq!(point(&read["point"]), start);

    // At the player's ref's point the move counts once the ref moves over it.
    ok(post(
        &router,
        "/api/refs/maze%2Fplay/advance",
        json!({ "expected": start.to_string(), "to": moved.to_string() }),
    )
    .await);
    assert_eq!(referred(&router, "maze/play").await, moved);
    assert_eq!(referred(&router, "maze/micro").await, start);
    let drawn = ok(get(&router, &format!("/api/points/{moved}/document")).await);
    assert_eq!(drawn["frames"].as_array().unwrap().len(), 1);
    assert_eq!(drawn["title"], "Key-door maze");

    // Asking again for the move made is told it was made.
    let again = ok(post(
        &router,
        "/api/refs/maze%2Fplay/advance",
        json!({ "expected": start.to_string(), "to": moved.to_string() }),
    )
    .await);
    assert_eq!(point(&again["point"]), moved);

    // A mover who still saw the start, with a move of its own, is told
    // where the ref went.
    let second = applicable.last().unwrap()["exchange"].clone();
    let other = ok(post(
        &router,
        &format!("/api/points/{start}/apply"),
        json!({ "exchanges": [second] }),
    )
    .await);
    let (status, conflict) = post(
        &router,
        "/api/refs/maze%2Fplay/advance",
        json!({ "expected": start.to_string(), "to": other["point"] }),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(point(&conflict["current"]), moved);

    // Anywhere else, applying makes a branch: the ref stays where it is. The
    // start of the moved point's history is the start, by its one name.
    let earlier = ok(get(&router, &format!("/api/points/{moved}/ancestor?step=0")).await);
    let earlier = point(&earlier["point"]);
    assert_eq!(earlier, start);
    let branched = ok(post(
        &router,
        &format!("/api/points/{earlier}/apply"),
        json!({ "exchanges": [second] }),
    )
    .await);
    let branch = point(&branched["point"]);
    assert_eq!(branch.point().step, 1);
    assert_ne!(branch, moved);
    assert_eq!(referred(&router, "maze/play").await, moved);

    // What the store lacks is 404; an exchange that does not apply is 400.
    let unknown = "run-000000000000feed0000000000000007:0";
    let (status, _) = get(&router, &format!("/api/points/{unknown}/document")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = get(
        &router,
        &format!("/api/points/{start}/candidates?set=maze%2Fnone"),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let mut impossible = first.clone();
    impossible["units"] = json!("1000");
    let (status, refused) = post(
        &router,
        &format!("/api/points/{start}/apply"),
        json!({ "exchanges": [impossible] }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{refused}");
}

#[tokio::test]
async fn a_strategy_continues_from_a_point_reporting_its_thinking() {
    let router = studio();
    ok(load(&router, maze()).await);
    let start = referred(&router, "maze/micro").await;
    let events = streamed(
        &router,
        &format!("/api/points/{start}/run"),
        json!({
            "strategy": "maze/a-star",
            "goal": "maze/solved",
            "seed": "17",
            "budget": "10000000",
            "turns": "0",
        }),
    )
    .await;
    let ran = events.last().unwrap();
    assert_eq!(ran["kind"], "ran", "{events:?}");
    let ran = &ran["ran"];
    assert_eq!(ran["strategy"], "maze/a-star");
    assert_eq!(ran["goal"], "maze/solved");
    assert_eq!(point(&ran["start"]), start);
    assert_eq!(ran["outcome"]["kind"], "reached");
    assert!(ran["thinking"]["expanded"].as_u64().unwrap() > 0);
    // The run is a branch from the start, drawn like any point.
    let end = point(&ran["end"]);
    assert!(end.point().step > 0);
    let drawn = ok(get(&router, &format!("/api/points/{end}/document")).await);
    assert_eq!(
        drawn["frames"].as_array().unwrap().len() as u64,
        end.point().step
    );

    // A strategy or a goal the store lacks is refused before anything runs.
    for (strategy, goal) in [("maze/none", "maze/solved"), ("maze/a-star", "maze/none")] {
        let (status, _) = post(
            &router,
            &format!("/api/points/{start}/run"),
            json!({ "strategy": strategy, "goal": goal, "seed": "17", "budget": "1", "turns": "0" }),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{strategy} toward {goal}");
    }
    // Every run names the goal it works toward.
    let (status, _) = post(
        &router,
        &format!("/api/points/{start}/run"),
        json!({ "strategy": "maze/a-star", "seed": "17", "budget": "1", "turns": "0" }),
    )
    .await;
    assert!(status.is_client_error(), "{status}");
}

#[tokio::test]
async fn an_experiment_runs_its_arms_and_their_ends_compare() {
    let router = studio();
    ok(load(&router, maze()).await);
    let (status, _) = get(&router, "/api/experiments/maze%2Fmicro/results").await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let events = streamed(&router, "/api/experiments/maze%2Fmicro/run", json!(null)).await;
    let last = events.last().unwrap();
    assert_eq!(last["kind"], "results", "{events:?}");
    let results = &last["results"];
    assert_eq!(results["experiment"], "maze/micro");
    let arms = results["arms"].as_array().unwrap();
    assert_eq!(arms.len(), 5);
    let trial = &arms[0]["trials"][0];
    assert_eq!(trial["ended"]["kind"], "reached");
    assert!(trial["seed"].is_string());
    assert_eq!(trial["values"].as_array().unwrap().len(), 2);
    assert_eq!(
        ok(get(&router, "/api/experiments/maze%2Fmicro/results").await),
        *results
    );

    // Every arm's end compared on energy and time.
    let ends: Vec<Value> = arms
        .iter()
        .map(|arm| arm["trials"][0]["end"].clone())
        .collect();
    let compared = ok(post(
        &router,
        "/api/compare",
        json!({ "points": ends, "objectives": ["maze/energy", "maze/time"] }),
    )
    .await);
    assert_eq!(compared["values"].as_array().unwrap().len(), 5);
    let front = compared["front"].as_array().unwrap();
    assert!(!front.is_empty());
    assert!(front.contains(&compared["pick"]));

    // An experiment the store lacks is refused before anything runs.
    let request = Request::post("/api/experiments/maze%2Fnone/run")
        .body(Body::empty())
        .unwrap();
    let (status, _) = send(&router, request).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
