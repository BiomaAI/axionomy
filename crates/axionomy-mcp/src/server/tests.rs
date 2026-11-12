use super::*;
use crate::{handles::StudyId, resources::Address};

use axionomy_view::DocumentPath;
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
    let tools = AxionomyMcp::default().tool_router.list_all();
    assert_eq!(tools.len(), 10);
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
    let server = AxionomyMcp::default();
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

fn maze_study() -> StudyRequest {
    StudyRequest::new("maze".parse().unwrap())
        .with_instance("micro".parse().unwrap())
        .with_strategy("a-star".parse().unwrap())
}

/// Studies a problem as `axionomy_problem_study` does, and gives its summary.
async fn study(server: &AxionomyMcp, request: StudyRequest) -> Value {
    let tasks = TaskManager::new();
    let archive = server.archive.clone();
    let registry = Arc::clone(&server.registry);
    let task = tasks.spawn(TaskOptions::new(), move |context| {
        Box::pin(study_problem(context, archive, registry, request))
    });
    completed(terminal_task(&tasks, &task.task_id).await)
}

fn handle<T: std::str::FromStr<Err: std::fmt::Debug>>(value: &Value) -> T {
    value.as_str().unwrap().parse().unwrap()
}

/// What the service reports of each document's trajectory, by document.
#[derive(Default)]
struct Trajectories(std::collections::BTreeMap<DocumentPath, (paths::Economy, paths::Trace)>);

impl axionomy_study::StudyObserver for Trajectories {
    fn progress(&mut self, _: Progress) {}

    fn trajectory(
        &mut self,
        document: &DocumentPath,
        initial: &paths::Economy,
        trace: &paths::Trace,
    ) {
        self.0
            .insert(document.clone(), (initial.clone(), trace.clone()));
    }
}

