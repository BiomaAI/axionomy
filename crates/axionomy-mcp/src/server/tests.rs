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
    assert_eq!(tools.len(), 9);
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
    let source = server.economy(&economy_id).await.unwrap();
    let goal = paths::Goal::new().require(
        "sink".parse().unwrap(),
        [("token".parse().unwrap(), axionomy::Quantity::new(1))].into(),
    );
    let tasks = TaskManager::new();
    let archive = server.archive.clone();
    let task = tasks.spawn(TaskOptions::new(), move |context| {
        Box::pin(run_search(context, archive, source, request, goal))
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
    let archive = server.archive.clone();
    let task_request = request.clone();
    let task = tasks.spawn(TaskOptions::new(), move |context| {
        Box::pin(run_problem(context, archive, task_request))
    });
    let summary = completed(terminal_task(&tasks, &task.task_id).await);
    let artifact_id = summary["artifact_id"].as_str().unwrap().to_owned();
    let served = server.artifact(&artifact_id).await.unwrap().unwrap();
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

#[tokio::test]
async fn exchanges_open_and_close_accounts_named_by_their_paths() {
    let server = AxionomyMcp::default();
    let economy_id = id(&create(
        &server,
        json!({
            "accounts": { "treasury": { "cash": "10" } },
            "rates": {
                "agents/spawn": {
                    "consume": { "treasury": { "cash": "3" } },
                    "produce": { "agent": { "cash": "3" } },
                    "opens": ["agent"]
                },
                "agents/retire": {
                    "consume": { "agent": { "cash": "3" } },
                    "produce": { "treasury": { "cash": "3" } },
                    "closes": ["agent"]
                }
            },
            "invariants": { "cash": { "cash": 1 } }
        }),
    )
    .await);
    let exchange = |rate: &str, agent: &str| -> paths::Exchange {
        serde_json::from_value(json!({
            "rate": rate,
            "bindings": { "treasury": "treasury", "agent": agent },
            "units": "1"
        }))
        .unwrap()
    };

    let spawned = apply(
        &server,
        &economy_id,
        exchange("agents/spawn", "agents/7/wallet"),
    )
    .await;
    assert_eq!(spawned.value["opened"], json!(["agents/7/wallet"]));
    let spawned_id = id(&spawned);
    let wallet = read(
        &server,
        &uri(Target::Balances {
            economy_id: spawned_id.clone(),
            account: "agents/7/wallet".parse().unwrap(),
            prefix: None,
        }),
    )
    .await;
    assert_eq!(
        wallet["items"],
        json!([{ "asset": "cash", "quantity": "3" }])
    );

    let again = apply(
        &server,
        &spawned_id,
        exchange("agents/spawn", "agents/7/wallet"),
    )
    .await;
    assert_eq!(again.value["error"], "invalid");
    assert_eq!(
        again.value["details"]["issues"][0]["AccountExists"]["account"],
        "agents/7/wallet"
    );

    let retired = apply(
        &server,
        &spawned_id,
        exchange("agents/retire", "agents/7/wallet"),
    )
    .await;
    assert_eq!(retired.value["closed"], json!(["agents/7/wallet"]));
    let retired_id = id(&retired);
    let accounts = read(
        &server,
        &uri(Target::Accounts {
            economy_id: retired_id.clone(),
            prefix: None,
        }),
    )
    .await;
    assert_eq!(
        accounts["items"],
        json!([{ "account": "treasury", "assets": 1 }])
    );

    let reopened = apply(
        &server,
        &retired_id,
        exchange("agents/spawn", "agents/7/wallet"),
    )
    .await;
    assert_eq!(reopened.value["opened"], json!(["agents/7/wallet"]));

    let rich = id(&extend(
        &server,
        &spawned_id,
        json!({ "accounts": { "agents/7/wallet": { "gem": "1" } } }),
    )
    .await);
    let stuck = apply(&server, &rich, exchange("agents/retire", "agents/7/wallet")).await;
    assert_eq!(stuck.value["error"], "invalid");
    assert_eq!(
        stuck.value["details"]["issues"][0]["ClosedNotEmpty"]["remaining"],
        json!([["gem", "1"]])
    );
}

async fn step(server: &AxionomyMcp, economy_id: &EconomyId, step: usize) -> Outcome {
    outcome(
        server
            .trace_step(Parameters(TraceStepRequest {
                economy_id: economy_id.clone(),
                step,
            }))
            .await,
    )
}

#[tokio::test]
async fn stepping_through_a_trace_names_each_state_and_diffs_compare_them() {
    let server = AxionomyMcp::default();
    let parent = create(&server, fixture()).await;
    let parent_id = id(&parent);
    let replayed = replay(
        &server,
        &parent_id,
        vec![transfer(1), transfer(2), transfer(1)],
    )
    .await;
    let replayed_id = id(&replayed);

    let mut states = Vec::new();
    for index in 0..=3 {
        let stepped = step(&server, &replayed_id, index).await;
        assert!(!stepped.is_error, "{}", stepped.value);
        assert_eq!(stepped.value["exchanges"], index);
        assert_eq!(stepped.value["source_economy_id"], parent_id.as_str());
        states.push(stepped);
    }
    // The ends of a trace are the states it connects.
    assert_eq!(states[0].value["root"], parent.value["root"]);
    assert_eq!(states[3].value["root"], replayed.value["root"]);
    assert_eq!(step(&server, &replayed_id, 2).await.value, {
        let mut again = states[2].value.clone();
        again["deduplicated"] = json!(true);
        again
    });

    let diff = read(
        &server,
        &uri(Target::Diff {
            economy_id: id(&states[1]),
            other_id: id(&states[3]),
        }),
    )
    .await;
    assert_eq!(
        diff["items"],
        json!([
            { "kind": "balance", "account": "sink", "asset": "token", "before": "1", "after": "4" },
            { "kind": "balance", "account": "source", "asset": "token", "before": "4", "after": "1" }
        ])
    );
    let same = read(
        &server,
        &uri(Target::Diff {
            economy_id: id(&states[3]),
            other_id: replayed_id.clone(),
        }),
    )
    .await;
    assert_eq!(same["items"], json!([]));

    assert!(step(&server, &replayed_id, 4).await.is_error);
    assert!(step(&server, &parent_id, 0).await.is_error);
}

#[tokio::test]
async fn a_store_on_disk_keeps_snapshots_and_handles_across_restarts() {
    let directory = std::env::temp_dir().join(format!("axionomy-mcp-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let file = directory.join("snapshots.redb");
    let _ = std::fs::remove_file(&file);
    let (parent_id, applied) = {
        let server = AxionomyMcp::new(axionomy_store::Store::open(&file).unwrap()).unwrap();
        let parent_id = id(&create(&server, fixture()).await);
        let applied = apply(&server, &parent_id, transfer(2)).await;
        (parent_id, applied)
    };
    let server = AxionomyMcp::new(axionomy_store::Store::open(&file).unwrap()).unwrap();
    let economy = read(
        &server,
        &uri(Target::Economy {
            economy_id: id(&applied),
        }),
    )
    .await;
    assert_eq!(economy["root"], applied.value["root"]);
    assert_eq!(economy["parent_id"], parent_id.as_str());
    let changes = read(
        &server,
        &uri(Target::Changes {
            economy_id: id(&applied),
        }),
    )
    .await;
    assert_eq!(changes["items"].as_array().unwrap().len(), 2);
    // The handle secret survived, so the same operation names the same snapshot.
    let again = apply(&server, &parent_id, transfer(2)).await;
    assert_eq!(id(&again), id(&applied));
    assert_eq!(again.value["deduplicated"], true);
    drop(server);
    std::fs::remove_dir_all(&directory).unwrap();
}
