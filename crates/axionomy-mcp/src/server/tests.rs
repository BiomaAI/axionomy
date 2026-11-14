use super::*;
use axionomy_store::Store;

/// A server that keeps its economies in memory.
fn server() -> AxionomyMcp {
    AxionomyMcp::new(Engine::new(Store::memory().unwrap()).unwrap()).unwrap()
}
use crate::{handles::RefId, resources::Address};

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
            "axionomy://economy/{economy_id}/accounts/{+account}{?prefix,cursor}",
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
    let tools = server().tool_router.list_all();
    assert_eq!(tools.len(), 17);
    for tool in tools {
        assert!(tool.output_schema.is_some(), "{} output schema", tool.name);
        let hints = tool
            .annotations
            .as_ref()
            .expect("every tool declares hints");
        assert_eq!(hints.open_world_hint, Some(false), "{}", tool.name);
        let read_only = matches!(
            tool.name.as_ref(),
            "axionomy_exchange_assess" | "axionomy_ref_read"
        );
        assert_eq!(
            hints.read_only_hint.unwrap_or(false),
            read_only,
            "{}",
            tool.name
        );
        if !read_only {
            // Deleting a ref is the one call that takes something away.
            let destructive = tool.name == "axionomy_ref_delete";
            assert_eq!(hints.destructive_hint, Some(destructive), "{}", tool.name);
            // Every call makes a ref, or records runs, of its own.
            let idempotent = !matches!(
                tool.name.as_ref(),
                "axionomy_ref_create" | "axionomy_experiment_run"
            );
            assert_eq!(hints.idempotent_hint, Some(idempotent), "{}", tool.name);
        }
    }
}

#[tokio::test]
async fn authoring_adds_entries_without_replacing_any() {
    let server = server();
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
    let server = server();
    let assets: serde_json::Map<String, Value> = (0..crate::authoring::MAX_ENTRIES)
        .map(|index| (format!("a{index}"), json!("1")))
        .collect();
    let refused = create(&server, json!({ "accounts": { "holder": assets } })).await;
    assert!(refused.is_error);
    assert_eq!(refused.value["error"], "too_large");
}

#[tokio::test]
async fn apply_and_replay_share_handles_and_keep_their_source() {
    let server = server();
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
            { "exchange_index": 0, "account": "sink", "asset": "token", "kind": "produced", "quantity": "2" },
            { "exchange_index": 0, "account": "source", "asset": "token", "kind": "consumed", "quantity": "2" }
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
    let server = server();
    let source_id = id(&create(&server, fixture()).await);

    let infeasible = apply(&server, &source_id, transfer(6)).await;
    assert!(infeasible.is_error);
    assert_eq!(infeasible.value["error"], "infeasible");
    assert_eq!(
        infeasible.value["message"],
        "the exchange is infeasible: `source` lacks 1 `token`; nothing was stored"
    );
    assert_eq!(
        infeasible.value["details"]["preview"],
        json!([{ "account": "source", "asset": "token", "kind": "missing", "quantity": "1" }])
    );
    let assessment = read(&server, &infeasible.links[0]).await;
    assert_eq!(assessment["status"], "infeasible");

    // Messages count exchanges from one; indices count them from zero.
    let failed = replay(&server, &source_id, vec![transfer(2), transfer(4)]).await;
    assert_eq!(failed.value["details"]["exchange_index"], 1);
    assert_eq!(
        failed.value["message"],
        "exchange 2 of 2 is infeasible: `source` lacks 1 `token`; nothing was stored"
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
    let issue = &invalid.value["details"]["issues"][0];
    assert_eq!(issue["kind"], "missing_account");
    assert_eq!(issue["account"], "nobody");
    assert!(
        issue["message"]
            .as_str()
            .unwrap()
            .contains("has no account `nobody`")
    );

    let assess = |exchange| {
        server.exchange_assess(Parameters(ExchangeRequest {
            economy_id: source_id.clone(),
            exchange,
        }))
    };
    let assessed = outcome(assess(transfer(2)).await);
    assert_eq!(assessed.value["status"], "applicable");
    assert_eq!(assessed.value["lines"], 2);
    assert_eq!(assessed.value["preview"].as_array().unwrap().len(), 2);

    // A rate the economy lacks points to the rates it has.
    let unknown = outcome(
        assess(
            serde_json::from_value(json!({ "rate": "ghost", "bindings": {}, "units": "1" }))
                .unwrap(),
        )
        .await,
    );
    assert_eq!(unknown.value["status"], "invalid");
    assert_eq!(unknown.value["issues"][0]["kind"], "missing_rate");
    assert_eq!(unknown.value["issues"][0]["rate"], "ghost");
    let rates = uri(Target::Rates {
        economy_id: source_id.clone(),
        asset: None,
    });
    assert!(
        unknown.value["issues"][0]["message"]
            .as_str()
            .unwrap()
            .ends_with(&rates)
    );
}

#[tokio::test]
async fn no_result_or_page_grows_with_the_economy() {
    const ASSETS: usize = 100_000;
    const CHUNK: usize = crate::authoring::MAX_ENTRIES - 1;
    let server = server();
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
    let server = server();
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
        economy_id: economy_id.clone(),
        account: "nobody".parse().unwrap(),
        prefix: None,
    }
    .uri();
    let error = server
        .read(&Address::parse(&account).unwrap())
        .await
        .unwrap_err();
    assert!(
        error
            .message
            .starts_with(&format!("economy `{economy_id}` has no account `nobody`")),
        "{}",
        error.message
    );
    let rate = Target::Rate {
        economy_id: economy_id.clone(),
        rate: "ghost".parse().unwrap(),
    };
    let error = server
        .read(&Address::parse(&rate.uri()).unwrap())
        .await
        .unwrap_err();
    assert!(
        error.message.contains("has no rate `ghost`"),
        "{}",
        error.message
    );

    // A malformed path says what is wrong with it.
    let error = Address::parse(&format!(
        "axionomy://economy/{economy_id}/accounts/Bad_Path"
    ))
    .unwrap_err();
    assert!(
        error
            .message
            .contains("`Bad_Path` is not an account path: `B`"),
        "{}",
        error.message
    );
}

