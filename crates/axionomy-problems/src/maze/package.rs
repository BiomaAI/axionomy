//! The key-door maze as a package: its instances, the scripts its searches
//! and view call, its goal, objectives and strategies, and an experiment
//! comparing every strategy on each instance.

use crate::maze::{self, Ontology};
use axionomy::{OntologyPath, path};
use axionomy_engine::{
    Arm, CandidateSet, Engine, EngineError, Experiment, Measure, Objective, ScriptCall, Strategy,
    View,
};
use axionomy_search::pareto::ObjectiveDirection;
use axionomy_store::Checkpoints;

/// The script the searches call, and the view's.
const SCRIPT: &str = include_str!("../../scripts/maze.rhai");
const VIEW: &str = include_str!("../../scripts/maze-view.rhai");

fn call(function: &str) -> ScriptCall {
    ScriptCall::new(path!("maze"), function.parse().expect("a function name"))
}

/// Writes the maze's package into `engine`'s store, everything named under
/// `maze`.
pub fn write(engine: &Engine) -> Result<(), EngineError> {
    engine.keep_script(&path!("maze"), SCRIPT)?;
    engine.keep_script(&path!("maze/view"), VIEW)?;
    let view = |function: &str| {
        Some(ScriptCall::new(
            path!("maze/view"),
            function.parse().expect("a function name"),
        ))
    };
    engine.keep_view(
        &OntologyPath::of::<Ontology>(),
        &View {
            title: "Key-door maze".into(),
            labels: view("label"),
            formats: view("format"),
            carry: None,
            scene: view("scene"),
            leaderboards: None,
        },
    )?;
    engine.keep_goal(&path!("maze/solved"), &written(&maze::goal()))?;
    engine.keep_candidates(
        &path!("maze/moves"),
        &CandidateSet::Script(call("candidates")),
    )?;
    for (objective, function) in [("maze/energy", "spent_energy"), ("maze/time", "spent_time")] {
        engine.keep_objective(
            &objective.parse().expect("a path"),
            &Objective {
                measure: Measure::State(call(function)),
                direction: ObjectiveDirection::Minimize,
            },
        )?;
    }
    let search = |heuristic: Option<ScriptCall>| Strategy::Cheapest {
        candidates: path!("maze/moves"),
        goal: path!("maze/solved"),
        cost: call("energy"),
        heuristic,
    };
    let pareto = |first: &str, second: &str| Strategy::Pareto {
        candidates: path!("maze/moves"),
        goal: path!("maze/solved"),
        objectives: vec![
            first.parse().expect("a path"),
            second.parse().expect("a path"),
        ],
    };
    let strategies = [
        (
            "maze/breadth-first",
            Strategy::BreadthFirst {
                candidates: path!("maze/moves"),
                goal: path!("maze/solved"),
            },
        ),
        ("maze/dijkstra", search(None)),
        ("maze/a-star", search(Some(call("heuristic")))),
        ("maze/pareto-energy", pareto("maze/energy", "maze/time")),
        ("maze/pareto-time", pareto("maze/time", "maze/energy")),
    ];
    for (name, strategy) in &strategies {
        engine.keep_strategy(&name.parse().expect("a path"), strategy)?;
    }
    for (instance, initial) in [
        ("micro", maze::initial()),
        ("showcase", maze::initial_showcase()),
        ("stress", maze::initial_stress()),
    ] {
        let start = engine
            .store()
            .record::<Ontology>(initial, Checkpoints::Ends)
            .finish()?;
        let name = format!("maze/{instance}");
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
                objectives: vec![path!("maze/energy"), path!("maze/time")],
                seed: 17,
                budget: 10_000_000,
                trials: 1,
            },
        )?;
    }
    Ok(())
}

/// A typed value written as paths, which D-044 makes the same bytes.
fn written<T: serde::de::DeserializeOwned>(value: &impl serde::Serialize) -> T {
    postcard::from_bytes(&postcard::to_allocvec(value).expect("values encode"))
        .expect("a typed value reads as paths")
}
