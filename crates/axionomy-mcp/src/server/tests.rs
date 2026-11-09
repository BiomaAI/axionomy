use super::*;
use crate::resources::Address;
use rmcp::model::{
    ArgumentInfo, CompletionContext, CompletionInfo, Reference, TaskPayload, TaskStatus,
};
use serde_json::{Value, json};

struct Outcome {
    value: Value,
    is_error: bool,
    links: Vec<String>,
}

fn outcome(result: impl IntoCallToolResult) -> Outcome {
    let CallToolResponse::Complete(result) = result.into_call_tool_result().unwrap() else {
        panic!("a tool returned something other than a complete result")
    };
    Outcome {
        value: result.structured_content.unwrap(),
        is_error: result.is_error == Some(true),
        links: result
            .content
            .iter()
            .filter_map(|block| match block {
                ContentBlock::ResourceLink(resource) => Some(resource.uri.clone()),
                _ => None,
            })
            .collect(),
    }
}

fn fixture() -> Value {
    json!({
        "accounts": { "source": { "token": "5" }, "sink": {} },
        "rates": {
            "transfer": {
                "consume": { "giver": { "token": "1" } },
                "produce": { "receiver": { "token": "1" } },
                "distinct": [["giver", "receiver"]]
            }
        },
        "invariants": { "supply": { "token": 1 } }
    })
}

fn transfer(units: u64) -> paths::Exchange {
    serde_json::from_value(json!({
        "rate": "transfer",
        "bindings": { "giver": "source", "receiver": "sink" },
        "units": units.to_string()
    }))
    .unwrap()
}

async fn create(server: &AxionomyMcp, entries: Value) -> Outcome {
    outcome(
        server
            .economy_create(Parameters(EconomyCreateRequest {
                entries: serde_json::from_value(entries).unwrap(),
            }))
            .await,
    )
}

async fn extend(server: &AxionomyMcp, economy_id: &EconomyId, entries: Value) -> Outcome {
    outcome(
        server
            .economy_extend(Parameters(EconomyExtendRequest {
                economy_id: economy_id.clone(),
                entries: serde_json::from_value(entries).unwrap(),
            }))
            .await,
    )
}

async fn apply(server: &AxionomyMcp, economy_id: &EconomyId, exchange: paths::Exchange) -> Outcome {
    outcome(
        server
            .exchange_apply(Parameters(ExchangeRequest {
                economy_id: economy_id.clone(),
                exchange,
            }))
            .await,
    )
}

async fn replay(
    server: &AxionomyMcp,
    economy_id: &EconomyId,
    exchanges: Vec<paths::Exchange>,
) -> Outcome {
    outcome(
        server
            .trace_replay(Parameters(ReplayRequest {
                economy_id: economy_id.clone(),
                exchanges,
            }))
            .await,
    )
}

async fn read(server: &AxionomyMcp, uri: &str) -> Value {
    let text = server.read(&Address::parse(uri).unwrap()).await.unwrap();
    serde_json::from_str(&text).unwrap()
}