#[tokio::test]
async fn completion_needs_the_arguments_it_depends_on() {
    let server = server();
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
    let server = server();
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
    let server = server();
    let (tasks, task_id) = search_task(&server, 100_000).await;
    tasks.cancel_task(&task_id).unwrap();
    let task = terminal_task(&tasks, &task_id).await;
    assert!(matches!(
        task.status(),
        TaskStatus::Cancelled | TaskStatus::Completed
    ));
}

fn handle<T: std::str::FromStr<Err: std::fmt::Debug>>(value: &Value) -> T {
    value.as_str().unwrap().parse().unwrap()
}

#[tokio::test]
async fn exchanges_open_and_close_accounts_named_by_their_paths() {
    let server = server();
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
        again.value["details"]["issues"][0]["kind"],
        "account_exists"
    );
    assert_eq!(
        again.value["details"]["issues"][0]["account"],
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
        stuck.value["details"]["issues"][0]["kind"],
        "closed_not_empty"
    );
    assert_eq!(
        stuck.value["details"]["issues"][0]["remaining"],
        json!({ "gem": "1" })
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
    let server = server();
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
        assert_eq!(stepped.value["step"], index);
        assert_eq!(stepped.value["exchanges"], index);
        // Step zero is the trace's source; the economy passed in is its last.
        assert_eq!(stepped.value["source_economy_id"], parent_id.as_str());
        assert_eq!(stepped.value["traced_economy_id"], replayed_id.as_str());
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

    let beyond = step(&server, &replayed_id, 4).await;
    assert_eq!(beyond.value["error"], "invalid_request");
    assert_eq!(beyond.value["details"]["pointer"], "/step");
    let authored = step(&server, &parent_id, 0).await;
    assert_eq!(authored.value["details"]["pointer"], "/economy_id");
}

async fn diff(server: &AxionomyMcp, arguments: Value) -> Outcome {
    outcome(
        server
            .trace_diff(Parameters(serde_json::from_value(arguments).unwrap()))
            .await,
    )
}

#[tokio::test]
async fn two_traces_from_one_economy_compare_step_by_step() {
    let server = server();
    let source = id(&create(&server, fixture()).await);
    let left = id(&replay(&server, &source, vec![transfer(1), transfer(2)]).await);
    let right = id(&apply(&server, &source, transfer(3)).await);
    let compared = diff(
        &server,
        json!({ "left_economy_id": left, "right_economy_id": right }),
    )
    .await;
    assert!(!compared.is_error, "{}", compared.value);
    assert_eq!(
        compared.value,
        json!({
            "left_economy_id": left,
            "right_economy_id": right,
            "shared_economy_id": source,
            "left_exchanges": 2,
            "right_exchanges": 1,
            "steps": [
                {
                    "step": 1,
                    "left_exchange": serde_json::to_value(transfer(1)).unwrap(),
                    "right_exchange": serde_json::to_value(transfer(3)).unwrap(),
                    "balances": [
                        { "account": "sink", "asset": "token", "left": "1", "right": "3" },
                        { "account": "source", "asset": "token", "left": "4", "right": "2" }
                    ]
                },
                {
                    "step": 2,
                    "left_exchange": serde_json::to_value(transfer(2)).unwrap(),
                    "balances": []
                }
            ],
            // Two transfers make what one of their sum made.
            "converged_step": 2
        })
    );
    assert_eq!(
        compared.links,
        [
            uri(Target::Economy {
                economy_id: source.clone()
            }),
            uri(Target::Diff {
                economy_id: left.clone(),
                other_id: right.clone(),
            }),
        ]
    );

    // A step at a time, paged on from the last listed.
    let first = diff(
        &server,
        json!({ "left_economy_id": left, "right_economy_id": right, "limit": 1 }),
    )
    .await;
    assert_eq!(first.value["steps"].as_array().unwrap().len(), 1);
    assert_eq!(first.value["next_after_step"], 1);
    let rest = diff(
        &server,
        json!({ "left_economy_id": left, "right_economy_id": right, "limit": 1, "after_step": 1 }),
    )
    .await;
    assert_eq!(rest.value["steps"][0]["step"], 2);
    assert!(rest.value.get("next_after_step").is_none());
    let wide = diff(
        &server,
        json!({ "left_economy_id": left, "right_economy_id": right, "limit": 101 }),
    )
    .await;
    assert_eq!(wide.value["details"]["pointer"], "/limit");

    // Economies authored apart share no trace; an objective no package
    // carries is not found.
    let apart = id(&extend(&server, &source, json!({ "accounts": { "other": {} } })).await);
    let unrelated = diff(
        &server,
        json!({ "left_economy_id": source, "right_economy_id": apart }),
    )
    .await;
    assert_eq!(unrelated.value["error"], "invalid_request");
    assert_eq!(unrelated.value["details"]["pointer"], "/right_economy_id");
    let unknown = diff(
        &server,
        json!({ "left_economy_id": left, "right_economy_id": right, "objectives": ["paid"] }),
    )
    .await;
    assert_eq!(unknown.value["error"], "not_found");
    assert_eq!(unknown.value["details"]["pointer"], "/objectives/0");
}

