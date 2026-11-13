//! A point's balances over HTTP: every account at the point and what it
//! holds there, exactly, by the paths exchanges bind.

use axionomy::{Quantity, Rate, basket, path::Path, paths};
use axionomy_engine::Engine;
use axionomy_store::{Point, Store};
use axionomy_studio_server::{PointId, StudioState, api};
use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

fn path<K>(text: &str) -> Path<K> {
    text.parse().unwrap()
}

/// A bank holding cash and gold, paying alice, served by a Studio server.
fn bank() -> (Engine, Point, Router) {
    let engine = Engine::new(Store::memory().unwrap()).unwrap();
    let economy = paths::EconomyBuilder::new()
        .account(
            path("bank"),
            basket([(path("gold"), 3), (path("cash"), 20)]).into(),
        )
        .account(path("alice"), basket([]).into())
        .rate(
            path("pay"),
            Rate::new()
                .consume(path("from"), basket([(path("cash"), 1)]))
                .produce(path("to"), basket([(path("cash"), 1)])),
        )
        .build()
        .unwrap();
    let start = engine.author(economy).unwrap();
    let (router, _) = api(StudioState::new(engine.clone()));
    (engine, start, router)
}

async fn balances(router: &Router, point: &str) -> (StatusCode, Value) {
    let request = Request::get(format!("/api/points/{point}/balances"))
        .body(Body::empty())
        .unwrap();
    let response = router.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    // JSON, or the text of a request Axum refused before it reached the API.
    let body = serde_json::from_slice(&bytes)
        .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).into_owned()));
    (status, body)
}

#[tokio::test]
async fn a_points_balances_are_every_account_and_what_it_holds() {
    let (engine, start, router) = bank();
    let at = PointId::from(start).to_string();
    let (status, held) = balances(&router, &at).await;
    assert_eq!(status, StatusCode::OK, "{held}");
    // Accounts and assets in the order of their paths, quantities exactly.
    assert_eq!(
        held,
        json!({
            "point": at,
            "accounts": [
                { "account": "alice", "balances": [] },
                { "account": "bank", "balances": [["cash", "20"], ["gold", "3"]] },
            ],
        })
    );

    // A point a step on holds what the step moved; the point it came from
    // never changes.
    let pay = paths::Exchange::new(path("pay"), Quantity::new(20))
        .bind(path("from"), path("bank"))
        .bind(path("to"), path("alice"));
    let paid = PointId::from(engine.apply(start, &[pay]).unwrap()).to_string();
    let (_, after) = balances(&router, &paid).await;
    assert_eq!(
        after["accounts"],
        json!([
            { "account": "alice", "balances": [["cash", "20"]] },
            { "account": "bank", "balances": [["gold", "3"]] },
        ])
    );
    assert_eq!(balances(&router, &at).await.1, held);

    // A point the store lacks is not found; text that is no point is refused.
    let unknown = "run-000000000000feed0000000000000007:0";
    assert_eq!(balances(&router, unknown).await.0, StatusCode::NOT_FOUND);
    assert_eq!(
        balances(&router, "nowhere").await.0,
        StatusCode::BAD_REQUEST
    );
}
