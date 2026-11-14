//! Two points' histories compared over HTTP: where they part, the exchange
//! each side applied at every step after, the balances the two states differ
//! in, exactly, and a window paged on from `next`.

use axionomy::{Quantity, Rate, basket, path::Path, paths};
use axionomy_engine::{Engine, Measure, Objective, ScriptCall};
use axionomy_search::pareto::ObjectiveDirection;
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

fn pay(units: u64) -> paths::Exchange {
    paths::Exchange::new(path("pay"), Quantity::new(units))
        .bind(path("from"), path("bank"))
        .bind(path("to"), path("alice"))
}

/// A bank paying alice, and an objective of what she holds, served by a
/// Studio server over the same engine.
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
    let start = engine.author(economy).unwrap();
    let (router, _) = api(StudioState::new(engine.clone()));
    (engine, start, router)
}

async fn divergence(router: &Router, query: &str) -> (StatusCode, Value) {
    let request = Request::get(format!("/api/divergence?{query}"))
        .body(Body::empty())
        .unwrap();
    let response = router.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body = serde_json::from_slice(&bytes)
        .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).into_owned()));
    (status, body)
}

#[tokio::test]
async fn a_branch_and_its_parent_compare_step_by_step() {
    let (engine, start, router) = bank();
    let parent = engine.apply(start, &[pay(1), pay(2), pay(3)]).unwrap();
    let fork = engine.ancestor(parent, 1).unwrap();
    let branch = engine.apply(fork, &[pay(5)]).unwrap();
    let (left, right) = (PointId::from(parent), PointId::from(branch));

    let (status, compared) = divergence(
        &router,
        &format!("left={left}&right={right}&objectives=paid"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{compared}");
    assert_eq!(
        compared,
        json!({
            "shared": PointId::from(fork).to_string(),
            "left": { "point": left.to_string(), "values": ["6"] },
            "right": { "point": right.to_string(), "values": ["6"] },
            "objectives": [{ "objective": "paid", "direction": "maximize" }],
            "steps": [
                {
                    "step": 2,
                    "left": pay(2),
                    "right": pay(5),
                    "balances": [
                        { "account": "alice", "asset": "cash", "left": "3", "right": "6" },
                        { "account": "bank", "asset": "cash", "left": "17", "right": "14" },
                    ],
                    "accounts": [],
                },
                {
                    "step": 3,
                    "left": pay(3),
                    "balances": [],
                    "accounts": [],
                },
            ],
            // The parent's third payment brings it to what the branch holds.
            "converged": 3,
        })
    );

    // One step at a time: `next` asks for the rest.
    let (_, page) = divergence(&router, &format!("left={left}&right={right}&limit=1")).await;
    assert_eq!(page["steps"].as_array().unwrap().len(), 1);
    assert_eq!(page["next"], 2);
    let (_, rest) = divergence(
        &router,
        &format!("left={left}&right={right}&limit=1&after=2"),
    )
    .await;
    assert_eq!(rest["steps"][0]["step"], 3);
    assert!(rest.get("next").is_none(), "{rest}");
}

#[tokio::test]
async fn histories_that_share_nothing_are_refused() {
    let (engine, start, router) = bank();
    let economy = engine.reach(start).unwrap().state.economy.clone();
    let apart = PointId::from(engine.author(economy).unwrap());
    let start = PointId::from(start);
    let (status, refused) = divergence(&router, &format!("left={start}&right={apart}")).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(
        refused["error"]
            .as_str()
            .unwrap()
            .contains("share no history"),
        "{refused}"
    );
    // A window too wide, and a point the store lacks.
    let (status, _) = divergence(&router, &format!("left={start}&right={start}&limit=0")).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let unknown = "run-000000000000feed0000000000000007:0";
    let (status, _) = divergence(&router, &format!("left={start}&right={unknown}")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