async fn find(
    server: &AxionomyMcp,
    economy_id: &EconomyId,
    account: &str,
    comparison: axionomy::QuantityComparison,
    quantity: u64,
) -> Outcome {
    outcome(
        server
            .trace_find(Parameters(TraceFindRequest {
                economy_id: economy_id.clone(),
                account: account.parse().unwrap(),
                asset: "token".parse().unwrap(),
                comparison,
                quantity: axionomy::Quantity::new(quantity),
            }))
            .await,
    )
}

#[tokio::test]
async fn finding_a_balance_condition_names_the_first_state_it_holds_in() {
    use axionomy::QuantityComparison::{Equal, GreaterThanOrEqual, LessThanOrEqual};

    let server = server();
    let parent_id = id(&create(&server, fixture()).await);
    let replayed_id = id(&replay(
        &server,
        &parent_id,
        vec![transfer(1), transfer(2), transfer(1)],
    )
    .await);
    // The sink holds 0, 1, 3 and 4 tokens; the source 5, 4, 2 and 1.
    for (account, comparison, quantity, expected) in [
        ("sink", GreaterThanOrEqual, 3, Some(2)),
        ("sink", GreaterThanOrEqual, 0, Some(0)),
        ("sink", Equal, 4, Some(3)),
        ("source", LessThanOrEqual, 2, Some(2)),
        ("sink", GreaterThanOrEqual, 5, None),
    ] {
        let found = find(&server, &replayed_id, account, comparison, quantity).await;
        assert!(!found.is_error, "{}", found.value);
        match expected {
            Some(index) => {
                assert_eq!(
                    found.value["step"], index,
                    "{account} {comparison:?} {quantity}"
                );
                let stepped = step(&server, &replayed_id, index).await;
                assert_eq!(found.value["economy"]["step"], index);
                assert_eq!(
                    found.value["economy"]["economy_id"],
                    stepped.value["economy_id"]
                );
            }
            None => {
                assert_eq!(found.value["step"], Value::Null);
                assert!(found.value.get("economy").is_none());
                assert_eq!(
                    found.value["message"],
                    format!(
                        "`sink`'s balance of `token` is at least 5 at none of the 4 steps of the trace that made `{replayed_id}`"
                    )
                );
            }
        }
    }
    // An account no economy of the trace has is not a balance of zero.
    let nobody = find(&server, &replayed_id, "nobody", Equal, 0).await;
    assert_eq!(nobody.value["error"], "not_found");
    assert_eq!(nobody.value["details"]["pointer"], "/account");
    assert!(find(&server, &parent_id, "sink", Equal, 0).await.is_error);
}

#[tokio::test]
async fn finding_counts_an_account_the_trace_opens() {
    let server = server();
    let economy_id = id(&create(
        &server,
        json!({
            "accounts": { "treasury": { "token": "3" } },
            "rates": {
                "spawn": {
                    "consume": { "treasury": { "token": "1" } },
                    "produce": { "agent": { "token": "1" } },
                    "opens": ["agent"]
                }
            }
        }),
    )
    .await);
    let spawned = id(&apply(
        &server,
        &economy_id,
        serde_json::from_value(json!({
            "rate": "spawn",
            "bindings": { "treasury": "treasury", "agent": "agent" },
            "units": "1"
        }))
        .unwrap(),
    )
    .await);
    let found = find(
        &server,
        &spawned,
        "agent",
        axionomy::QuantityComparison::Equal,
        1,
    )
    .await;
    assert_eq!(found.value["step"], 1, "{}", found.value);
}

