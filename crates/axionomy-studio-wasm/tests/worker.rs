//! The Worker's engine, natively: a package loads from its bytes, an
//! experiment runs, and a store kept on a backend that outlives it finds
//! both again, with the URL the package came from and what it said.

use axionomy_engine::{Engine, EngineError, Version, zipped};
use axionomy_store::Store;
use axionomy_studio_api as api;
use axionomy_studio_wasm::{Source, Validators, WorkerEngine};
use redb::{StorageBackend, backends::InMemoryBackend};
use serde_json::{Value, json};
use std::{io, ops::ControlFlow, sync::Arc};

fn path<K>(text: &str) -> axionomy::path::Path<K> {
    text.parse().unwrap()
}

/// Bytes that outlive the stores opened on them, as the origin private file
/// system's do.
#[derive(Debug, Clone, Default)]
struct Kept(Arc<InMemoryBackend>);

impl Kept {
    fn bytes(&self) -> Vec<u8> {
        let mut bytes = vec![0; usize::try_from(self.0.len().unwrap()).unwrap()];
        self.0.read(0, &mut bytes).unwrap();
        bytes
    }
}

impl StorageBackend for Kept {
    fn len(&self) -> Result<u64, io::Error> {
        self.0.len()
    }

    fn read(&self, offset: u64, out: &mut [u8]) -> Result<(), io::Error> {
        self.0.read(offset, out)
    }

    fn set_len(&self, len: u64) -> Result<(), io::Error> {
        self.0.set_len(len)
    }

    fn sync_data(&self) -> Result<(), io::Error> {
        self.0.sync_data()
    }

    fn write(&self, offset: u64, data: &[u8]) -> Result<(), io::Error> {
        self.0.write(offset, data)
    }
}

/// The maze sample's package file, zipped, as Studio serves it.
fn maze() -> Vec<u8> {
    let name = path("maze");
    let (_, write) = axionomy_problems::packages()
        .into_iter()
        .find(|(sample, _)| *sample == name)
        .unwrap();
    let engine = Engine::new(Store::memory().unwrap()).unwrap();
    write(&engine).unwrap();
    let file = Kept::default();
    {
        let package = Store::on(file.clone()).unwrap();
        let contents = engine.named_under(&name).unwrap();
        engine
            .export(
                &name,
                "0.1.0".parse::<Version>().unwrap(),
                contents,
                &package,
            )
            .unwrap();
    }
    zipped(&file.bytes()).unwrap()
}

const URL: &str = "https://studio.example/packages/maze.zip";

/// What the store remembers of loading the maze from `URL`.
fn source(validators: &Validators) -> Source {
    Source {
        validators: validators.clone(),
        packages: vec![path("maze")],
    }
}

#[test]
fn a_package_and_its_experiment_are_found_again_with_where_it_came_from() {
    let bytes = maze();
    let validators = Validators {
        etag: Some("\"maze-1\"".to_owned()),
        last_modified: None,
    };
    let kept = Kept::default();
    let ran = {
        let worker = WorkerEngine::over(Store::on(kept.clone()).unwrap()).unwrap();
        assert_eq!(worker.source_of(URL).unwrap(), None);
        let loaded = worker
            .load_from(&bytes, Some((URL, validators.clone())))
            .unwrap();
        assert_eq!(loaded.manifests[0].name, path("maze"));
        assert!(loaded.kept.refs.is_empty() && loaded.kept.definitions.is_empty());
        assert_eq!(worker.source_of(URL).unwrap(), Some(source(&validators)));
        assert_eq!(
            worker
                .source_of("https://elsewhere.example/maze.zip")
                .unwrap(),
            None
        );
        let experiments = worker.engine().experiments().unwrap();
        assert!(experiments.contains(&path("maze/micro")), "{experiments:?}");
        worker
            .engine()
            .run_experiment(&path("maze/micro"), &mut |_, _| ControlFlow::Continue(()))
            .unwrap()
    };

    // Opened again, as a reloaded Worker opens it.
    let worker = WorkerEngine::over(Store::on(kept).unwrap()).unwrap();
    let manifests = worker.engine().manifests().unwrap();
    assert_eq!(manifests.len(), 1);
    assert_eq!(manifests[0].name, path("maze"));
    assert_eq!(worker.engine().results(&path("maze/micro")).unwrap(), ran);
    assert_eq!(worker.source_of(URL).unwrap(), Some(source(&validators)));
    let end = ran.arms[0].trials[0].end;
    assert!(!worker.engine().draw(end).unwrap().frames.is_empty());
}