#[tokio::test]
async fn a_problem_study_returns_a_summary_and_pages_its_frames() {
    let server = AxionomyMcp::default();
    let request = maze_study();
    let summary = study(&server, request.clone()).await;
    let study_id: StudyId = summary["study_id"].as_str().unwrap().parse().unwrap();
    let served = server.study(&study_id).await.unwrap().unwrap();
    assert_eq!(
        *served,
        axionomy_problems::registry().study(request).unwrap()
    );
    assert_eq!(summary["key"]["strategy"], "a-star");
    assert_eq!(summary["selected_document"], "a-star");
    let document: DocumentPath = summary["selected_document"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    let frames = summary["documents"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["id"] == document.as_str())
        .unwrap()["frames"]
        .as_u64()
        .unwrap();

    let first = read(
        &server,
        &uri(Target::Frames {
            study_id: study_id.clone(),
            document: document.clone(),
        }),
    )
    .await;
    assert_eq!(
        first["items"].as_array().unwrap().len() as u64,
        frames.min(resources::PAGE_FRAMES as u64)
    );
    let entry = summary["documents"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["id"] == document.as_str())
        .unwrap();
    let view = read(&server, &uri(Target::Document { study_id, document })).await;
    assert_eq!(view["frames"], frames);
    assert_eq!(view["document"]["frames"], json!([]));
    assert_eq!(view["initial_economy_id"], entry["initial_economy_id"]);
    assert_eq!(view["final_economy_id"], entry["final_economy_id"]);
}

#[tokio::test]
async fn a_frames_exchange_applies_to_the_economy_of_the_step_before_it() {
    let server = AxionomyMcp::default();
    let summary = study(&server, maze_study()).await;
    let study_id: StudyId = handle(&summary["study_id"]);
    for entry in summary["documents"].as_array().unwrap() {
        let final_id: EconomyId = handle(&entry["final_economy_id"]);
        let trace = server.economy(&final_id).await.unwrap();
        let frames = Target::Frames {
            study_id: study_id.clone(),
            document: handle(&entry["id"]),
        };
        let mut page_uri = frames.uri();
        let mut index = 0;
        loop {
            let page = read(&server, &page_uri).await;
            for frame in page["items"].as_array().unwrap() {
                assert_eq!(frame["step"], index + 1);
                assert_eq!(frame["view"]["index"], index);
                let exchange: paths::Exchange =
                    serde_json::from_value(frame["exchange"].clone()).unwrap();
                assert_eq!(&exchange, trace.receipts[index].exchange());

                // Frame i's exchange takes step i of the trace to step i + 1.
                let before = id(&step(&server, &final_id, index).await);
                let tried = apply(&server, &before, exchange).await;
                let after = step(&server, &final_id, index + 1).await;
                assert_eq!(tried.value["root"], after.value["root"]);
                index += 1;
            }
            let Some(cursor) = page["nextCursor"].as_str() else {
                break;
            };
            page_uri = frames.page_uri(cursor);
        }
        assert_eq!(entry["frames"], index);
    }
}

#[tokio::test]
async fn a_studys_documents_are_economies_that_replay_its_trajectories() {
    let server = AxionomyMcp::default();
    let summary = study(&server, maze_study()).await;
    let mut reported = Trajectories::default();
    axionomy_problems::registry()
        .study_with(&maze_study(), &StudyControl::default(), &mut reported)
        .unwrap();
    let documents = summary["documents"].as_array().unwrap();
    assert_eq!(documents.len(), reported.0.len());
    for document in documents {
        let (initial, trace) = &reported.0[&handle::<DocumentPath>(&document["id"])];
        let initial_id: EconomyId = handle(&document["initial_economy_id"]);
        let final_id: EconomyId = handle(&document["final_economy_id"]);
        assert_eq!(document["frames"], trace.exchanges().len());

        let start = server.economy(&initial_id).await.unwrap();
        assert_eq!(start.record.parent, None);
        assert_eq!(start.state.root(), State::new(initial.clone()).root());

        // The final economy is the trace applied to the initial one: its
        // parent, its exchanges, and the state replaying them reaches.
        let end = server.economy(&final_id).await.unwrap();
        assert_eq!(end.record.parent.as_ref(), Some(&initial_id));
        assert!(
            end.receipts
                .iter()
                .map(paths::Receipt::exchange)
                .eq(trace.exchanges())
        );
        let replayed = initial.replayed(trace).unwrap();
        assert_eq!(end.state.root(), State::new(replayed).root());

        // It is an ordinary economy: replaying the trace on the initial
        // economy is the operation that made it, and every step is a state.
        let again = replay(&server, &initial_id, trace.exchanges().to_vec()).await;
        assert_eq!(id(&again), final_id);
        assert_eq!(again.value["deduplicated"], true);
        let halfway = trace.exchanges().len() / 2;
        let stepped = step(&server, &final_id, halfway).await;
        let prefix: paths::Trace = trace.exchanges()[..halfway].iter().cloned().collect();
        let root = State::new(initial.replayed(&prefix).unwrap()).root();
        assert_eq!(stepped.value["root"], root.to_hex().as_str());
    }
}

#[tokio::test]
async fn a_study_keeps_its_key_and_handles_and_computes_its_views_again() {
    let server = AxionomyMcp::default();
    let summary = study(&server, maze_study()).await;
    let study_id: StudyId = handle(&summary["study_id"]);

    // What is kept is the summary without the study's handle: no views.
    let bytes = server.archive.study_bytes(&study_id).unwrap();
    assert!(serde_json::from_slice::<axionomy_study::Study>(&bytes).is_err());
    let mut kept: Value = serde_json::from_slice(&bytes).unwrap();
    let mut expected = summary.clone();
    expected.as_object_mut().unwrap().remove("study_id");
    assert_eq!(kept, expected);

    // A server with nothing in memory computes the study again from its key
    // and serves the same documents and frames.
    let document: DocumentPath = handle(&summary["selected_document"]);
    let targets = [
        Target::Study {
            study_id: study_id.clone(),
        },
        Target::Document {
            study_id: study_id.clone(),
            document: document.clone(),
        },
        Target::Frames {
            study_id: study_id.clone(),
            document: document.clone(),
        },
    ];
    let restarted =
        AxionomyMcp::new(axionomy_problems::registry(), server.archive.store()).unwrap();
    assert!(restarted.archive.cached_study(&study_id).is_none());
    for target in &targets {
        assert_eq!(
            read(&restarted, &uri(target.clone())).await,
            read(&server, &uri(target.clone())).await
        );
    }

    // Trajectories that differ from those recorded are refused: the study
    // was recorded by another version.
    let stepped = kept["documents"][0]["final_economy_id"].clone();
    kept["documents"][0]["final_economy_id"] = kept["documents"][1]["final_economy_id"].clone();
    kept["documents"][1]["final_economy_id"] = stepped;
    server
        .archive
        .replace_study_bytes(&study_id, serde_json::to_vec(&kept).unwrap())
        .await;
    let stale = AxionomyMcp::new(axionomy_problems::registry(), server.archive.store()).unwrap();
    let error = stale
        .read(&Address::parse(&uri(targets[1].clone())).unwrap())
        .await
        .unwrap_err();
    assert!(
        error.message.contains("another version"),
        "{}",
        error.message
    );
    // Its key and handles are still read as they were kept.
    assert_eq!(
        read(&stale, &uri(targets[0].clone())).await["documents"],
        kept["documents"]
    );
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

    let server = AxionomyMcp::default();
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
    let server = AxionomyMcp::default();
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
    for tool in AxionomyMcp::default().tool_router.list_all() {
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
    let server = AxionomyMcp::default();
    let missing = extend(&server, &missing(), json!({})).await;
    assert_eq!(missing.value["error"], "not_found");
    let message = missing.value["message"].as_str().unwrap();
    assert!(message.contains("in memory"), "{message}");
    assert!(message.contains("axionomy_problem_study"), "{message}");

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
    let (parent_id, applied) = {
        let server = AxionomyMcp::new(
            axionomy_problems::registry(),
            axionomy_store::Store::open(&file).unwrap(),
        )
        .unwrap();
        let parent_id = id(&create(&server, fixture()).await);
        let applied = apply(&server, &parent_id, transfer(2)).await;
        (parent_id, applied)
    };
    let server = AxionomyMcp::new(
        axionomy_problems::registry(),
        axionomy_store::Store::open(&file).unwrap(),
    )
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
    drop(server);
    std::fs::remove_dir_all(&directory).unwrap();
}