#[test]
fn malformed_arguments_name_the_argument_at_fault() {
    let economy_id = missing();
    let refused = |tool: &str, arguments: Value| {
        let Value::Object(arguments) = arguments else {
            panic!("arguments are an object")
        };
        let error = check_arguments(tool, Some(&arguments)).unwrap_err();
        assert_eq!(error.failure.error, FailureCode::InvalidRequest);
        let details = error.failure.details.unwrap();
        (
            details["pointer"].as_str().unwrap().to_owned(),
            error.failure.message,
        )
    };
    let exchange = |exchange: Value| json!({ "economy_id": economy_id, "exchange": exchange });
    for (arguments, pointer) in [
        (
            exchange(json!({ "rate": "transfer", "bindings": {} })),
            "/exchange/units",
        ),
        (
            exchange(json!({ "rate": "Transfer", "bindings": {}, "units": "1" })),
            "/exchange/rate",
        ),
        (
            exchange(json!({ "rate": "transfer", "bindings": {}, "units": "-1" })),
            "/exchange/units",
        ),
        (
            exchange(
                json!({ "rate": "transfer", "bindings": { "giver": "Source" }, "units": "1" }),
            ),
            "/exchange/bindings/giver",
        ),
        (
            json!({ "economy_id": "eco-1", "exchange": { "rate": "t", "bindings": {}, "units": "1" } }),
            "/economy_id",
        ),
    ] {
        assert_eq!(refused("axionomy_exchange_apply", arguments).0, pointer);
    }
    let (pointer, message) = refused(
        "axionomy_economy_create",
        json!({ "entries": { "accounts": { "agents/7/Wallet": {} } } }),
    );
    assert_eq!(pointer, "/entries/accounts/agents~17~1Wallet");
    assert!(message.starts_with("argument `/entries/accounts/agents~17~1Wallet` is invalid: `W`"));

    // Every tool reads its arguments before it runs.
    for tool in server().tool_router.list_all() {
        assert_eq!(
            refused(&tool.name, json!({ "surplus": true })).0,
            "/surplus",
            "{}",
            tool.name
        );
    }
}

#[tokio::test]
async fn a_handle_the_store_lacks_says_how_to_make_it_again() {
    let server = server();
    let missing = extend(&server, &missing(), json!({})).await;
    assert_eq!(missing.value["error"], "not_found");
    let message = missing.value["message"].as_str().unwrap();
    assert!(message.contains("in memory"), "{message}");
    assert!(message.contains("axionomy_experiment_run"), "{message}");

    let error = server
        .read(
            &Address::parse(&uri(Target::Economy {
                economy_id: self::missing(),
            }))
            .unwrap(),
        )
        .await
        .unwrap_err();
    assert!(
        error.message.starts_with("economy `eco-"),
        "{}",
        error.message
    );
}

