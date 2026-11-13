//! The samples as packages (P9): each package, loaded alone into a fresh
//! store, runs experiments that reproduce its study's outcomes, objectives
//! and traces.
#![cfg(feature = "packages")]

use axionomy::{path, paths};
use axionomy_engine::{Engine, Version};
use axionomy_problems::{maze, registry};
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
fn loaded(name: &str, write: fn(&Engine) -> Result<(), axionomy_engine::EngineError>) -> Engine {
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

/// Every document of the study of `problem` at `instance` that names one of
/// `strategies` comes out of the package's experiment: the same exchanges,
/// and the same value on every objective the document reports.
fn reproduces(engine: &Engine, problem: &str, instance: &str, strategies: &[&str]) {
    let experiment = format!("{problem}/{instance}").parse().unwrap();
    let results = engine
        .run_experiment(&experiment, &mut |_| ControlFlow::Continue(()))
        .unwrap();
    let request =
        StudyRequest::new(problem.parse().unwrap()).with_instance(instance.parse().unwrap());
    let mut observed = Trajectories::default();
    let study = registry()
        .study_with(&request, &StudyControl::default(), &mut observed)
        .unwrap();
    for strategy in strategies {
        let arm = results
            .arms
            .iter()
            .find(|arm| arm.arm.strategy.as_str() == format!("{problem}/{strategy}"))
            .unwrap_or_else(|| panic!("no arm runs {strategy}"));
        let (_, trace) = observed
            .0
            .iter()
            .find(|(document, _)| document.as_str() == *strategy)
            .unwrap_or_else(|| panic!("no document of {strategy}"));
        let document = study
            .documents
            .iter()
            .find(|document| document.id.as_str() == *strategy)
            .unwrap();
        let trial = &arm.trials[0];
        let (_, exchanges) = engine.history(trial.end).unwrap();
        assert_eq!(
            exchanges,
            trace.exchanges(),
            "{problem} {instance} {strategy}: the trace"
        );
        let objectives: Vec<String> = document
            .objectives
            .iter()
            .map(|objective| objective.value.to_string())
            .collect();
        let values: Vec<String> = trial.values.iter().map(ToString::to_string).collect();
        assert_eq!(
            values, objectives,
            "{problem} {instance} {strategy}: the objectives"
        );
    }
}

#[test]
fn maze_reproduces_its_study() {
    let engine = loaded("maze", maze::package::write);
    for instance in ["micro", "showcase", "stress"] {
        reproduces(
            &engine,
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
    // The package carries maze's view, so its runs draw as maze.
    let start = engine.referred(&path!("maze/micro")).unwrap();
    assert_eq!(engine.draw(start).unwrap().title, "Key-door maze");
}
