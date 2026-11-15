//! The Sokoban warehouse as a package: its instances, the scripts its
//! searches and view call, its goal and strategies, an experiment comparing
//! them on each instance, and each instance's legal but losing plan,
//! recorded.

use crate::{
    package::{call, written},
    sokoban::{self, Ontology, RateId},
};
use axionomy::{OntologyPath, path, paths};
use axionomy_engine::{Arm, CandidateSet, Effort, Engine, EngineError, Experiment, Strategy, View};
use axionomy_store::Checkpoints;

/// The script the searches call, and the view's.
const SCRIPT: &str = include_str!("../../scripts/sokoban.rhai");
const VIEW: &str = include_str!("../../scripts/sokoban-view.rhai");

/// A legal plan at each instance that pushes a crate onto a square it can
/// never leave: the `deadlock` ref after it is lost, though nothing in it
/// was refused.
const MICRO_DEADLOCK: &[RateId] = &[
    RateId::Move { from: 8, to: 9 },
    RateId::Push {
        behind: 9,
        crate_at: 10,
        to: 11,
        crate_id: 0,
    },
    RateId::Push {
        behind: 10,
        crate_at: 11,
        to: 12,
        crate_id: 0,
    },
];
const SHOWCASE_DEADLOCK: &[RateId] = &[
    RateId::Push {
        behind: 38,
        crate_at: 29,
        to: 20,
        crate_id: 0,
    },
    RateId::Move { from: 29, to: 30 },
    RateId::Move { from: 30, to: 21 },
    RateId::Push {
        behind: 21,
        crate_at: 20,
        to: 19,
        crate_id: 0,
    },
];
const STRESS_DEADLOCK: &[RateId] = &[
    RateId::Push {
        behind: 54,
        crate_at: 41,
        to: 28,
        crate_id: 0,
    },
    RateId::Move { from: 41, to: 42 },
    RateId::Move { from: 42, to: 29 },
    RateId::Push {
        behind: 29,
        crate_at: 28,
        to: 27,
        crate_id: 0,
    },
];

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
    // Every rate fixes each role, so the moves are the instances that apply.
    engine.keep_candidates(
        &path!("sokoban/moves"),
        &CandidateSet::Instances { under: None },
    )?;
    let strategies = [
        (
            "sokoban/a-star",
            Strategy::Cheapest {
                candidates: path!("sokoban/moves"),
                cost: call("step"),
                heuristic: Some(call("crate_goal_distance")),
            },
        ),
        (
            "sokoban/breadth-first",
            Strategy::BreadthFirst {
                candidates: path!("sokoban/moves"),
            },
        ),
    ];
    for (name, strategy) in &strategies {
        engine.keep_strategy(&name.parse().expect("a path"), strategy)?;
    }
    for (instance, initial, losing) in [
        ("micro", sokoban::initial(), MICRO_DEADLOCK),
        ("showcase", sokoban::initial_showcase(), SHOWCASE_DEADLOCK),
        ("stress", sokoban::initial_stress(), STRESS_DEADLOCK),
    ] {
        let losing: Vec<paths::Exchange> = losing
            .iter()
            .map(|rate| written(&sokoban::exchange(&initial, *rate)))
            .collect();
        let start = engine
            .store()
            .record::<Ontology>(initial, Checkpoints::Ends)
            .finish()?;
        let name = format!("sokoban/{instance}");
        engine.refer(&name.parse().expect("a path"), start)?;
        let deadlocked = engine.apply(start, &losing)?;
        engine.refer(
            &format!("{name}/deadlock").parse().expect("a path"),
            deadlocked,
        )?;
        engine.keep_experiment(
            &name.parse().expect("a path"),
            &Experiment {
                start: name.parse().expect("a path"),
                goal: path!("sokoban/solved"),
                arms: strategies
                    .iter()
                    .map(|(strategy, _)| Arm {
                        strategy: strategy.parse().expect("a path"),
                        start: None,
                    })
                    .collect(),
                objectives: Vec::new(),
                effort: Effort {
                    seed: 17,
                    budget: 10_000_000,
                    // Its strategies search or plan; none plays turns.
                    turns: 0,
                },
                trials: 1,
            },
        )?;
    }
    Ok(())
}