#[tokio::test]
async fn a_store_on_disk_keeps_snapshots_and_handles_across_restarts() {
    let directory = std::env::temp_dir().join(format!("axionomy-mcp-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let file = directory.join("snapshots.redb");
    let _ = std::fs::remove_file(&file);
    let (parent_id, applied, ref_id) = {
        let server =
            AxionomyMcp::new(Engine::new(axionomy_store::Store::open(&file).unwrap()).unwrap())
                .unwrap();
        let parent_id = id(&create(&server, fixture()).await);
        let applied = apply(&server, &parent_id, transfer(2)).await;
        let ref_id = refer(&server, &parent_id).await;
        assert!(
            !advance(&server, &ref_id, &parent_id, &id(&applied))
                .await
                .is_error
        );
        (parent_id, applied, ref_id)
    };
    let server =
        AxionomyMcp::new(Engine::new(axionomy_store::Store::open(&file).unwrap()).unwrap())
            .unwrap();
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
    // So does a ref, at the economy it was moved to.
    assert_eq!(id(&read_ref(&server, &ref_id).await), id(&applied));
    drop(server);
    std::fs::remove_dir_all(&directory).unwrap();
}

async fn refer(server: &AxionomyMcp, economy_id: &EconomyId) -> RefId {
    let created = outcome(
        server
            .ref_create(Parameters(RefCreateRequest {
                economy_id: economy_id.clone(),
            }))
            .await,
    );
    assert_eq!(id(&created), *economy_id);
    handle(&created.value["ref_id"])
}

async fn read_ref(server: &AxionomyMcp, ref_id: &RefId) -> Outcome {
    outcome(
        server
            .ref_read(Parameters(RefReadRequest {
                ref_id: ref_id.clone(),
            }))
            .await,
    )
}

async fn advance(
    server: &AxionomyMcp,
    ref_id: &RefId,
    expected: &EconomyId,
    to: &EconomyId,
) -> Outcome {
    outcome(
        server
            .ref_advance(Parameters(RefAdvanceRequest {
                ref_id: ref_id.clone(),
                expected_economy_id: expected.clone(),
                economy_id: to.clone(),
            }))
            .await,
    )
}

async fn delete(server: &AxionomyMcp, ref_id: &RefId, expected: &EconomyId) -> Outcome {
    outcome(
        server
            .ref_delete(Parameters(RefDeleteRequest {
                ref_id: ref_id.clone(),
                expected_economy_id: expected.clone(),
            }))
            .await,
    )
}

async fn follow(
    server: &AxionomyMcp,
    ref_id: &RefId,
    after: &EconomyId,
    limit: usize,
    wait_seconds: u64,
    goal: Option<Value>,
) -> Outcome {
    outcome(
        server
            .ref_frames(Parameters(RefFramesRequest {
                ref_id: ref_id.clone(),
                after_economy_id: after.clone(),
                limit,
                wait_seconds,
                goal: goal.map(|goal| serde_json::from_value(goal).unwrap()),
            }))
            .await,
    )
}

/// The economies a follower's frames made, in order.
fn delivered(followed: &Outcome) -> Vec<EconomyId> {
    assert!(!followed.is_error, "{}", followed.value);
    followed.value["frames"]
        .as_array()
        .unwrap()
        .iter()
        .map(|frame| handle(&frame["economy_id"]))
        .collect()
}

#[tokio::test]
async fn a_ref_moves_only_forward_from_where_its_mover_expects_it() {
    let server = server();
    let start = id(&create(&server, fixture()).await);
    let live = refer(&server, &start).await;
    assert_eq!(id(&read_ref(&server, &live).await), start);
    // Every call makes a ref of its own.
    assert_ne!(refer(&server, &start).await, live);

    // Two proposers from the same economy: the first moves the ref, the
    // second is told where it went and looks again from there.
    let mine = id(&apply(&server, &start, transfer(1)).await);
    let theirs = id(&apply(&server, &start, transfer(2)).await);
    let moved = advance(&server, &live, &start, &mine).await;
    assert_eq!(id(&moved), mine);
    assert_eq!(moved.value["ref_id"], live.as_str());
    // A mover that asks again for the move it made is told it was made.
    assert_eq!(id(&advance(&server, &live, &start, &mine).await), mine);
    let refused = advance(&server, &live, &start, &theirs).await;
    assert!(refused.is_error);
    assert_eq!(refused.value["error"], "moved");
    assert_eq!(
        refused.value["details"],
        json!({ "current_economy_id": mine })
    );
    assert_eq!(id(&read_ref(&server, &live).await), mine);
    let again = id(&apply(&server, &mine, transfer(2)).await);
    assert_eq!(id(&advance(&server, &live, &mine, &again).await), again);

    // Nor does it move back, or onto an economy not made from where it is.
    for to in [&start, &theirs] {
        let refused = advance(&server, &live, &again, to).await;
        assert_eq!(refused.value["error"], "invalid_request");
        assert_eq!(refused.value["details"]["pointer"], "/economy_id");
        let message = refused.value["message"].as_str().unwrap();
        assert!(message.contains("moves only forward"), "{message}");
    }

    // A follower follows from an economy the ref was at or moved over.
    let elsewhere = follow(&server, &live, &theirs, 10, 0, None).await;
    assert_eq!(elsewhere.value["error"], "invalid_request");
    assert_eq!(elsewhere.value["details"]["pointer"], "/after_economy_id");
    for (limit, wait_seconds, pointer) in [
        (0, 0, "/limit"),
        (101, 0, "/limit"),
        (1, 31, "/wait_seconds"),
    ] {
        let refused = follow(&server, &live, &start, limit, wait_seconds, None).await;
        assert_eq!(refused.value["error"], "invalid_request");
        assert_eq!(refused.value["details"]["pointer"], pointer);
    }

    // A ref this store does not hold says how to make one.
    let unknown = RefId::from_digest(&blake3::hash(b"never made"));
    let missing = read_ref(&server, &unknown).await;
    assert_eq!(missing.value["error"], "not_found");
    let message = missing.value["message"].as_str().unwrap();
    assert!(message.contains("axionomy_ref_create"), "{message}");
    for refused in [
        advance(&server, &unknown, &start, &mine).await,
        follow(&server, &unknown, &start, 10, 0, None).await,
    ] {
        assert_eq!(refused.value["error"], "not_found");
    }
}

#[tokio::test]
async fn a_ref_is_deleted_from_where_its_deleter_expects_it() {
    let server = server();
    let start = id(&create(&server, fixture()).await);
    let live = refer(&server, &start).await;
    let mine = id(&apply(&server, &start, transfer(1)).await);
    assert_eq!(id(&advance(&server, &live, &start, &mine).await), mine);

    // A deleter who still saw the start is told where the ref went.
    let refused = delete(&server, &live, &start).await;
    assert_eq!(refused.value["error"], "moved");
    assert_eq!(
        refused.value["details"],
        json!({ "current_economy_id": mine })
    );
    assert_eq!(id(&read_ref(&server, &live).await), mine);

    // From where it is, it goes, and says where it was; the economy stays.
    let deleted = delete(&server, &live, &mine).await;
    assert!(!deleted.is_error, "{}", deleted.value);
    assert_eq!(
        deleted.value,
        json!({ "ref_id": live.as_str(), "economy_id": mine })
    );
    for gone in [
        read_ref(&server, &live).await,
        delete(&server, &live, &mine).await,
        follow(&server, &live, &mine, 10, 0, None).await,
    ] {
        assert_eq!(gone.value["error"], "not_found");
    }
    assert!(!apply(&server, &mine, transfer(1)).await.is_error);
}

#[tokio::test]
async fn only_the_steps_a_ref_moves_over_are_delivered() {
    let server = server();
    let start = id(&create(&server, fixture()).await);
    let live = refer(&server, &start).await;
    let first = id(&apply(&server, &start, transfer(1)).await);
    // Tried out, never moved over: never delivered.
    let tried = id(&apply(&server, &first, transfer(4)).await);
    assert!(!advance(&server, &live, &start, &first).await.is_error);
    let replayed = id(&replay(&server, &first, vec![transfer(1), transfer(2)]).await);
    assert!(!advance(&server, &live, &first, &replayed).await.is_error);

    let goal = json!({ "sink": { "token": "4" } });
    let followed = follow(&server, &live, &start, 10, 0, Some(goal.clone())).await;
    let economies = delivered(&followed);
    assert_eq!(id(&followed), replayed);
    assert_eq!(economies.len(), 3);
    assert!(!economies.contains(&tried));
    // A step the ref moved over to an economy a call made is named by that
    // economy's handle; one inside a replay gets a handle of its own.
    assert_eq!((&economies[0], &economies[2]), (&first, &replayed));
    let frames = followed.value["frames"].as_array().unwrap();
    assert_eq!(
        frames
            .iter()
            .map(|frame| (frame["exchange"]["units"].clone(), frame["holds"].clone()))
            .collect::<Vec<_>>(),
        [
            (json!("1"), json!(false)),
            (json!("1"), json!(false)),
            (json!("2"), json!(true)),
        ]
    );
    assert_eq!(
        frames[2]["receipt"],
        json!({ "changes": [
            { "account": "sink", "asset": "token", "kind": "produced", "quantity": "2" },
            { "account": "source", "asset": "token", "kind": "consumed", "quantity": "2" },
        ] })
    );
    // Without a goal, no frame says whether one holds.
    let plain = follow(&server, &live, &start, 10, 0, None).await;
    assert!(plain.value["frames"][0].get("holds").is_none());
    assert_eq!(delivered(&plain), economies);

    // The economy a step made inside the replay is an economy like any other:
    // made by the frame's exchange from the frame before it.
    let middle = &economies[1];
    let economy = read(
        &server,
        &uri(Target::Economy {
            economy_id: middle.clone(),
        }),
    )
    .await;
    assert_eq!(economy["parent_id"], first.as_str());
    assert_eq!(
        economy["origin"],
        json!({ "kind": "exchanged", "exchanges": 1 })
    );
    let exchanges = read(
        &server,
        &uri(Target::Exchanges {
            economy_id: middle.clone(),
        }),
    )
    .await;
    assert_eq!(exchanges["items"].as_array().unwrap().len(), 1);
    assert_eq!(exchanges["items"][0]["units"], "1");
    let source = step(&server, middle, 0).await;
    assert_eq!(source.value["source_economy_id"], first.as_str());
    let first_root = read(
        &server,
        &uri(Target::Economy {
            economy_id: first.clone(),
        }),
    )
    .await["root"]
        .clone();
    assert_eq!(source.value["root"], first_root);
    let onward = apply(&server, middle, transfer(2)).await;
    let replayed_root = read(
        &server,
        &uri(Target::Economy {
            economy_id: replayed.clone(),
        }),
    )
    .await["root"]
        .clone();
    assert_eq!(onward.value["root"], replayed_root);
    let diff = read(
        &server,
        &uri(Target::Diff {
            economy_id: middle.clone(),
            other_id: replayed.clone(),
        }),
    )
    .await;
    assert_eq!(diff["items"].as_array().unwrap().len(), 2);

    // A follower pulls from the last economy it handled, a page at a time,
    // and every call names the same steps the same way.
    let page = delivered(&follow(&server, &live, &start, 2, 0, None).await);
    assert_eq!(page, economies[..2]);
    let rest = delivered(&follow(&server, &live, &page[1], 2, 0, None).await);
    assert_eq!(rest, economies[2..]);
    let caught_up = follow(&server, &live, &replayed, 2, 0, None).await;
    assert!(delivered(&caught_up).is_empty());
    assert_eq!(id(&caught_up), replayed);
}

#[tokio::test]
async fn a_follower_waits_for_the_ref_to_move() {
    let server = server();
    let start = id(&create(&server, fixture()).await);
    let live = refer(&server, &start).await;
    // Nothing moves: the wait ends with the time, where the ref is.
    let waited = std::time::Instant::now();
    let idle = follow(&server, &live, &start, 10, 1, None).await;
    assert!(waited.elapsed() >= std::time::Duration::from_secs(1));
    assert!(delivered(&idle).is_empty());
    assert_eq!(id(&idle), start);

    let mover = {
        let (server, live, start) = (server.clone(), live.clone(), start.clone());
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            let next = id(&apply(&server, &start, transfer(1)).await);
            assert!(!advance(&server, &live, &start, &next).await.is_error);
            next
        })
    };
    let waited = std::time::Instant::now();
    let moved = follow(&server, &live, &start, 10, 30, None).await;
    assert!(waited.elapsed() < std::time::Duration::from_secs(30));
    let next = mover.await.unwrap();
    assert_eq!(id(&moved), next);
    assert_eq!(delivered(&moved), [next]);
}

