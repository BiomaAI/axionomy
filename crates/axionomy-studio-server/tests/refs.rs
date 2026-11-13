//! Refs over HTTP: a ref names the point an application is at, moving it
//! forward is what makes a step count, and a follower receives only the steps
//! it was moved over, waiting for the next.

use axionomy::{Quantity, Rate, basket, path::Path, paths};
use axionomy_engine::{Engine, Measure, Objective, ScriptCall};
use axionomy_search::pareto::ObjectiveDirection;
use axionomy_store::{Point, Store};
use axionomy_studio_server::{
    ErrorResponse, PointId, RefFrames, RefMoved, RefView, StudioState, api,
};
use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::time::{Duration, Instant};
use tower::ServiceExt;

fn path<K>(text: &str) -> Path<K> {
    text.parse().unwrap()
}

fn pay(units: u64) -> paths::Exchange {
    paths::Exchange::new(path("pay"), Quantity::new(units))
        .bind(path("from"), path("bank"))
        .bind(path("to"), path("alice"))
}

/// A bank paying alice, a goal of alice holding ten, and an objective of
/// what she holds, served by a Studio server over the same engine.
fn bank() -> (Engine, Point, Router) {
    let engine = Engine::new(Store::memory().unwrap()).unwrap();
    let economy = paths::EconomyBuilder::new()
        .account(path("bank"), basket([(path("cash"), 20)]).into())
        .account(path("alice"), basket([]).into())
        .rate(
            path("pay"),
            Rate::new()
                .consume(path("from"), basket([(path("cash"), 1)]))
                .produce(path("to"), basket([(path("cash"), 1)])),
        )
        .build()
        .unwrap();
    engine
        .keep_script(
            &path("bank"),
            r#"fn paid(state) { state.balance("alice", "cash") }"#,
        )
        .unwrap();
    engine
        .keep_objective(
            &path("paid"),
            &Objective {
                measure: Measure::State(ScriptCall::new(path("bank"), "paid".parse().unwrap())),
                direction: ObjectiveDirection::Maximize,
            },
        )
        .unwrap();
    engine
        .keep_goal(
            &path("rich"),
            &paths::Goal::new().require(path("alice"), basket([(path("cash"), 10)])),
        )
        .unwrap();
    let start = engine.author(economy).unwrap();
    let (router, _) = api(StudioState::new(engine.clone()));
    (engine, start, router)
}

fn point(at: Point) -> String {
    PointId::from(at).to_string()
}

/// A request's status and body: JSON, or the text of a request Axum refused
/// before it reached the API.
async fn send(router: &Router, request: Request<Body>) -> (StatusCode, Value) {
    let response = router.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body = serde_json::from_slice(&bytes)
        .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).into_owned()));
    (status, body)
}

async fn post(router: &Router, uri: &str, body: Value) -> (StatusCode, Value) {
    let request = Request::post(uri)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    send(router, request).await
}

async fn get(router: &Router, uri: &str) -> (StatusCode, Value) {
    send(router, Request::get(uri).body(Body::empty()).unwrap()).await
}

async fn delete(router: &Router, expected: Point) -> (StatusCode, Value) {
    let uri = format!("{LIVE}?expected={}", point(expected));
    send(router, Request::delete(uri).body(Body::empty()).unwrap()).await
}

fn ok<T: DeserializeOwned>((status, body): (StatusCode, Value)) -> T {
    assert_eq!(status, StatusCode::OK, "{body}");
    serde_json::from_value(body).unwrap()
}

/// A name with a `/` is one segment of the URL, its `/` written `%2F`.
const LIVE: &str = "/api/refs/live%2Fgame";

async fn advance(router: &Router, expected: Point, to: Point) -> (StatusCode, Value) {
    post(
        router,
        &format!("{LIVE}/advance"),
        json!({ "expected": point(expected), "to": point(to) }),
    )
    .await
}

