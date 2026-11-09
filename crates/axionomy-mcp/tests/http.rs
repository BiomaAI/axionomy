use axionomy_mcp::{MemorySnapshotStore, stateless_http_service};
use axum::Router;
use reqwest::{Client, StatusCode};
use serde_json::{Value, json};
use tokio_util::sync::CancellationToken;

const PROTOCOL_VERSION: &str = "2026-07-28";

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

/// One JSON-RPC request with the routing headers the transport requires; the
/// result, or the error when there is no result.
async fn call(client: &Client, url: &str, method: &str, mut params: Value, tasks: bool) -> Value {
    let name_field = match method {
        "tools/call" => Some("name"),
        "resources/read" => Some("uri"),
        _ if method.starts_with("tasks/") => Some("taskId"),
        _ => None,
    };
    let mcp_name = name_field
        .and_then(|field| params.get(field))
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
        .json(&json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params }));
    if let Some(mcp_name) = mcp_name {
        request = request.header("Mcp-Name", mcp_name);
    }
    let response = request.send().await.unwrap();
    let status = response.status();
    let body: Value = response.json().await.unwrap();
    assert_eq!(status, StatusCode::OK, "{body}");
    body.get("result")
        .cloned()
        .unwrap_or_else(|| body["error"].clone())
}

async fn tool(client: &Client, url: &str, name: &str, arguments: Value, tasks: bool) -> Value {
    call(
        client,
        url,
        "tools/call",
        json!({ "name": name, "arguments": arguments }),
        tasks,
    )
    .await
}

async fn read(client: &Client, url: &str, uri: &str) -> (Value, Value) {
    let result = call(client, url, "resources/read", json!({ "uri": uri }), false).await;
    let text = result["contents"][0]["text"]
        .as_str()
        .unwrap_or_else(|| panic!("{result}"));
    (serde_json::from_str(text).unwrap(), result)
}

async fn finished_task(client: &Client, url: &str, task_id: &str) -> Value {
    for _ in 0..1_000 {
        let polled = call(client, url, "tasks/get", json!({ "taskId": task_id }), true).await;
        if polled["status"] != "working" {
            return polled;
        }
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
    panic!("task {task_id} did not finish")
}

fn transfer(units: &str) -> Value {
    json!({ "rate": "transfer", "bindings": { "giver": "source", "receiver": "sink" }, "units": units })
}

#[tokio::test]
async fn strict_stateless_http_serves_tools_resources_completion_and_tasks() {
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

    let tools = call(&client, &url, "tools/list", json!({}), false).await;
    assert_eq!(tools["tools"].as_array().unwrap().len(), 8);
    assert!(tools["ttlMs"].as_u64().unwrap() > 0);
    assert_eq!(tools["cacheScope"], "public");
    let templates = call(&client, &url, "resources/templates/list", json!({}), false).await;
    assert_eq!(templates["resourceTemplates"].as_array().unwrap().len(), 16);
    let listed = call(&client, &url, "resources/list", json!({}), false).await;
    assert_eq!(listed["resources"], json!([]));

    let created = tool(
        &client,
        &url,
        "axionomy_economy_create",
        json!({
            "entries": {
                "accounts": { "source": { "token": "1" }, "sink": {} },
                "rates": {
                    "transfer": {
                        "consume": { "giver": { "token": "1" } },
                        "produce": { "receiver": { "token": "1" } },
                        "distinct": [["giver", "receiver"]]
                    }
                }
            }
        }),
        false,
    )
    .await;
    let economy_id = created["structuredContent"]["economy_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let link = created["content"]
        .as_array()
        .unwrap()
        .iter()
        .find(|block| block["type"] == "resource_link")
        .unwrap()["uri"]
        .as_str()
        .unwrap()
        .to_owned();
    let (summary, result) = read(&client, &url, &link).await;
    assert_eq!(summary["counts"]["accounts"], 2);
    assert_eq!(result["cacheScope"], "private");
    assert!(result["ttlMs"].as_u64().unwrap() > 0);

    let completion = call(
        &client,
        &url,
        "completion/complete",
        json!({
            "ref": { "type": "ref/resource", "uri": "axionomy://economy/{economy_id}/accounts/{account}/balances{?prefix,cursor}" },
            "argument": { "name": "account", "value": "so" },
            "context": { "arguments": { "economy_id": economy_id } }
        }),
        false,
    )
    .await;
    assert_eq!(completion["completion"]["values"], json!(["source"]));

    let infeasible = tool(
        &client,
        &url,
        "axionomy_exchange_apply",
        json!({ "economy_id": economy_id, "exchange": transfer("2") }),
        false,
    )
    .await;
    assert_eq!(infeasible["isError"], true);
    assert_eq!(infeasible["structuredContent"]["error"], "infeasible");

    let search = json!({
        "economy_id": economy_id,
        "goal": { "sink": { "token": "1" } },
        "candidates": [transfer("1")],
        "max_expansions": 8,
        "chunk_size": 1
    });
    let without_tasks = tool(&client, &url, "axionomy_search", search.clone(), false).await;
    assert_eq!(
        without_tasks["structuredContent"]["error"],
        "tasks_required"
    );

    let created = tool(&client, &url, "axionomy_search", search, true).await;
    assert_eq!(created["resultType"], "task");
    let task_id = created["taskId"].as_str().unwrap().to_owned();
    let completed = finished_task(&client, &url, &task_id).await;
    assert_eq!(completed["status"], "completed");
    let solution = &completed["result"]["structuredContent"];
    assert_eq!(solution["outcome"], "solved");
    let solved_id = solution["solution"]["economy_id"].as_str().unwrap();
    let (trace, _) = read(
        &client,
        &url,
        &format!("axionomy://economy/{solved_id}/exchanges"),
    )
    .await;
    assert_eq!(trace["items"].as_array().unwrap().len(), 1);

    let endless = tool(
        &client,
        &url,
        "axionomy_search",
        json!({
            "economy_id": economy_id,
            "goal": { "sink": { "token": "2" } },
            "candidates": [transfer("1")],
            "max_expansions": 100_000,
            "chunk_size": 1
        }),
        true,
    )
    .await;
    let task_id = endless["taskId"].as_str().unwrap().to_owned();
    call(
        &client,
        &url,
        "tasks/cancel",
        json!({ "taskId": task_id }),
        true,
    )
    .await;
    let cancelled = finished_task(&client, &url, &task_id).await;
    assert!(
        matches!(
            cancelled["status"].as_str(),
            Some("cancelled" | "completed")
        ),
        "{cancelled}"
    );

    let unknown = call(
        &client,
        &url,
        "resources/read",
        json!({ "uri": "axionomy://economy/eco_missing" }),
        false,
    )
    .await;
    assert_eq!(unknown["code"], -32602);

    cancellation.cancel();
}