/// The maze's package, exported from the store it was written in.
fn maze_package() -> Store {
    let source = Engine::new(Store::memory().unwrap()).unwrap();
    axionomy_problems::maze::package::write(&source).unwrap();
    let name = "maze".parse().unwrap();
    let package = Store::memory().unwrap();
    source
        .export(
            &name,
            "0.1.0".parse().unwrap(),
            source.named_under(&name).unwrap(),
            &package,
        )
        .unwrap();
    package
}

/// A server whose store the maze's package was loaded into, as
/// `axionomy mcp --package maze.zip` loads it.
fn maze_server() -> AxionomyMcp {
    let engine = Engine::new(Store::memory().unwrap()).unwrap();
    engine.load(&maze_package()).unwrap();
    AxionomyMcp::new(engine).unwrap()
}

async fn packages(server: &AxionomyMcp) -> Outcome {
    outcome(
        server
            .package_list(Parameters(PackageListRequest::default()))
            .await,
    )
}

fn experiment(path: &str) -> Parameters<ExperimentRequest> {
    Parameters(ExperimentRequest {
        experiment: path.parse().unwrap(),
    })
}

async fn run_experiment(server: &AxionomyMcp, path: &str) -> Outcome {
    outcome(server.experiment_run(experiment(path)).await)
}

