//! The samples as packages (P9): each package, loaded alone into a fresh
//! store, runs experiments that reproduce its study's outcomes, objectives
//! and traces.
#![cfg(feature = "packages")]

use axionomy::{path, paths};
use axionomy_engine::{Engine, Version};
use axionomy_problems::registry;
use axionomy_store::Store;
use axionomy_study::{Progress, StudyControl, StudyObserver, StudyRequest};
use axionomy_view::DocumentPath;
use std::ops::ControlFlow;

#[derive(Default)]
struct Trajectories(Vec<(DocumentPath, paths::Trace)>);

impl StudyObserver for Trajectories {
    fn progress(&mut self, _: Progress) {}

    fn trajectory(&mut self, document: &DocumentPath, _: &paths::Economy, trace: &paths::Trace) {
        self.0.push((document.clone(), trace.clone()));
    }
}

/// `write`'s package, exported from the store it was written in and
/// loaded alone into a fresh one.
fn loaded(name: &str, write: axionomy_problems::WritePackage) -> Engine {
    let source = Engine::new(Store::memory().unwrap()).unwrap();
    write(&source).unwrap();
    let name = name.parse().unwrap();
    let file = Store::memory().unwrap();
    source
        .export(
            &name,
            "0.1.0".parse::<Version>().unwrap(),
            source.named_under(&name).unwrap(),
            &file,
        )
        .unwrap();
    let engine = Engine::new(Store::memory().unwrap()).unwrap();
    engine.load(&file).unwrap();
    engine
}

/// The study of `problem` at `instance`, asked for `strategy` or its
/// default, and the trajectory of each of its documents.
fn study(
    problem: &str,
    instance: &str,
    strategy: Option<&str>,
) -> (axionomy_study::Study, Vec<(DocumentPath, paths::Trace)>) {
    let mut request =
        StudyRequest::new(problem.parse().unwrap()).with_instance(instance.parse().unwrap());
    if let Some(strategy) = strategy {
        request = request.with_strategy(strategy.parse().unwrap());
    }
    let mut observed = Trajectories::default();
    let study = registry()
        .study_with(&request, &StudyControl::default(), &mut observed)
        .unwrap();
    (study, observed.0)
}

/// The study of `problem` at `instance`, asked for each of `strategies`,
/// gives a document named for the strategy that the package's experiment
/// at `instance` reproduces: the arm running `<problem>/<strategy>` reaches
/// the document's exchanges, and its value on each objective the document
/// reports, the experiment's objective of that name.
fn reproduces(engine: &Engine, problem: &str, instance: &str, strategies: &[&str]) {
    let experiment = format!("{problem}/{instance}").parse().unwrap();
    let objectives = engine.experiment(&experiment).unwrap().objectives;
    let results = engine
        .run_experiment(&experiment, &mut |_| ControlFlow::Continue(()))
        .unwrap();
    let default = study(problem, instance, None);
    for strategy in strategies {
        // A study holds documents for its default and other strategies; one
        // asked for its strategy is studied only when that one is missing.
        let has = |(study, _): &(axionomy_study::Study, _)| {
            study
                .documents
                .iter()
                .any(|document| document.id.as_str() == *strategy)
        };
        let asked;
        let (study, trajectories) = if has(&default) {
            &default
        } else {
            asked = study(problem, instance, Some(strategy));
            &asked
        };
        let arm = results
            .arms
            .iter()
            .find(|arm| arm.arm.strategy.as_str() == format!("{problem}/{strategy}"))
            .unwrap_or_else(|| panic!("no arm runs {strategy}"));
        let (_, trace) = trajectories
            .iter()
            .find(|(document, _)| document.as_str() == *strategy)
            .unwrap_or_else(|| panic!("no document of {strategy}"));
        let trial = &arm.trials[0];
        let (_, exchanges) = engine.history(trial.end).unwrap();
        assert_eq!(
            exchanges,
            trace.exchanges(),
            "{problem} {instance} {strategy}: the trace"
        );
        let document = study
            .documents
            .iter()
            .find(|document| document.id.as_str() == *strategy)
            .unwrap();
        for reported in &document.objectives {
            let index = objectives
                .iter()
                .position(|objective| {
                    objective.as_str().rsplit('/').next() == Some(reported.key.as_str())
                })
                .unwrap_or_else(|| panic!("{problem} measures no `{}`", reported.key));
            assert_eq!(
                trial.values[index], reported.value,
                "{problem} {instance} {strategy}: {}",
                reported.key
            );
        }
    }
}

/// The document `document` of the study of `problem` at `instance` is the
/// history of the package's ref `reference`.
fn recorded(engine: &Engine, problem: &str, instance: &str, document: &str, reference: &str) {
    let (_, trajectories) = study(problem, instance, None);
    let (_, trace) = trajectories
        .iter()
        .find(|(id, _)| id.as_str() == document)
        .unwrap_or_else(|| panic!("no document {document}"));
    let point = engine.referred(&reference.parse().unwrap()).unwrap();
    assert_eq!(
        engine.history(point).unwrap().1,
        trace.exchanges(),
        "{problem} {instance} {document}"
    );
}

/// What each sample checks of its package at `instance`.
fn maze(engine: &Engine, instance: &str) {
    reproduces(
        engine,
        "maze",
        instance,
        &[
            "breadth-first",
            "dijkstra",
            "a-star",
            "pareto-energy",
            "pareto-time",
        ],
    );
}

fn sokoban(engine: &Engine, instance: &str) {
    reproduces(engine, "sokoban", instance, &["a-star", "breadth-first"]);
    recorded(
        engine,
        "sokoban",
        instance,
        "deadlock",
        &format!("sokoban/{instance}/deadlock"),
    );
}

/// Every sample, its package builder and its check.
type Sample = (
    &'static str,
    axionomy_problems::WritePackage,
    fn(&Engine, &str),
);

const SAMPLES: [Sample; 2] = [
    ("maze", axionomy_problems::maze::package::write, maze),
    (
        "sokoban",
        axionomy_problems::sokoban::package::write,
        sokoban,
    ),
];

/// Each sample's package reproduces its study at `instances`.
fn samples_reproduce(instances: &[&str]) {
    for (name, write, check) in SAMPLES {
        let engine = loaded(name, write);
        for instance in instances {
            check(&engine, instance);
        }
    }
}

#[test]
fn every_sample_reproduces_its_small_instances() {
    samples_reproduce(&["micro", "showcase"]);
}

#[test]
#[ignore = "minutes in a debug build; CI runs it in a release build"]
fn every_sample_reproduces_its_stress_instance() {
    samples_reproduce(&["stress"]);
}

#[test]
fn a_package_carries_its_view() {
    let engine = loaded("maze", axionomy_problems::maze::package::write);
    let start = engine.referred(&path!("maze/micro")).unwrap();
    assert_eq!(engine.draw(start).unwrap().title, "Key-door maze");
}