#[test]
fn a_package_without_a_url_loads_and_names_no_source() {
    let worker = WorkerEngine::over(Store::memory().unwrap()).unwrap();
    let loaded = worker.load_from(&maze(), None).unwrap();
    assert_eq!(loaded.manifests[0].name, path("maze"));
    assert_eq!(worker.source_of(URL).unwrap(), None);
}

fn parsed(json: &str) -> Value {
    serde_json::from_str(json).unwrap()
}

/// A request as the Worker reads its JSON.
fn from<T: serde::de::DeserializeOwned>(json: &str) -> T {
    serde_json::from_str(json).unwrap()
}

fn apply_request(candidate: &Value) -> String {
    json!({ "exchanges": [candidate["exchange"]] }).to_string()
}

#[test]
fn the_worker_offers_the_operations_the_server_offers() {
    let worker = WorkerEngine::over(Store::memory().unwrap()).unwrap();
    let loaded = parsed(&worker.load(&maze(), None, None, None).unwrap());
    assert_eq!(
        loaded,
        json!({ "packages": ["maze"], "kept": { "refs": [], "definitions": [] } })
    );
    // The same functions the server calls, so the same JSON.
    let listed = parsed(&worker.packages().unwrap());
    assert_eq!(
        listed,
        serde_json::to_value(axionomy_studio_api::packages(worker.engine()).unwrap()).unwrap()
    );
    let micro = &listed["packages"][0]["refs"][0];
    assert_eq!(micro["name"], "maze/micro");
    // The package ships it, so it is fixed.
    assert_eq!(micro["shipped_by"], "maze");
    let start = micro["point"].as_str().unwrap().to_owned();
    let held = parsed(&worker.balances(&start).unwrap());
    assert_eq!(
        held,
        serde_json::to_value(
            axionomy_studio_api::balances(worker.engine(), start.parse().unwrap()).unwrap()
        )
        .unwrap()
    );
    assert!(!held["accounts"].as_array().unwrap().is_empty());
    // A player names a ref of their own where it starts, and moves that.
    let named = json!({ "name": "maze/play", "point": start }).to_string();
    assert_eq!(
        parsed(&worker.create_ref(&named).unwrap()),
        json!({ "name": "maze/play", "point": start })
    );

    // The moves at the start, assessed; one applied, and the player's ref
    // moved over it.
    let offered = parsed(
        &worker
            .candidates(&start, r#"{"set":"maze/moves"}"#)
            .unwrap(),
    );
    let candidate = offered["candidates"]
        .as_array()
        .unwrap()
        .iter()
        .find(|candidate| candidate["proposal"]["assessment"]["status"] == "applicable")
        .unwrap()
        .clone();
    let assess = json!({ "exchange": candidate["exchange"] }).to_string();
    let assessed = parsed(&worker.assess(&start, &assess).unwrap());
    assert_eq!(assessed["proposal"]["assessment"]["status"], "applicable");
    let applied = parsed(&worker.apply(&start, &apply_request(&candidate)).unwrap());
    let moved = applied["point"].as_str().unwrap().to_owned();
    let advance = json!({ "expected": start, "to": moved }).to_string();
    // The package's ref refuses to move, with the status the server
    // answers with: what the Worker throws.
    let fixed = api::advance_ref(worker.engine(), path("maze/micro"), from(&advance)).unwrap_err();
    assert_eq!(fixed.status(), 403);
    assert!(
        fixed.message().contains("`maze/micro`") && fixed.message().contains("package `maze`"),
        "{}",
        fixed.message()
    );
    let advanced = parsed(&worker.advance_ref("maze/play", &advance).unwrap());
    assert_eq!(advanced, json!({ "name": "maze/play", "point": moved }));
    // Asking again for the move made is told it was made.
    let again = parsed(&worker.advance_ref("maze/play", &advance).unwrap());
    assert_eq!(again, advanced);
    assert_eq!(
        parsed(&worker.read_ref("maze/play").unwrap())["point"],
        json!(moved)
    );
    assert_eq!(
        parsed(&worker.read_ref("maze/micro").unwrap()),
        json!({ "name": "maze/micro", "point": start, "shipped_by": "maze" })
    );
    let query = json!({ "after": start, "objectives": "maze/energy" }).to_string();
    let followed = parsed(&worker.frames("maze/play", &query).unwrap());
    assert_eq!(followed["frames"].as_array().unwrap().len(), 1);
    assert_eq!(followed["frames"][0]["point"], json!(moved));
    let drawn = parsed(&worker.draw(&moved).unwrap());
    assert_eq!(drawn["frames"].as_array().unwrap().len(), 1);

    // Anywhere else, applying branches: the ref stays.
    let earlier = parsed(&worker.ancestor(&moved, r#"{"step":0}"#).unwrap());
    let earlier = earlier["point"].as_str().unwrap().to_owned();
    assert_eq!(earlier, start);
    let branched = parsed(&worker.apply(&earlier, &apply_request(&candidate)).unwrap());
    assert_ne!(branched["point"], json!(moved));
    assert_eq!(
        parsed(&worker.read_ref("maze/play").unwrap())["point"],
        json!(moved)
    );
    let named = json!({ "name": "maze/branch/one", "point": branched["point"] }).to_string();
    worker.create_ref(&named).unwrap();
    let refs = parsed(&worker.packages().unwrap())["packages"][0]["refs"].clone();
    assert!(
        refs.as_array()
            .unwrap()
            .iter()
            .any(|entry| entry["name"] == "maze/branch/one"),
        "{refs}"
    );
    // The branch and the move compared: they part at the start, and the
    // same move made on both reaches the same state.
    let query = json!({ "left": moved, "right": branched["point"], "objectives": "maze/energy" })
        .to_string();
    let compared = parsed(&worker.divergence(&query).unwrap());
    assert_eq!(
        compared,
        serde_json::to_value(api::divergence(worker.engine(), from(&query)).unwrap()).unwrap()
    );
    assert_eq!(compared["shared"], json!(start));
    assert_eq!(compared["converged"], 1);
    assert_eq!(compared["steps"][0]["left"], compared["steps"][0]["right"]);
    assert_eq!(compared["left"]["values"], compared["right"]["values"]);

    // A ref goes from where its deleter saw it; the package's never does.
    let at_start = json!({ "expected": start }).to_string();
    let fixed = api::delete_ref(worker.engine(), path("maze/micro"), from(&at_start)).unwrap_err();
    assert_eq!(fixed.status(), 403);
    let elsewhere =
        api::delete_ref(worker.engine(), path("maze/play"), from(&at_start)).unwrap_err();
    assert_eq!(elsewhere.status(), 409);
    let at_moved = json!({ "expected": moved }).to_string();
    assert_eq!(
        parsed(&worker.delete_ref("maze/play", &at_moved).unwrap()),
        json!({ "name": "maze/play", "point": moved })
    );
    assert!(matches!(
        worker.engine().referred(&path("maze/play")),
        Err(EngineError::UnknownRef(_))
    ));
    assert!(
        !parsed(&worker.draw(&moved).unwrap())["frames"]
            .as_array()
            .unwrap()
            .is_empty()
    );

    // A strategy continued from the start, as the Worker runs it.
    let from: axionomy_studio_api::PointId = start.parse().unwrap();
    let request = serde_json::from_value(
        json!({ "strategy": "maze/a-star", "goal": "maze/solved", "seed": "17", "budget": "100000", "turns": "0" }),
    )
    .unwrap();
    let ran = axionomy_studio_api::run_strategy(worker.engine(), from, request, &mut |_| {
        ControlFlow::Continue(())
    })
    .unwrap();
    assert_eq!(
        serde_json::to_value(ran).unwrap()["outcome"]["kind"],
        "reached"
    );

    // An experiment's ends compared.
    let results =
        axionomy_studio_api::run_experiment(worker.engine(), path("maze/micro"), &mut |_| {
            ControlFlow::Continue(())
        })
        .unwrap();
    assert_eq!(
        parsed(&worker.results("maze/micro").unwrap()),
        serde_json::to_value(&results).unwrap()
    );
    let ends: Vec<String> = results
        .arms
        .iter()
        .map(|arm| arm.trials[0].end.to_string())
        .collect();
    let compare = json!({ "points": ends, "objectives": ["maze/energy"] }).to_string();
    let compared = parsed(&worker.compare(&compare).unwrap());
    assert!(!compared["front"].as_array().unwrap().is_empty());
}

/// The point the first maze move that applies at `at` reaches.
fn moved_on(engine: &Engine, at: axionomy_store::Point) -> axionomy_store::Point {
    let offered = engine.candidates(at, &path("maze/moves")).unwrap();
    let applicable = engine.reach(at).unwrap().state.economy.applicable(offered);
    engine.apply(at, &applicable[..1]).unwrap()
}

/// A Worker's store opened on `bytes`, as the next Worker opens the file a
/// terminated one left.
fn reopened(bytes: &[u8]) -> WorkerEngine {
    let file = Kept::default();
    file.set_len(bytes.len() as u64).unwrap();
    file.write(0, bytes).unwrap();
    WorkerEngine::over(Store::on(file).unwrap()).unwrap()
}

/// Cancelling a run in the browser terminates its Worker: whatever the
/// store's file holds at that moment is what the next Worker opens. A
/// Worker writes through its file's handle as it goes, holding nothing back,
/// so the file at any of the run's looks at its progress is the file a
/// Worker terminated there leaves. Every such file opens, holding all that
/// was committed before the run and none of what the run had not finished.
#[test]
fn a_worker_terminated_mid_run_leaves_its_store_whole() {
    let file = Kept::default();
    let worker = WorkerEngine::over(Store::on(file.clone()).unwrap()).unwrap();
    worker.load_from(&maze(), None).unwrap();
    let engine = worker.engine();
    // Committed before the run: an experiment's results, and a move at the
    // point of a ref the package ships, which is fixed, with a ref of the
    // Worker's own named there and moved over it, and another naming it.
    let results = engine
        .run_experiment(&path("maze/micro"), &mut |_, _| ControlFlow::Continue(()))
        .unwrap();
    let start = engine.referred(&path("maze/micro")).unwrap();
    engine.refer(&path("maze/play"), start).unwrap();
    let moved = moved_on(engine, start);
    engine.advance(&path("maze/play"), start, moved).unwrap();
    engine.refer(&path("maze/kept"), moved).unwrap();

    // The experiment runs again, recording its trials as it goes; a Worker
    // is cut off at each of its looks, its trials' records among them.
    let mut cut = Vec::new();
    let again = engine
        .run_experiment(&path("maze/micro"), &mut |_, _| {
            cut.push(file.bytes());
            ControlFlow::Continue(())
        })
        .unwrap();
    assert!(cut.len() > 4, "{} looks", cut.len());
    for bytes in cut {
        let opened = reopened(&bytes);
        let engine = opened.engine();
        assert_eq!(engine.manifests().unwrap()[0].name, path("maze"));
        assert_eq!(engine.referred(&path("maze/micro")).unwrap(), start);
        assert_eq!(engine.referred(&path("maze/play")).unwrap(), moved);
        assert_eq!(engine.referred(&path("maze/kept")).unwrap(), moved);
        // The results kept are those the stopped run had not replaced yet.
        assert_eq!(engine.results(&path("maze/micro")).unwrap(), results);
        assert_eq!(engine.draw(moved).unwrap().frames.len(), 1);
        // And the store goes on: the next move is recorded and counts.
        let next = moved_on(engine, moved);
        engine.advance(&path("maze/play"), moved, next).unwrap();
    }
    // The run that went on recorded what it found.
    assert_eq!(engine.results(&path("maze/micro")).unwrap(), again);
}
