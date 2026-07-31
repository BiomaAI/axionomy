use axionomy::{Account, Basket, EconomyBuilder, Exchange, Goal, Quantity, Rate};
use axionomy_mcp::{MemorySnapshotStore, SearchRequest, WireEconomy, stateless_http_service};
use axum::Router;
use reqwest::{Client, Response, StatusCode};
use serde_json::{Value, json};
use tokio_util::sync::CancellationToken;

const PROTOCOL_VERSION: &str = "2026-07-28";

fn one(asset: &str) -> Basket<String> {
    [(asset.to_owned(), Quantity::new(1))].into_iter().collect()
}

fn amount(asset: &str, quantity: u64) -> Basket<String> {
    [(asset.to_owned(), Quantity::new(quantity))]
        .into_iter()
        .collect()
}

fn fixture() -> (
    WireEconomy,
    Exchange<String, String, String>,
    Goal<String, String>,
) {
    fixture_with_amounts(1, 1)
}

fn fixture_with_amounts(
    source_balance: u64,
    goal_balance: u64,
) -> (
    WireEconomy,
    Exchange<String, String, String>,
    Goal<String, String>,
) {
    let economy = EconomyBuilder::new()
        .account(
            "source".to_owned(),
            Account::new(amount("token", source_balance)),
        )
        .account("sink".to_owned(), Account::default())
        .rate(
            "transfer".to_owned(),
            Rate::new()
                .consume("giver".to_owned(), one("token"))
                .produce("receiver".to_owned(), one("token"))
                .distinct("giver".to_owned(), "receiver".to_owned()),
        )
        .build()
        .unwrap();
    let exchange = Exchange::new("transfer".to_owned(), Quantity::new(1))
        .bind("giver".to_owned(), "source".to_owned())
        .bind("receiver".to_owned(), "sink".to_owned());
    let goal = Goal::new().require("sink".to_owned(), amount("token", goal_balance));
    (economy, exchange, goal)
}

async fn spawn_server() -> (Client, String, CancellationToken) {
    let cancellation = CancellationToken::new();
    let service = stateless_http_service(MemorySnapshotStore::default(), cancellation.clone());
    let app = Router::new().nest_service("/mcp", service);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let shutdown = cancellation.clone();
    tokio::spawn(async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(shutdown.cancelled_owned())
            .await
            .unwrap();
    });
    (Client::new(), format!("http://{address}/mcp"), cancellation)
}

fn metadata(tasks: bool) -> Value {
    let capabilities = if tasks {
        json!({ "extensions": { "io.modelcontextprotocol/tasks": {} } })
    } else {
        json!({})
    };
    json!({
        "io.modelcontextprotocol/protocolVersion": PROTOCOL_VERSION,
        "io.modelcontextprotocol/clientInfo": {
            "name": "axionomy-mcp-tests",
            "version": "0.1.0"
        },
        "io.modelcontextprotocol/clientCapabilities": capabilities
    })
}

async fn post(
    client: &Client,
    url: &str,
    id: u64,
    method: &str,
    mut params: Value,
    tasks: bool,
) -> Response {
    let mcp_name = params
        .get(if method.starts_with("tasks/") {
            "taskId"
        } else {
            "name"
        })
        .and_then(Value::as_str)
        .map(str::to_owned);
    params
        .as_object_mut()
        .expect("MCP params must be an object")
        .insert("_meta".to_owned(), metadata(tasks));
    let mut request = client
        .post(url)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json, text/event-stream")
        .header("MCP-Protocol-Version", PROTOCOL_VERSION)
        .header("Mcp-Method", method)
        .json(&json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params
        }));
    if let Some(mcp_name) = mcp_name {
        request = request.header("Mcp-Name", mcp_name);
    }
    request.send().await.unwrap()
}

