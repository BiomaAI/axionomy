//! The stoichiometric workshop as a package: its instances, the script its
//! searches call, its goal, waste, time and revenue objectives, its
//! strategies, and an experiment comparing them on each instance.

use crate::{
    package::{call, written},
    workshop::{self, Ontology},
};
use axionomy::path;
use axionomy_engine::{
    Arm, CandidateSet, Engine, EngineError, Experiment, Measure, Objective, ObjectivePath, Strategy,
};
use axionomy_search::pareto::ObjectiveDirection;
use axionomy_store::Checkpoints;

const SCRIPT: &str = include_str!("../../scripts/workshop.rhai");

/// Writes the workshop's package into `engine`'s store, everything named
/// under `workshop`.
pub fn write(engine: &Engine) -> Result<(), EngineError> {
    let call = |function| call("workshop", function);
    engine.keep_script(&path!("workshop"), SCRIPT)?;
    engine.keep_goal(&path!("workshop/delivered"), &written(&workshop::goal()))?;
    engine.keep_candidates(
        &path!("workshop/recipes"),
        &CandidateSet::Script(call("candidates")),
    )?;
    let objectives: Vec<ObjectivePath> = vec![
        path!("workshop/waste"),
        path!("workshop/time"),
        path!("workshop/revenue"),
    ];
    for (objective, function, direction) in [
        (&objectives[0], "waste", ObjectiveDirection::Minimize),
        (&objectives[1], "spent_time", ObjectiveDirection::Minimize),
        (&objectives[2], "revenue", ObjectiveDirection::Maximize),
    ] {
        engine.keep_objective(
            objective,
            &Objective {
                measure: Measure::State(call(function)),
                direction,
            },
        )?;
    }
    let pareto = |by: &[&ObjectivePath]| Strategy::Pareto {
        candidates: path!("workshop/recipes"),
        goal: path!("workshop/delivered"),
        objectives: objectives.clone(),
        by: by.iter().map(|objective| (*objective).clone()).collect(),
    };
    let strategies = [
        (
            "workshop/breadth-first",
            Strategy::BreadthFirst {
                candidates: path!("workshop/recipes"),
                goal: path!("workshop/delivered"),
            },
        ),
        (
            "workshop/minimum-waste",
            Strategy::BestFirst {
                candidates: path!("workshop/recipes"),
                goal: path!("workshop/delivered"),
                cost: call("waste"),
                heuristic: None,
            },
        ),
        (
            "workshop/pareto-waste",
            pareto(&[&objectives[0], &objectives[1]]),
        ),
        (
            "workshop/pareto-time",
            pareto(&[&objectives[1], &objectives[0]]),
        ),
        ("workshop/pareto-revenue", pareto(&[&objectives[2]])),
    ];
    for (name, strategy) in &strategies {
        engine.keep_strategy(&name.parse().expect("a path"), strategy)?;
    }
    for (instance, initial) in [
        ("micro", workshop::initial()),
        ("showcase", workshop::initial_showcase()),
        ("stress", workshop::initial_stress()),
    ] {
        let start = engine
            .store()
            .record::<Ontology>(initial, Checkpoints::Ends)
            .finish()?;
        let name = format!("workshop/{instance}");
        engine.refer(&name.parse().expect("a path"), start)?;
        engine.keep_experiment(
            &name.parse().expect("a path"),
            &Experiment {
                start: name.parse().expect("a path"),
                arms: strategies
                    .iter()
                    .map(|(strategy, _)| Arm {
                        strategy: strategy.parse().expect("a path"),
                        start: None,
                    })
                    .collect(),
                objectives: objectives.clone(),
                seed: 17,
                budget: 10_000_000,
                trials: 1,
            },
        )?;
    }
    Ok(())
}
