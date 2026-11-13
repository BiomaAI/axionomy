//! The Sokoban warehouse as a package: its instances, the scripts its
//! searches and view call, its goal and strategies, an experiment comparing
//! them on each instance, and each instance's legal but losing plan,
//! recorded.

use crate::{
    package::{call, written},
    sokoban::{self, Ontology},
};
use axionomy::{OntologyPath, path, paths};
use axionomy_engine::{Arm, CandidateSet, Engine, EngineError, Experiment, Strategy, View};
use axionomy_store::Checkpoints;

/// The script the searches call, and the view's.
const SCRIPT: &str = include_str!("../../scripts/sokoban.rhai");
const VIEW: &str = include_str!("../../scripts/sokoban-view.rhai");

/// Writes the warehouse's package into `engine`'s store, everything named
/// under `sokoban`.
pub fn write(engine: &Engine) -> Result<(), EngineError> {
    let view = |function| Some(call("sokoban/view", function));
    let call = |function| call("sokoban", function);
    engine.keep_script(&path!("sokoban"), SCRIPT)?;
    engine.keep_script(&path!("sokoban/view"), VIEW)?;
    engine.keep_view(
        &OntologyPath::of::<Ontology>(),
        &View {
            title: "Sokoban".into(),
            labels: view("label"),
            formats: None,
            carry: None,
            scene: view("scene"),
            leaderboards: None,
        },
    )?;
    engine.keep_goal(&path!("sokoban/solved"), &written(&sokoban::goal()))?;
    engine.keep_candidates(
        &path!("sokoban/moves"),
        &CandidateSet::Script(call("candidates")),
    )?;
    let strategies = [
        (
            "sokoban/a-star",
            Strategy::Cheapest {
                candidates: path!("sokoban/moves"),
                goal: path!("sokoban/solved"),
                cost: call("step"),
                heuristic: Some(call("crate_goal_distance")),
            },
        ),
        (
            "sokoban/breadth-first",
            Strategy::BreadthFirst {
                candidates: path!("sokoban/moves"),
                goal: path!("sokoban/solved"),
            },
        ),
    ];
    for (name, strategy) in &strategies {
        engine.keep_strategy(&name.parse().expect("a path"), strategy)?;
    }
    for (instance, initial) in [
        ("micro", sokoban::initial()),
        ("showcase", sokoban::initial_showcase()),
        ("stress", sokoban::initial_stress()),
    ] {
        let losing = sokoban::losing_trace(&initial);
        let start = engine
            .store()
            .record::<Ontology>(initial, Checkpoints::Ends)
            .finish()?;
        let name = format!("sokoban/{instance}");
        engine.refer(&name.parse().expect("a path"), start)?;
        if let Some(losing) = losing {
            let exchanges: Vec<paths::Exchange> = losing.exchanges().iter().map(written).collect();
            let deadlocked = engine.apply(start, &exchanges)?;
            engine.refer(
                &format!("{name}/deadlock").parse().expect("a path"),
                deadlocked,
            )?;
        }
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
                objectives: Vec::new(),
                seed: 17,
                budget: 10_000_000,
                trials: 1,
            },
        )?;
    }
    Ok(())
}
