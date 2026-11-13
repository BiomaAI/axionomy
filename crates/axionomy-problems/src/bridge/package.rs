//! The bridge allocation as a package: its instances, the script its
//! searches and policies call, its goal, the travellers' priority, credit and
//! on-time objectives and the elapsed time, its strategies, and an experiment
//! comparing them on each instance.
//!
//! The exact Pareto searches run on Micro alone, as the study runs them:
//! timed traffic is compared over the policies it evaluates, and its study's
//! preference for a traveller is the best of those runs by that traveller's
//! on-time trips, priority and credit, which `pick` finds among the
//! experiment's results.

use crate::{
    bridge::{self, Ontology},
    package::{call, written},
};
use axionomy::path;
use axionomy_engine::{
    Arm, CandidateSet, Engine, EngineError, Experiment, Measure, Objective, ObjectivePath, Strategy,
};
use axionomy_search::pareto::ObjectiveDirection;
use axionomy_store::Checkpoints;

const SCRIPT: &str = include_str!("../../scripts/bridge.rhai");

/// The most turns a policy takes to clear the bridge's traffic.
const HORIZON: u64 = 150;

/// Writes the bridge's package into `engine`'s store, everything named
/// under `bridge`.
pub fn write(engine: &Engine) -> Result<(), EngineError> {
    let call = |function| call("bridge", function);
    engine.keep_script(&path!("bridge"), SCRIPT)?;
    engine.keep_goal(&path!("bridge/solved"), &written(&bridge::goal()))?;
    engine.keep_candidates(
        &path!("bridge/mechanisms"),
        &CandidateSet::Script(call("candidates")),
    )?;
    engine.keep_candidates(
        &path!("bridge/routes"),
        &CandidateSet::Script(call("routes")),
    )?;
    // Named by the keys the study's documents report them under.
    let objective =
        |key: &str| -> ObjectivePath { format!("bridge/{key}").parse().expect("a path") };
    for (key, function, direction) in [
        ("priority/a", "priority_a", ObjectiveDirection::Maximize),
        ("credit/a", "credit_a", ObjectiveDirection::Maximize),
        ("priority/b", "priority_b", ObjectiveDirection::Maximize),
        ("credit/b", "credit_b", ObjectiveDirection::Maximize),
        ("ontime/a", "on_time_a", ObjectiveDirection::Maximize),
        ("ontime/b", "on_time_b", ObjectiveDirection::Maximize),
        ("elapsed", "elapsed", ObjectiveDirection::Minimize),
    ] {
        engine.keep_objective(
            &objective(key),
            &Objective {
                measure: Measure::State(call(function)),
                direction,
            },
        )?;
    }
    let policy = |function| Strategy::Policy {
        policy: call(function),
        goal: path!("bridge/solved"),
        horizon: HORIZON,
    };
    // The exact front over both travellers' priority and credit, ranked by
    // one traveller's priority and credit, then the other's.
    let pareto = |by: [&str; 4]| Strategy::Pareto {
        candidates: path!("bridge/mechanisms"),
        goal: path!("bridge/solved"),
        objectives: ["priority/a", "priority/b", "credit/a", "credit/b"]
            .map(objective)
            .to_vec(),
        by: by.map(objective).to_vec(),
    };
    let strategies = [
        (
            "bridge/breadth-first",
            Strategy::BreadthFirst {
                candidates: path!("bridge/routes"),
                goal: path!("bridge/solved"),
            },
        ),
        ("bridge/first-come-a", policy("first_come_a")),
        ("bridge/first-come-b", policy("first_come_b")),
        ("bridge/auction", policy("auction")),
        ("bridge/adaptive", policy("adaptive")),
        (
            "bridge/pareto-a",
            pareto(["priority/a", "credit/a", "priority/b", "credit/b"]),
        ),
        (
            "bridge/pareto-b",
            pareto(["priority/b", "credit/b", "priority/a", "credit/a"]),
        ),
    ];
    for (name, strategy) in &strategies {
        engine.keep_strategy(&name.parse().expect("a path"), strategy)?;
    }
    for (instance, initial) in [
        ("micro", bridge::initial()),
        ("showcase", bridge::initial_showcase()),
        ("stress", bridge::initial_stress()),
    ] {
        // Micro is one round without a clock: its mechanisms are compared
        // on an exact front. Timed traffic adds the deadline-responsive
        // policy, and measures on-time trips and the time taken.
        let timed = bridge::timed::enabled(&initial);
        let arms: &[&str] = if timed {
            &[
                "bridge/breadth-first",
                "bridge/first-come-a",
                "bridge/first-come-b",
                "bridge/auction",
                "bridge/adaptive",
            ]
        } else {
            &[
                "bridge/breadth-first",
                "bridge/first-come-a",
                "bridge/first-come-b",
                "bridge/auction",
                "bridge/pareto-a",
                "bridge/pareto-b",
            ]
        };
        let mut objectives = ["priority/a", "credit/a", "priority/b", "credit/b"]
            .map(objective)
            .to_vec();
        if timed {
            objectives.extend(["ontime/a", "ontime/b", "elapsed"].map(objective));
        }
        let start = engine
            .store()
            .record::<Ontology>(initial, Checkpoints::Ends)
            .finish()?;
        let name = format!("bridge/{instance}");
        engine.refer(&name.parse().expect("a path"), start)?;
        engine.keep_experiment(
            &name.parse().expect("a path"),
            &Experiment {
                start: name.parse().expect("a path"),
                arms: arms
                    .iter()
                    .map(|strategy| Arm {
                        strategy: strategy.parse().expect("a path"),
                        start: None,
                    })
                    .collect(),
                objectives,
                seed: 17,
                budget: 10_000_000,
                trials: 1,
            },
        )?;
    }
    Ok(())
}