#[tokio::test]
async fn a_ref_moves_only_forward_from_where_its_mover_expects_it() {
    let (engine, start, router) = bank();
    let created: RefView = ok(post(
        &router,
        "/api/refs",
        json!({ "name": "live/game", "point": point(start) }),
    )
    .await);
    assert_eq!(created.name, path("live/game"));
    assert_eq!(created.point, start.into());
    let read: RefView = ok(get(&router, LIVE).await);
    assert_eq!(read, created);
    // A name is given once.
    let (status, again) = post(
        &router,
        "/api/refs",
        json!({ "name": "live/game", "point": point(start) }),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    let taken: RefMoved = serde_json::from_value(again).unwrap();
    assert_eq!(taken.current, start.into());

    // Two proposers from the same point: the first moves the ref, the second
    // is told where it went and looks again from there.
    let first = engine.apply(start, &[pay(5)]).unwrap();
    let second = engine.apply(start, &[pay(3)]).unwrap();
    let moved: RefView = ok(advance(&router, start, first).await);
    assert_eq!(moved.point, first.into());
    let (status, refused) = advance(&router, start, second).await;
    assert_eq!(status, StatusCode::CONFLICT);
    let refused: RefMoved = serde_json::from_value(refused).unwrap();
    assert_eq!(
        (refused.name, refused.current),
        (path("live/game"), first.into())
    );
    let read: RefView = ok(get(&router, LIVE).await);
    assert_eq!(read.point, first.into());

    // Nor does it move back, or onto another history.
    for to in [start, second] {
        let (status, refused) = advance(&router, first, to).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        let refused: ErrorResponse = serde_json::from_value(refused).unwrap();
        assert!(
            refused.error.contains("moves only forward"),
            "{}",
            refused.error
        );
    }

    // What the store lacks is not found; what is not a point is refused.
    let (status, _) = get(&router, "/api/refs/nothing").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = post(
        &router,
        "/api/refs/nothing/advance",
        json!({ "expected": point(start), "to": point(first) }),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let beyond = Point {
        run: start.run,
        step: 9,
    };
    let (status, _) = post(
        &router,
        "/api/refs",
        json!({ "name": "ghost", "point": point(beyond) }),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = post(
        &router,
        "/api/refs",
        json!({ "name": "ghost", "point": "run-feed:0" }),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn a_ref_is_deleted_from_where_its_deleter_saw_it() {
    let (engine, start, router) = bank();
    let _: RefView = ok(post(
        &router,
        "/api/refs",
        json!({ "name": "live/game", "point": point(start) }),
    )
    .await);
    let first = engine.apply(start, &[pay(5)]).unwrap();
    let _: RefView = ok(advance(&router, start, first).await);

    // A deleter who still saw the start is told where the ref went.
    let (status, refused) = delete(&router, start).await;
    assert_eq!(status, StatusCode::CONFLICT);
    let refused: RefMoved = serde_json::from_value(refused).unwrap();
    assert_eq!(refused.current, first.into());
    let read: RefView = ok(get(&router, LIVE).await);
    assert_eq!(read.point, first.into());

    // From where it is, it goes; what it named stays a point.
    let deleted: RefView = ok(delete(&router, first).await);
    assert_eq!(deleted, read);
    let (status, _) = get(&router, LIVE).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = delete(&router, first).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = get(
        &router,
        &format!("/api/points/{}/ancestor?step=0", point(first)),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    // Without the point it saw, a deleter is refused before anything goes.
    let request = Request::delete(LIVE).body(Body::empty()).unwrap();
    let (status, _) = send(&router, request).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn a_follower_receives_only_the_steps_a_ref_moves_over() {
    let (engine, start, router) = bank();
    let _: RefView = ok(post(
        &router,
        "/api/refs",
        json!({ "name": "live/game", "point": point(start) }),
    )
    .await);
    let first = engine.apply(start, &[pay(5)]).unwrap();
    // Tried out, never moved over: never delivered.
    let tried = engine.apply(first, &[pay(15)]).unwrap();
    let _: RefView = ok(advance(&router, start, first).await);
    let then = engine.apply(first, &[pay(1), pay(4)]).unwrap();
    let _: RefView = ok(advance(&router, first, then).await);

    let frames = |after: Point, query: &str| {
        let router = router.clone();
        let uri = format!("{LIVE}/frames?after={}{query}", point(after));
        async move { ok::<RefFrames>(get(&router, &uri).await) }
    };
    let followed = frames(start, "&goal=rich&objectives=paid").await;
    assert_eq!(followed.point, then.into());
    let delivered: Vec<_> = followed
        .frames
        .iter()
        .map(|frame| {
            (
                frame.point.point().step,
                frame.exchange.units(),
                frame.values.clone(),
                frame.holds,
            )
        })
        .collect();
    assert_eq!(
        delivered,
        [
            (1, Quantity::new(5), vec![Quantity::new(5)], Some(false)),
            (2, Quantity::new(1), vec![Quantity::new(6)], Some(false)),
            (3, Quantity::new(4), vec![Quantity::new(10)], Some(true)),
        ]
    );
    assert_eq!(followed.frames[2].point, then.into());
    assert!(
        followed
            .frames
            .iter()
            .all(|frame| frame.point != tried.into())
    );
    assert_eq!(
        followed.frames[0].receipt.exchange().units(),
        Quantity::new(5)
    );
    // Without a goal or objectives, a frame measures nothing.
    let plain = frames(start, "").await;
    assert!(plain.frames[0].values.is_empty() && plain.frames[0].holds.is_none());

    // A follower pulls from the last point it handled, a page at a time.
    let page = frames(start, "&limit=2").await;
    assert_eq!(page.frames.len(), 2);
    let rest = frames(page.frames[1].point.point(), "&limit=2").await;
    assert_eq!(rest.frames.len(), 1);
    assert_eq!(rest.frames[0].point, then.into());
    assert!(frames(then, "").await.frames.is_empty());

    // A point that is not on the ref's history is not where anyone follows
    // from, and what the store does not define is not found.
    let elsewhere = engine.apply(start, &[pay(2)]).unwrap();
    let (status, _) = get(
        &router,
        &format!("{LIVE}/frames?after={}", point(elsewhere)),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = get(
        &router,
        &format!("{LIVE}/frames?after={}&goal=poor", point(start)),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    for query in ["&limit=0", "&limit=1001", "&wait_seconds=31"] {
        let (status, _) = get(
            &router,
            &format!("{LIVE}/frames?after={}{query}", point(start)),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{query}");
    }
}

#[tokio::test]
async fn a_follower_waits_for_the_ref_to_move() {
    let (engine, start, router) = bank();
    let _: RefView = ok(post(
        &router,
        "/api/refs",
        json!({ "name": "live/game", "point": point(start) }),
    )
    .await);
    // Nothing moves: the wait ends with the time, where the ref is.
    let waited = Instant::now();
    let idle: RefFrames = ok(get(
        &router,
        &format!("{LIVE}/frames?after={}&wait_seconds=1", point(start)),
    )
    .await);
    assert!(waited.elapsed() >= Duration::from_secs(1));
    assert!(idle.frames.is_empty());
    assert_eq!(idle.point, start.into());

    let mover = {
        let router = router.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(100)).await;
            let next = engine.apply(start, &[pay(1)]).unwrap();
            let _: RefView = ok(advance(&router, start, next).await);
            next
        })
    };
    let waited = Instant::now();
    let moved: RefFrames = ok(get(
        &router,
        &format!("{LIVE}/frames?after={}&wait_seconds=30", point(start)),
    )
    .await);
    assert!(waited.elapsed() < Duration::from_secs(30));
    let next = mover.await.unwrap();
    assert_eq!(moved.point, next.into());
    assert_eq!(
        moved
            .frames
            .iter()
            .map(|frame| frame.point)
            .collect::<Vec<_>>(),
        [next.into()]
    );
}