async fn experiment_results(server: &AxionomyMcp, path: &str) -> Outcome {
    outcome(server.experiment_results(experiment(path)).await)
}

async fn root_of(server: &AxionomyMcp, economy_id: &Value) -> Value {
    let economy_id = handle(economy_id);
    read(server, &uri(Target::Economy { economy_id })).await["root"].clone()
}

#[tokio::test]
async fn a_loaded_package_lists_its_refs_experiments_and_strategies() {
    let server = maze_server();
    let listed = packages(&server).await;
    assert!(!listed.is_error, "{}", listed.value);
    let packages_listed = listed.value["packages"].as_array().unwrap();
    assert_eq!(packages_listed.len(), 1);
    let maze = &packages_listed[0];
    assert_eq!(maze["name"], "maze");
    assert_eq!(maze["version"], "0.1.0");
    assert_eq!(maze["format"], axionomy_store::FORMAT_VERSION);
    assert_eq!(maze["host_api"], axionomy_engine::HOST_API);
    let instances = ["maze/micro", "maze/showcase", "maze/stress"];
    assert_eq!(maze["experiments"], json!(instances));
    assert_eq!(
        maze["strategies"],
        json!([
            "maze/a-star",
            "maze/breadth-first",
            "maze/dijkstra",
            "maze/pareto-energy",
            "maze/pareto-time"
        ])
    );
    let refs = maze["refs"].as_array().unwrap();
    assert_eq!(
        refs.iter().map(|named| &named["ref"]).collect::<Vec<_>>(),
        instances
    );

    // Each ref names an authored economy: the instance's start, the state
    // at the point the package's ref names.
    let engine = server.archive.engine();
    for named in refs {
        let economy_id: EconomyId = handle(&named["economy_id"]);
        let economy = read(
            &server,
            &uri(Target::Economy {
                economy_id: economy_id.clone(),
            }),
        )
        .await;
        assert!(listed.links.contains(&uri(Target::Economy { economy_id })));
        assert!(economy.get("parent_id").is_none());
        assert_eq!(economy["origin"]["kind"], "authored");
        let point = engine
            .referred(&named["ref"].as_str().unwrap().parse().unwrap())
            .unwrap();
        let root = engine.reach(point).unwrap().state.root();
        assert_eq!(economy["root"], root.to_hex().as_str());
    }
    // A point keeps the handle that first named it.
    assert_eq!(packages(&server).await.value, listed.value);

    // A server that loaded nothing lists nothing.
    assert_eq!(
        packages(&self::server()).await.value,
        json!({ "packages": [] })
    );
}