fn id(outcome: &Outcome) -> EconomyId {
    assert!(!outcome.is_error, "{}", outcome.value);
    outcome.value["economy_id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap()
}

/// A well-formed handle no store issued.
fn missing() -> EconomyId {
    EconomyId::from_digest(&blake3::hash(b"never stored"))
}

fn uri(target: Target) -> String {
    target.uri()
}

async fn complete(
    server: &AxionomyMcp,
    argument: &str,
    value: &str,
    context: &[(&str, &str)],
) -> Result<CompletionInfo, McpError> {
    let arguments = context
        .iter()
        .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
        .collect();
    let request = CompleteRequestParams::new(
        Reference::for_resource(
            "axionomy://economy/{economy_id}/accounts/{account}/balances{?prefix,cursor}",
        ),
        ArgumentInfo::new(argument, value),
    )
    .with_context(CompletionContext::with_arguments(arguments));
    server
        .completion(request)
        .await
        .map(|result| result.completion)
}

#[test]
fn tools_declare_schemas_and_behavior_hints() {
    let tools = AxionomyMcp::default().tool_router.list_all();
    assert_eq!(tools.len(), 8);
    for tool in tools {
        assert!(tool.output_schema.is_some(), "{} output schema", tool.name);
        let hints = tool
            .annotations
            .as_ref()
            .expect("every tool declares hints");
        assert_eq!(hints.open_world_hint, Some(false), "{}", tool.name);
        let read_only = matches!(
            tool.name.as_ref(),
            "axionomy_exchange_assess" | "axionomy_problem_catalog"
        );
        assert_eq!(
            hints.read_only_hint.unwrap_or(false),
            read_only,
            "{}",
            tool.name
        );
        if !read_only {
            assert_eq!(hints.destructive_hint, Some(false), "{}", tool.name);
            assert_eq!(hints.idempotent_hint, Some(true), "{}", tool.name);
        }
    }
}

#[tokio::test]
async fn authoring_adds_entries_without_replacing_any() {
    let server = AxionomyMcp::default();
    let created = create(&server, fixture()).await;
    let economy_id = id(&created);
    assert_eq!(created.value["deduplicated"], false);
    assert_eq!(
        created.value["counts"],
        json!({ "accounts": 2, "assets": 1, "rates": 1, "invariants": 1 })
    );
    assert_eq!(
        created.links,
        [uri(Target::Economy {
            economy_id: economy_id.clone()
        })]
    );

    let again = create(&server, fixture()).await;
    assert_eq!(id(&again), economy_id);
    assert_eq!(again.value["deduplicated"], true);

    let extended = extend(
        &server,
        &economy_id,
        json!({
            "accounts": { "source": { "gem": "2" }, "third": {} },
            "rates": { "transfer": { "consume": { "giver": { "gem": "1" } } } },
            "invariants": { "supply": { "gem": 3 } }
        }),
    )
    .await;
    let extended_id = id(&extended);
    assert_eq!(extended.value["parent_id"], economy_id.as_str());
    assert_eq!(
        extended.value["counts"],
        json!({ "accounts": 3, "assets": 2, "rates": 1, "invariants": 1 })
    );

    let conflict = extend(
        &server,
        &economy_id,
        json!({ "accounts": { "source": { "token": "1" } } }),
    )
    .await;
    assert!(conflict.is_error);
    assert_eq!(conflict.value["error"], "conflict");
    let conflict = extend(
        &server,
        &extended_id,
        json!({ "invariants": { "supply": { "gem": 1 } } }),
    )
    .await;
    assert_eq!(conflict.value["error"], "conflict");

    let source = Target::Balances {
        economy_id: economy_id.clone(),
        account: "source".parse().unwrap(),
        prefix: None,
    };
    assert_eq!(
        read(&server, &source.uri()).await["items"],
        json!([{ "asset": "token", "quantity": "5" }])
    );
    let amounts = read(
        &server,
        &uri(Target::RateAmounts {
            economy_id: extended_id,
            rate: "transfer".parse().unwrap(),
        }),
    )
    .await;
    assert_eq!(amounts["items"].as_array().unwrap().len(), 3);

    let missing = extend(&server, &missing(), json!({})).await;
    assert_eq!(missing.value["error"], "not_found");
}

#[tokio::test]
async fn one_call_adds_at_most_ten_thousand_entries() {
    let server = AxionomyMcp::default();
    let assets: serde_json::Map<String, Value> = (0..crate::authoring::MAX_ENTRIES)
        .map(|index| (format!("a{index}"), json!("1")))
        .collect();
    let refused = create(&server, json!({ "accounts": { "holder": assets } })).await;
    assert!(refused.is_error);
    assert_eq!(refused.value["error"], "too_large");
}

#[tokio::test]
async fn apply_and_replay_share_handles_and_keep_their_source() {
    let server = AxionomyMcp::default();
    let source_id = id(&create(&server, fixture()).await);

    let applied = apply(&server, &source_id, transfer(2)).await;
    let applied_id = id(&applied);
    assert_eq!(applied.value["changes"], 2);
    assert_eq!(applied.links.len(), 3);
    let replayed = replay(&server, &source_id, vec![transfer(2)]).await;
    assert_eq!(id(&replayed), applied_id);
    assert_eq!(replayed.value["deduplicated"], true);

    let changes = read(
        &server,
        &uri(Target::Changes {
            economy_id: applied_id.clone(),
        }),
    )
    .await;
    assert_eq!(
        changes["items"],
        json!([
            { "step": 0, "account": "sink", "asset": "token", "kind": "produced", "quantity": "2" },
            { "step": 0, "account": "source", "asset": "token", "kind": "consumed", "quantity": "2" }
        ])
    );
    let exchanges = read(
        &server,
        &uri(Target::Exchanges {
            economy_id: applied_id.clone(),
        }),
    )
    .await;
    assert_eq!(exchanges["items"].as_array().unwrap().len(), 1);
    let summary = read(
        &server,
        &uri(Target::Economy {
            economy_id: applied_id,
        }),
    )
    .await;
    assert_eq!(
        summary["origin"],
        json!({ "kind": "exchanged", "exchanges": 1 })
    );

    let source = Target::Balances {
        economy_id: source_id,
        account: "source".parse().unwrap(),
        prefix: None,
    };
    assert_eq!(
        read(&server, &source.uri()).await["items"][0]["quantity"],
        "5"
    );
}

#[tokio::test]
async fn failures_explain_themselves_and_store_nothing() {
    let server = AxionomyMcp::default();
    let source_id = id(&create(&server, fixture()).await);

    let infeasible = apply(&server, &source_id, transfer(6)).await;
    assert!(infeasible.is_error);
    assert_eq!(infeasible.value["error"], "infeasible");
    assert_eq!(
        infeasible.value["details"]["preview"],
        json!([{ "account": "source", "asset": "token", "kind": "missing", "quantity": "1" }])
    );
    let assessment = read(&server, &infeasible.links[0]).await;
    assert_eq!(assessment["status"], "Infeasible");

    let failed = replay(&server, &source_id, vec![transfer(2), transfer(4)]).await;
    assert_eq!(failed.value["details"]["step"], 1);
    assert!(
        failed.value["message"]
            .as_str()
            .unwrap()
            .contains("exchange 1 of 2")
    );
    let assessment = read(&server, &failed.links[0]).await;
    assert_eq!(assessment["after_exchanges"], 1);

    let unbound: paths::Exchange = serde_json::from_value(json!({
        "rate": "transfer",
        "bindings": { "giver": "source", "receiver": "nobody" },
        "units": "1"
    }))
    .unwrap();
    let invalid = apply(&server, &source_id, unbound).await;
    assert_eq!(invalid.value["error"], "invalid");
    assert!(
        !invalid.value["details"]["issues"]
            .as_array()
            .unwrap()
            .is_empty()
    );

    let assessed = outcome(
        server
            .exchange_assess(Parameters(ExchangeRequest {
                economy_id: source_id,
                exchange: transfer(2),
            }))
            .await,
    );
    assert_eq!(assessed.value["status"], "Applicable");
    assert_eq!(assessed.value["lines"], 2);
    assert_eq!(assessed.value["preview"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn no_result_or_page_grows_with_the_economy() {
    const ASSETS: usize = 100_000;
    const CHUNK: usize = crate::authoring::MAX_ENTRIES - 1;
    let server = AxionomyMcp::default();
    let mut economy_id = id(&create(
        &server,
        json!({
            "accounts": { "store": {} },
            "rates": {
                "ship": {
                    "consume": { "from": { "sku/000000": "1" } },
                    "produce": { "to": { "sku/000000": "1" } },
                    "distinct": [["from", "to"]]
                }
            }
        }),
    )
    .await);
    for start in (0..ASSETS).step_by(CHUNK) {
        let balances: serde_json::Map<String, Value> = (start..(start + CHUNK).min(ASSETS))
            .map(|index| (format!("sku/{index:06}"), json!("3")))
            .collect();
        let extended = extend(
            &server,
            &economy_id,
            json!({ "accounts": { "warehouse": balances } }),
        )
        .await;
        assert!(extended.value.to_string().len() < 512, "{}", extended.value);
        economy_id = id(&extended);
    }

    let balances = Target::Balances {
        economy_id: economy_id.clone(),
        account: "warehouse".parse().unwrap(),
        prefix: None,
    };
    let mut seen = std::collections::HashSet::new();
    let mut page_uri = balances.uri();
    loop {
        let page = read(&server, &page_uri).await;
        let items = page["items"].as_array().unwrap();
        assert!(items.len() <= resources::PAGE_ROWS);
        seen.extend(
            items
                .iter()
                .map(|item| item["asset"].as_str().unwrap().to_owned()),
        );
        let Some(cursor) = page["nextCursor"].as_str() else {
            break;
        };
        page_uri = balances.page_uri(cursor);
    }
    assert_eq!(seen.len(), ASSETS);

    let filtered = Target::Balances {
        economy_id: economy_id.clone(),
        account: "warehouse".parse().unwrap(),
        prefix: Some("sku/0999".into()),
    };
    let filtered = read(&server, &filtered.uri()).await;
    assert_eq!(filtered["items"].as_array().unwrap().len(), 100);
    assert!(filtered.get("nextCursor").is_none());

    let shipped = apply(
        &server,
        &economy_id,
        serde_json::from_value(json!({
            "rate": "ship",
            "bindings": { "from": "warehouse", "to": "store" },
            "units": "2"
        }))
        .unwrap(),
    )
    .await;
    assert!(shipped.value.to_string().len() < 512, "{}", shipped.value);
    assert_eq!(shipped.value["changes"], 2);

    let suggestions = complete(
        &server,
        "prefix",
        "sku/",
        &[
            ("economy_id", economy_id.as_str()),
            ("account", "warehouse"),
        ],
    )
    .await
    .unwrap();
    assert_eq!(suggestions.values.len(), CompletionInfo::MAX_VALUES);
    assert_eq!(suggestions.total, Some(ASSETS as u32));
    assert_eq!(suggestions.has_more, Some(true));
}

#[tokio::test]
async fn bad_cursors_and_unknown_resources_are_refused() {
    let server = AxionomyMcp::default();
    let economy_id = id(&create(&server, fixture()).await);
    let foreign = Target::Accounts {
        economy_id: economy_id.clone(),
        prefix: None,
    }
    .page_uri("1.00000000");
    assert!(
        server
            .read(&Address::parse(&foreign).unwrap())
            .await
            .is_err()
    );
    let missing = Target::Economy {
        economy_id: missing(),
    }
    .uri();
    assert!(
        server
            .read(&Address::parse(&missing).unwrap())
            .await
            .is_err()
    );
    let account = Target::Balances {
        economy_id,
        account: "nobody".parse().unwrap(),
        prefix: None,
    }
    .uri();
    assert!(
        server
            .read(&Address::parse(&account).unwrap())
            .await
            .is_err()
    );
}

#[tokio::test]
async fn completion_needs_the_arguments_it_depends_on() {
    let server = AxionomyMcp::default();
    let economy_id = id(&create(&server, fixture()).await);
    let accounts = complete(
        &server,
        "account",
        "s",
        &[("economy_id", economy_id.as_str())],
    )
    .await
    .unwrap();
    assert_eq!(accounts.values, ["sink", "source"]);
    assert!(complete(&server, "account", "s", &[]).await.is_err());
}

async fn terminal_task(tasks: &TaskManager, task_id: &str) -> rmcp::model::DetailedTask {
    for _ in 0..500 {
        let task = tasks.get_task(task_id).unwrap();
        if task.status().is_terminal() {
            return task;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    panic!("task did not reach a terminal state")
}

fn completed(task: rmcp::model::DetailedTask) -> Value {
    assert_eq!(task.status(), TaskStatus::Completed);
    let TaskPayload::Completed { result } = task.payload else {
        panic!("the task should complete with a tool result")
    };
    let result: CallToolResult = serde_json::from_value(Value::Object(result)).unwrap();
    result.structured_content.unwrap()
}

async fn search_task(server: &AxionomyMcp, max_expansions: usize) -> (TaskManager, String) {
    let economy_id = id(&create(
        server,
        json!({
            "accounts": { "source": { "token": "1" }, "sink": {} },
            "rates": fixture()["rates"]
        }),
    )
    .await);
    let request: SearchRequest = serde_json::from_value(json!({
        "economy_id": economy_id,
        "goal": { "sink": { "token": "1" } },
        "candidates": [transfer(1)],
        "max_expansions": max_expansions,
        "chunk_size": 1
    }))
    .unwrap();
    let source = server.snapshot(&economy_id).await.unwrap();
    let goal = paths::Goal::new().require(
        "sink".parse().unwrap(),
        [("token".parse().unwrap(), axionomy::Quantity::new(1))].into(),
    );
    let tasks = TaskManager::new();
    let snapshots = server.snapshots.clone();
    let task = tasks.spawn(TaskOptions::new(), move |context| {
        Box::pin(run_search(context, snapshots, source, request, goal))
    });
    (tasks, task.task_id)
}

#[tokio::test]
async fn a_search_solution_is_the_snapshot_it_reaches() {
    let server = AxionomyMcp::default();
    let (tasks, task_id) = search_task(&server, 8).await;
    let response = completed(terminal_task(&tasks, &task_id).await);
    assert_eq!(response["outcome"], "solved");
    let solution = response["solution"]["economy_id"].as_str().unwrap();
    let trace = read(
        &server,
        &uri(Target::Exchanges {
            economy_id: solution.parse().unwrap(),
        }),
    )
    .await;
    assert_eq!(trace["items"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn a_search_observes_cancellation() {
    let server = AxionomyMcp::default();
    let (tasks, task_id) = search_task(&server, 100_000).await;
    tasks.cancel_task(&task_id).unwrap();
    let task = terminal_task(&tasks, &task_id).await;
    assert!(matches!(
        task.status(),
        TaskStatus::Cancelled | TaskStatus::Completed
    ));
}

#[tokio::test]
async fn a_problem_run_returns_a_summary_and_pages_its_frames() {
    let server = AxionomyMcp::default();
    let request = RunRequest::new("maze")
        .with_instance("micro")
        .with_strategy("a_star");
    let tasks = TaskManager::new();
    let artifacts = Arc::clone(&server.artifacts);
    let task_request = request.clone();
    let task = tasks.spawn(TaskOptions::new(), move |context| {
        Box::pin(run_problem(context, artifacts, task_request))
    });
    let summary = completed(terminal_task(&tasks, &task.task_id).await);
    let artifact_id = summary["artifact_id"].as_str().unwrap().to_owned();
    let served = server.artifact(&artifact_id).unwrap();
    assert_eq!(*served, ReferenceService.run(request).unwrap());
    let document_id = summary["selected_document_id"].as_str().unwrap().to_owned();
    let frames = summary["documents"]
        .as_array()
        .unwrap()
        .iter()
        .find(|document| document["id"] == document_id.as_str())
        .unwrap()["frames"]
        .as_u64()
        .unwrap();

    let first = read(
        &server,
        &uri(Target::Frames {
            artifact_id: artifact_id.clone(),
            document_id: document_id.clone(),
        }),
    )
    .await;
    assert_eq!(
        first["items"].as_array().unwrap().len() as u64,
        frames.min(resources::PAGE_FRAMES as u64)
    );
    let document = read(
        &server,
        &uri(Target::Document {
            artifact_id,
            document_id,
        }),
    )
    .await;
    assert_eq!(document["frames"], frames);
    assert_eq!(document["document"]["frames"], json!([]));
}
