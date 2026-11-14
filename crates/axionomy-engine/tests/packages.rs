//! A package carries what an experiment needs, and re-runs it identically
//! wherever it is loaded, zipped or not.

use axionomy::{Quantity, Rate, basket, path::Path, paths};
use axionomy_engine::{
    Arm, CandidateSet, Contents, Effort, Engine, EngineError, Experiment, Manifest, Measure,
    Objective, PackageError, ScriptCall, Strategy, Version, package, zipped,
};
use axionomy_search::pareto::ObjectiveDirection;
use axionomy_store::Store;
use std::{ops::ControlFlow, path::PathBuf};

fn path<K>(text: &str) -> Path<K> {
    text.parse().unwrap()
}

fn call(function: &str) -> ScriptCall {
    ScriptCall::new(path("bank"), function.parse().unwrap())
}

/// A file in a directory of its own, removed when the test ends.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let directory = std::env::temp_dir().join(format!(
            "axionomy-packages-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        Self(directory)
    }

    fn file(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A bank paying alice, with what an experiment on it needs, and a run of
/// that experiment.
fn bank() -> Engine {
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
            r#"
            fn randomly(state, random) {
                exchange("pay", #{ from: "bank", to: "alice" }, random.next(5) + 1)
            }
            fn paid(state) { state.balance("alice", "cash") }
            fn payment(before, exchange, after) { 1 }
            "#,
        )
        .unwrap();
    // A script nothing in the package uses stays out of it.
    engine
        .keep_script(&path("bank/unused"), "fn nothing() { () }")
        .unwrap();
    engine
        .keep_goal(
            &path("bank/rich"),
            &paths::Goal::new().require(path("alice"), basket([(path("cash"), 10)])),
        )
        .unwrap();
    let pay = |units| {
        paths::Exchange::new(path("pay"), Quantity::new(units))
            .bind(path("from"), path("bank"))
            .bind(path("to"), path("alice"))
    };
    engine
        .keep_candidates(
            &path("bank/payments"),
            &CandidateSet::Listed(vec![pay(1), pay(5)]),
        )
        .unwrap();
    engine
        .keep_strategy(
            &path("bank/randomly"),
            &Strategy::Policy {
                policy: call("randomly"),
            },
        )
        .unwrap();
    engine
        .keep_strategy(
            &path("bank/searched"),
            &Strategy::BreadthFirst {
                candidates: path("bank/payments"),
            },
        )
        .unwrap();
    for (objective, measure, direction) in [
        (
            "bank/payments",
            Measure::Steps(call("payment")),
            ObjectiveDirection::Minimize,
        ),
        (
            "bank/paid",
            Measure::State(call("paid")),
            ObjectiveDirection::Maximize,
        ),
    ] {
        engine
            .keep_objective(&path(objective), &Objective { measure, direction })
            .unwrap();
    }
    let start = engine.author(economy).unwrap();
    engine.refer(&path("bank/start"), start).unwrap();
    engine
        .keep_experiment(
            &path("bank/paying"),
            &Experiment {
                start: path("bank/start"),
                goal: path("bank/rich"),
                arms: ["bank/randomly", "bank/searched"]
                    .iter()
                    .map(|strategy| Arm {
                        strategy: path(strategy),
                        start: None,
                    })
                    .collect(),
                objectives: vec![path("bank/payments"), path("bank/paid")],
                effort: Effort {
                    seed: 5,
                    budget: 1_000,
                    turns: 20,
                },
                trials: 3,
            },
        )
        .unwrap();
    run(&engine);
    engine
}

fn run(engine: &Engine) -> axionomy_engine::Results {
    engine
        .run_experiment(&path("bank/paying"), &mut |_, _| ControlFlow::Continue(()))
        .unwrap()
}

fn version() -> Version {
    "1.2.0".parse().unwrap()
}

/// `engine`'s experiment exported as package `bank` into a compacted file.
fn exported(engine: &Engine, scratch: &Scratch) -> (Manifest, Vec<u8>) {
    let file = scratch.file("bank.redb");
    let manifest = {
        let to = Store::open(&file).unwrap();
        engine
            .export(
                &path("bank"),
                version(),
                Contents {
                    refs: Vec::new(),
                    definitions: vec![path("experiment/bank/paying")],
                },
                &to,
            )
            .unwrap()
    };
    Store::compact(&file).unwrap();
    (manifest, std::fs::read(&file).unwrap())
}

/// Every trial's seed, how it ended, its values and its exchanges.
fn trials(engine: &Engine, results: &axionomy_engine::Results) -> Vec<String> {
    results
        .arms
        .iter()
        .flat_map(|arm| &arm.trials)
        .map(|trial| {
            format!(
                "{} {:?} {:?} {:?}",
                trial.seed,
                trial.ended,
                trial.values,
                engine.history(trial.end).unwrap().1
            )
        })
        .collect()
}

#[test]
fn a_package_carries_what_its_experiment_needs_and_reruns_it() {
    let scratch = Scratch::new("rerun");
    let source = bank();
    let (manifest, bytes) = exported(&source, &scratch);
    assert_eq!(manifest.name.as_str(), "bank");
    assert_eq!(manifest.version.to_string(), "1.2.0");
    assert_eq!(
        manifest
            .refs
            .iter()
            .map(|reference| reference.as_str())
            .collect::<Vec<_>>(),
        ["bank/start"]
    );
    let carried: Vec<&str> = manifest
        .definitions
        .iter()
        .map(|definition| definition.as_str())
        .collect();
    assert_eq!(
        carried,
        [
            "candidates/bank/payments",
            "experiment/bank/paying",
            "goal/bank/rich",
            "objective/bank/paid",
            "objective/bank/payments",
            "results/bank/paying",
            "script/bank",
            "strategy/bank/randomly",
            "strategy/bank/searched",
        ]
    );

    for file in [bytes.clone(), zipped(&bytes).unwrap()] {
        let loaded = Engine::new(Store::memory().unwrap()).unwrap();
        let outcome = loaded.load(&package(&file).unwrap()).unwrap();
        assert_eq!(outcome.manifests, std::slice::from_ref(&manifest));
        assert!(outcome.kept.refs.is_empty() && outcome.kept.definitions.is_empty());
        assert_eq!(loaded.manifests().unwrap(), std::slice::from_ref(&manifest));
        // The results came with the package, and running the experiment
        // again gives them again.
        let carried = loaded.results(&path("bank/paying")).unwrap();
        assert_eq!(carried, source.results(&path("bank/paying")).unwrap());
        let again = run(&loaded);
        assert_eq!(trials(&loaded, &again), trials(&source, &carried));
        assert!(loaded.script(&path("bank/unused")).unwrap().is_none());
    }
}

fn pay(units: u64) -> paths::Exchange {
    paths::Exchange::new(path("pay"), Quantity::new(units))
        .bind(path("from"), path("bank"))
        .bind(path("to"), path("alice"))
}

/// `engine`'s experiment and `refs` as package `bank` at `version`, in a
/// store in memory.
fn release(engine: &Engine, version: &str, refs: &[&str]) -> Store {
    let to = Store::memory().unwrap();
    engine
        .export(
            &path("bank"),
            version.parse().unwrap(),
            Contents {
                refs: refs.iter().map(|reference| path(reference)).collect(),
                definitions: vec![path("experiment/bank/paying")],
            },
            &to,
        )
        .unwrap();
    to
}

fn shipped(result: Result<(), EngineError>) -> bool {
    matches!(result, Err(EngineError::Shipped { package, .. }) if package.as_str() == "bank")
}

#[test]
fn another_version_replaces_what_the_first_shipped() {
    let source = bank();
    let start = source.referred(&path("bank/start")).unwrap();
    source.refer(&path("bank/other"), start).unwrap();
    // The first version counts a payment with scripts it imports.
    source
        .keep_script(&path("bank/count"), "fn one() { 1 }")
        .unwrap();
    source
        .keep_script(&path("bank/old"), "fn none() { 0 }")
        .unwrap();
    source
        .keep_script(
            &path("bank"),
            r#"
            import "bank/count" as count;
            import "bank/old" as old;
            fn randomly(state, random) {
                exchange("pay", #{ from: "bank", to: "alice" }, random.next(5) + 1)
            }
            fn paid(state) { state.balance("alice", "cash") }
            fn payment(before, exchange, after) { count::one() + old::none() }
            "#,
        )
        .unwrap();
    let first = release(&source, "1.0.0", &["bank/other"]);

    let engine = Engine::new(Store::memory().unwrap()).unwrap();
    engine.load(&first).unwrap();
    // What a person makes of it: a ref of their own under the package's
    // name moved over a step of theirs, the experiment run again, a goal,
    // and a ref the next version happens to ship too.
    let live = path("bank/live");
    engine.refer(&live, start).unwrap();
    let played = engine.apply(start, &[pay(2)]).unwrap();
    engine.advance(&live, start, played).unwrap();
    let rerun = run(&engine);
    engine
        .keep_goal(&path("bank/mine"), &paths::Goal::new())
        .unwrap();
    engine.refer(&path("bank/extra"), played).unwrap();
    // The refs the package ships are fixed.
    assert!(shipped(engine.advance(&path("bank/start"), start, played)));
    assert!(shipped(engine.delete_ref(&path("bank/start"), start)));
    assert!(shipped(engine.delete_ref(&path("bank/other"), start)));
    assert_eq!(engine.referred(&path("bank/start")).unwrap(), start);
    // Alice holding ten is rich, as the first version's goal says; and one
    // payment is one, as its scripts count it, the one it imports evaluated
    // and kept.
    let ten = engine.apply(start, &[pay(10)]).unwrap();
    assert!(engine.holds(ten, &path("bank/rich")).unwrap());
    assert_eq!(
        engine.measure(played, &path("bank/payments")).unwrap(),
        Quantity::new(1)
    );

    // The second version asks more of its goal, counts a payment twice with
    // the script it imports, no longer imports `bank/old`, no longer
    // searches, starts a step on, ships `bank/extra` and no longer ships
    // `bank/other`.
    source
        .keep_goal(
            &path("bank/rich"),
            &paths::Goal::new().require(path("alice"), basket([(path("cash"), 12)])),
        )
        .unwrap();
    source
        .keep_script(&path("bank/count"), "fn one() { 2 }")
        .unwrap();
    source
        .keep_script(
            &path("bank"),
            r#"
            import "bank/count" as count;
            fn randomly(state, random) {
                exchange("pay", #{ from: "bank", to: "alice" }, random.next(5) + 1)
            }
            fn paid(state) { state.balance("alice", "cash") }
            fn payment(before, exchange, after) { count::one() }
            "#,
        )
        .unwrap();
    let mut experiment = source.experiment(&path("bank/paying")).unwrap();
    experiment.arms.truncate(1);
    source
        .keep_experiment(&path("bank/paying"), &experiment)
        .unwrap();
    let later = source.apply(start, &[pay(1)]).unwrap();
    source.advance(&path("bank/start"), start, later).unwrap();
    source.refer(&path("bank/extra"), later).unwrap();
    let results = run(&source);
    let second = release(&source, "2.0.0", &["bank/extra"]);

    let loaded = engine.load(&second).unwrap();
    assert_eq!(loaded.replaced.len(), 1);
    assert_eq!(loaded.replaced[0].version.to_string(), "1.0.0");
    // The person's `bank/extra` names their point, so it keeps it.
    assert_eq!(loaded.kept.refs, [path("bank/extra")]);
    assert!(loaded.kept.definitions.is_empty());

    // What only the first version shipped is gone.
    assert!(matches!(
        engine.referred(&path("bank/other")),
        Err(EngineError::UnknownRef(_))
    ));
    assert!(matches!(
        engine.strategy(&path("bank/searched")),
        Err(EngineError::Undefined(_))
    ));
    assert!(matches!(
        engine.candidates(start, &path("bank/payments")),
        Err(EngineError::Undefined(_))
    ));
    // Among it a script the first version's scripts imported.
    assert!(
        loaded.replaced[0]
            .definitions
            .contains(&path("script/bank/old"))
    );
    assert_eq!(engine.script(&path("bank/old")).unwrap(), None);
    // What both carry is the second's, its results and scripts among it,
    // the scripts it imports evaluated again.
    assert_eq!(engine.referred(&path("bank/start")).unwrap(), later);
    assert!(!engine.holds(ten, &path("bank/rich")).unwrap());
    assert_eq!(engine.experiment(&path("bank/paying")).unwrap(), experiment);
    assert_eq!(engine.results(&path("bank/paying")).unwrap(), results);
    assert_ne!(engine.results(&path("bank/paying")).unwrap(), rerun);
    assert_eq!(
        engine.measure(played, &path("bank/payments")).unwrap(),
        Quantity::new(2)
    );
    let manifests = engine.manifests().unwrap();
    assert_eq!(manifests.len(), 1);
    assert_eq!(manifests[0].version.to_string(), "2.0.0");
    assert_eq!(manifests[0].refs, [path("bank/start")]);

    // What the person made stays, and so do the runs: every point anything
    // named is still reached.
    assert_eq!(engine.referred(&live).unwrap(), played);
    assert_eq!(engine.history(played).unwrap().1, [pay(2)]);
    assert_eq!(engine.referred(&path("bank/extra")).unwrap(), played);
    assert!(engine.goal(&path("bank/mine")).is_ok());
    for trial in rerun.arms.iter().flat_map(|arm| &arm.trials) {
        engine.reach(trial.end).unwrap();
    }
    engine.reach(start).unwrap();

    // The person's refs move; the package's stays fixed.
    assert_eq!(engine.shipped_by(&live).unwrap(), None);
    assert_eq!(engine.shipped_by(&path("bank/extra")).unwrap(), None);
    assert_eq!(
        engine.shipped_by(&path("bank/start")).unwrap(),
        Some(path("bank"))
    );
    let further = engine.apply(played, &[pay(1)]).unwrap();
    engine.advance(&live, played, further).unwrap();
    engine
        .advance(&path("bank/extra"), played, further)
        .unwrap();
    assert!(shipped(engine.advance(&path("bank/start"), later, further)));

    // The same version again changes nothing, not even what the person
    // changed since.
    let again = run(&engine);
    let unchanged = engine.load(&second).unwrap();
    assert!(unchanged.replaced.is_empty());
    assert!(unchanged.kept.refs.is_empty() && unchanged.kept.definitions.is_empty());
    assert_eq!(engine.results(&path("bank/paying")).unwrap(), again);
    assert_eq!(engine.referred(&path("bank/extra")).unwrap(), further);
    assert_eq!(engine.manifests().unwrap(), manifests);
}

#[test]
fn a_package_names_its_refs_under_its_name() {
    let source = bank();
    let start = source.referred(&path("bank/start")).unwrap();
    source.refer(&path("elsewhere"), start).unwrap();
    let refused = source.export(
        &path("bank"),
        version(),
        Contents {
            refs: vec![path("elsewhere")],
            definitions: Vec::new(),
        },
        &Store::memory().unwrap(),
    );
    assert!(matches!(
        refused,
        Err(EngineError::Package(PackageError::Outside { .. }))
    ));
}

#[test]
fn loading_checks_the_package() {
    let scratch = Scratch::new("checks");
    let (mut manifest, bytes) = exported(&bank(), &scratch);
    let engine = Engine::new(Store::memory().unwrap()).unwrap();
    assert!(matches!(
        engine.load(&Store::memory().unwrap()),
        Err(EngineError::Package(PackageError::NoManifest))
    ));

    // A package written for a newer script host API is refused, and
    // nothing of it is merged.
    let newer = package(&bytes).unwrap();
    manifest.host_api = axionomy_engine::HOST_API + 1;
    let encoded = postcard::to_allocvec(&manifest).unwrap();
    newer
        .transact(axionomy_store::Durability::Immediate, move |txn| {
            txn.define(&path("package/bank"), Some(&encoded))
        })
        .wait()
        .unwrap();
    assert!(matches!(
        engine.load(&newer),
        Err(EngineError::Package(PackageError::HostApi { .. }))
    ));
    assert!(engine.manifests().unwrap().is_empty());

    // So is one whose manifest says it is written in another store format.
    let other = package(&bytes).unwrap();
    manifest.host_api = axionomy_engine::HOST_API;
    manifest.format = axionomy_store::FORMAT_VERSION + 1;
    let encoded = postcard::to_allocvec(&manifest).unwrap();
    other
        .transact(axionomy_store::Durability::Immediate, move |txn| {
            txn.define(&path("package/bank"), Some(&encoded))
        })
        .wait()
        .unwrap();
    assert!(matches!(
        engine.load(&other),
        Err(EngineError::Package(PackageError::Format { .. }))
    ));
    assert!(engine.manifests().unwrap().is_empty());

    assert!(matches!(
        package(b"PK\x03\x04 not really a zip"),
        Err(EngineError::Package(PackageError::Archive(_)))
    ));
}

#[test]
fn a_package_carries_the_scripts_its_scripts_import() {
    let scratch = Scratch::new("imports");
    let source = Engine::new(Store::memory().unwrap()).unwrap();
    // The bank's script imports its amounts, which import its limits.
    for (script, text) in [
        ("bank/limits", "fn ceiling() { 3 }"),
        (
            "bank/amounts",
            r#"
            import "bank/limits" as limits;
            fn amounts() {
                let found = [];
                for units in 1..=limits::ceiling() { found.push(units); }
                found
            }
            "#,
        ),
        (
            "bank",
            r#"
            import "bank/amounts" as amounts;
            fn payments(state) {
                amounts::amounts()
                    .map(|units| exchange("pay", #{ from: "bank", to: "alice" }, units))
                    .filter(|payment| state.applicable(payment))
            }
            fn paid(state) { state.balance("alice", "cash") * amounts::limits::ceiling() }
            "#,
        ),
        ("bank/unused", "fn nothing() { () }"),
    ] {
        source.keep_script(&path(script), text).unwrap();
    }
    source
        .keep_candidates(
            &path("bank/payments"),
            &CandidateSet::Script(call("payments")),
        )
        .unwrap();
    source
        .keep_objective(
            &path("bank/paid"),
            &Objective {
                measure: Measure::State(call("paid")),
                direction: ObjectiveDirection::Maximize,
            },
        )
        .unwrap();
    let economy = paths::EconomyBuilder::new()
        .account(path("bank"), basket([(path("cash"), 2)]).into())
        .account(path("alice"), basket([(path("cash"), 2)]).into())
        .rate(
            path("pay"),
            Rate::new()
                .consume(path("from"), basket([(path("cash"), 1)]))
                .produce(path("to"), basket([(path("cash"), 1)])),
        )
        .build()
        .unwrap();
    source
        .refer(&path("bank/start"), source.author(economy).unwrap())
        .unwrap();

    let file = scratch.file("bank.redb");
    let manifest = source
        .export(
            &path("bank"),
            version(),
            Contents {
                refs: vec![path("bank/start")],
                definitions: vec![
                    path("candidates/bank/payments"),
                    path("objective/bank/paid"),
                ],
            },
            &Store::open(&file).unwrap(),
        )
        .unwrap();
    let carried: Vec<&str> = manifest
        .definitions
        .iter()
        .map(|definition| definition.as_str())
        .collect();
    assert_eq!(
        carried,
        [
            "candidates/bank/payments",
            "objective/bank/paid",
            "script/bank",
            "script/bank/amounts",
            "script/bank/limits",
        ]
    );
    Store::compact(&file).unwrap();
    let bytes = std::fs::read(&file).unwrap();

    let answers = |engine: &Engine| {
        let start = engine.referred(&path("bank/start")).unwrap();
        (
            engine.candidates(start, &path("bank/payments")).unwrap(),
            engine.measure(start, &path("bank/paid")).unwrap(),
        )
    };
    let (payments, paid) = answers(&source);
    assert_eq!(payments.len(), 2);
    assert_eq!(paid, Quantity::new(6));
    for file in [bytes.clone(), zipped(&bytes).unwrap()] {
        let loaded = Engine::new(Store::memory().unwrap()).unwrap();
        loaded.load(&package(&file).unwrap()).unwrap();
        assert_eq!(answers(&loaded), (payments.clone(), paid));
        assert!(loaded.script(&path("bank/unused")).unwrap().is_none());
    }
}