#[tokio::test]
async fn an_experiment_runs_as_the_engine_runs_it_and_names_every_trial_end() {
    let server = maze_server();
    let ran = run_experiment(&server, "maze/micro").await;
    assert!(!ran.is_error, "{}", ran.value);
    assert_eq!(ran.value["experiment"], "maze/micro");
    assert_eq!(ran.value["goal"], "maze/solved");
    assert_eq!(
        ran.value["objectives"],
        json!([
            { "objective": "maze/energy", "direction": "minimize" },
            { "objective": "maze/time", "direction": "minimize" },
        ])
    );

    // The engine, running the package's experiment alone, gives the same
    // arms the same trials: seeds, outcomes, objective values and traces.
    let engine = Engine::new(Store::memory().unwrap()).unwrap();
    engine.load(&maze_package()).unwrap();
    let expected = engine
        .run_experiment(&"maze/micro".parse().unwrap(), &mut |_, _| {
            ControlFlow::Continue(())
        })
        .unwrap();
    let arms = ran.value["arms"].as_array().unwrap();
    assert_eq!(arms.len(), expected.arms.len());
    let listed = packages(&server).await.value;
    let start = &listed["packages"][0]["refs"][0];
    assert_eq!(start["ref"], "maze/micro");
    for (arm, expected) in arms.iter().zip(&expected.arms) {
        assert_eq!(arm["strategy"], expected.arm.strategy.as_str());
        assert_eq!(arm["start"], "maze/micro");
        // Every arm starts from the economy the package's ref names.
        assert_eq!(arm["start_economy_id"], start["economy_id"]);
        let trials = arm["trials"].as_array().unwrap();
        assert_eq!(trials.len(), expected.trials.len());
        for (trial, expected) in trials.iter().zip(&expected.trials) {
            assert_eq!(trial["seed"], expected.seed);
            assert_eq!(trial["ended"], json!({ "kind": "reached" }));
            assert_eq!(
                trial["values"],
                json!({
                    "maze/energy": expected.values[0].to_string(),
                    "maze/time": expected.values[1].to_string(),
                })
            );
            let (_, trace) = engine.history(expected.end).unwrap();
            assert_eq!(trial["exchanges"], trace.len());

            // The trial's end is an economy like any other: its exchanges
            // are the trace, its step zero the arm's start, and each step's
            // exchange takes the step before it to it.
            let end: EconomyId = handle(&trial["economy_id"]);
            let exchanges = read(
                &server,
                &uri(Target::Exchanges {
                    economy_id: end.clone(),
                }),
            )
            .await;
            assert_eq!(exchanges["items"], serde_json::to_value(&trace).unwrap());
            let root = engine.reach(expected.end).unwrap().state.root();
            assert_eq!(
                root_of(&server, &trial["economy_id"]).await,
                root.to_hex().as_str()
            );
            let source = step(&server, &end, 0).await;
            assert_eq!(source.value["source_economy_id"], arm["start_economy_id"]);
            assert_eq!(
                source.value["root"],
                root_of(&server, &arm["start_economy_id"]).await
            );
            for (index, exchange) in trace.into_iter().enumerate() {
                let before = id(&step(&server, &end, index).await);
                let tried = apply(&server, &before, exchange).await;
                let after = step(&server, &end, index + 1).await;
                assert_eq!(tried.value["root"], after.value["root"]);
            }
        }
    }

    // The recorded results read back as they ran, under the same handles.
    let recorded = experiment_results(&server, "maze/micro").await;
    assert_eq!(recorded.value, ran.value);

    // Running again records runs of its own: the same trials and values,
    // reaching equal economies under handles of their own.
    let again = run_experiment(&server, "maze/micro").await;
    for (arm, earlier) in again.value["arms"].as_array().unwrap().iter().zip(arms) {
        assert_eq!(arm["start_economy_id"], earlier["start_economy_id"]);
        let (trial, earlier) = (&arm["trials"][0], &earlier["trials"][0]);
        assert_eq!(trial["values"], earlier["values"]);
        assert_ne!(trial["economy_id"], earlier["economy_id"]);
        assert_eq!(
            root_of(&server, &trial["economy_id"]).await,
            root_of(&server, &earlier["economy_id"]).await
        );
    }
    assert_eq!(
        experiment_results(&server, "maze/micro").await.value,
        again.value
    );
}

#[tokio::test]
async fn an_experiment_is_one_a_loaded_package_carries_within_the_trial_limit() {
    let server = maze_server();
    for path in ["maze/huge", "sokoban/micro"] {
        for refused in [
            run_experiment(&server, path).await,
            experiment_results(&server, path).await,
        ] {
            assert!(refused.is_error);
            assert_eq!(refused.value["error"], "not_found");
            assert_eq!(refused.value["details"]["pointer"], "/experiment");
            let message = refused.value["message"].as_str().unwrap();
            assert!(message.contains("axionomy_package_list"), "{message}");
        }
    }

    // An experiment that has not run has no results to give.
    let unrun = experiment_results(&server, "maze/showcase").await;
    assert_eq!(unrun.value["error"], "not_found");
    let message = unrun.value["message"].as_str().unwrap();
    assert!(message.contains("axionomy_experiment_run"), "{message}");

    // An experiment of more trials than one call runs is refused before
    // anything runs.
    let engine = server.archive.engine();
    let mut huge = engine.experiment(&"maze/micro".parse().unwrap()).unwrap();
    huge.trials = MAX_TRIALS / huge.arms.len() as u64 + 1;
    engine
        .keep_experiment(&"maze/huge".parse().unwrap(), &huge)
        .unwrap();
    let refused = run_experiment(&server, "maze/huge").await;
    assert_eq!(refused.value["error"], "too_large", "{}", refused.value);
    assert_eq!(refused.value["details"]["pointer"], "/experiment");
    assert!(engine.results(&"maze/huge".parse().unwrap()).is_err());
}