#[tokio::test]
async fn strict_stateless_http_runs_a_process_local_search_task() {
    let (client, url, cancellation) = spawn_server().await;

    let missing_header = client
        .post(&url)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json, text/event-stream")
        .json(&json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/list",
            "params": { "_meta": metadata(false) }
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(missing_header.status(), StatusCode::BAD_REQUEST);

    let listed = post(&client, &url, 2, "tools/list", json!({}), false).await;
    let listed_status = listed.status();
    assert!(listed.headers().get("mcp-session-id").is_none());
    let listed_text = listed.text().await.unwrap();
    assert_eq!(listed_status, StatusCode::OK, "{listed_text}");
    let listed: Value = serde_json::from_str(&listed_text).unwrap();
    assert_eq!(listed["result"]["tools"].as_array().unwrap().len(), 5);

    let (economy, exchange, goal) = fixture();
    let put = post(
        &client,
        &url,
        3,
        "tools/call",
        json!({
            "name": "axionomy_economy_put",
            "arguments": { "economy": economy }
        }),
        false,
    )
    .await;
    assert_eq!(put.status(), StatusCode::OK);
    let put: Value = put.json().await.unwrap();
    let economy_id = put["result"]["structuredContent"]["economy_id"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(economy_id.starts_with("eco_"));

    let search = SearchRequest {
        economy_id,
        goal,
        candidates: vec![exchange],
        max_expansions: 8,
        chunk_size: 1,
    };
    let created = post(
        &client,
        &url,
        4,
        "tools/call",
        json!({
            "name": "axionomy_search",
            "arguments": search
        }),
        true,
    )
    .await;
    assert_eq!(created.status(), StatusCode::OK);
    let created: Value = created.json().await.unwrap();
    assert_eq!(created["result"]["resultType"], "task");
    let task_id = created["result"]["taskId"].as_str().unwrap().to_owned();

    let completed = loop {
        let polled = post(
            &client,
            &url,
            5,
            "tasks/get",
            json!({ "taskId": task_id }),
            true,
        )
        .await;
        assert_eq!(polled.status(), StatusCode::OK);
        let polled: Value = polled.json().await.unwrap();
        match polled["result"]["status"].as_str().unwrap() {
            "completed" => break polled,
            "working" => tokio::task::yield_now().await,
            status => panic!("unexpected task status: {status}"),
        }
    };
    assert_eq!(
        completed["result"]["result"]["structuredContent"]["outcome"],
        "solved"
    );
    assert_eq!(
        completed["result"]["result"]["structuredContent"]["solution"]["cost"],
        1
    );

    let (large_economy, large_exchange, unreachable_goal) = fixture_with_amounts(100_000, 100_001);
    let put = post(
        &client,
        &url,
        6,
        "tools/call",
        json!({
            "name": "axionomy_economy_put",
            "arguments": { "economy": large_economy }
        }),
        false,
    )
    .await;
    let put: Value = put.json().await.unwrap();
    let economy_id = put["result"]["structuredContent"]["economy_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let created = post(
        &client,
        &url,
        7,
        "tools/call",
        json!({
            "name": "axionomy_search",
            "arguments": SearchRequest {
                economy_id,
                goal: unreachable_goal,
                candidates: vec![large_exchange],
                max_expansions: 100_000,
                chunk_size: 1,
            }
        }),
        true,
    )
    .await;
    let created: Value = created.json().await.unwrap();
    let task_id = created["result"]["taskId"].as_str().unwrap().to_owned();
    let cancelled = post(
        &client,
        &url,
        8,
        "tasks/cancel",
        json!({ "taskId": task_id }),
        true,
    )
    .await;
    assert_eq!(cancelled.status(), StatusCode::OK);

    let mut status = None;
    for _ in 0..100 {
        let polled = post(
            &client,
            &url,
            9,
            "tasks/get",
            json!({ "taskId": task_id }),
            true,
        )
        .await;
        let polled: Value = polled.json().await.unwrap();
        let current = polled["result"]["status"].as_str().unwrap();
        if current != "working" {
            status = Some(current.to_owned());
            break;
        }
        tokio::task::yield_now().await;
    }
    assert_eq!(status.as_deref(), Some("cancelled"));

    cancellation.cancel();
}
