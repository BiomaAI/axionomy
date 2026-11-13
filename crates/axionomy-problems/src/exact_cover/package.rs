//! Exact cover as a package: its instances and each one's unsatisfiable
//! twin, breadth-first search and Algorithm X as strategies, an experiment
//! comparing them on every instance, and its view.

use crate::{
    exact_cover::{self, Ontology},
    package::{call, written},
};
use axionomy::{OntologyPath, path};
use axionomy_engine::{Arm, CandidateSet, Effort, Engine, EngineError, Experiment, Strategy, View};
use axionomy_store::Checkpoints;

/// The script the searches call, and the view's, both importing Algorithm X.
const SCRIPT: &str = include_str!("../../scripts/exact-cover.rhai");
const VIEW: &str = include_str!("../../scripts/exact-cover-view.rhai");
const ALGORITHM_X: &str = include_str!("../../scripts/exact-cover-algorithm-x.rhai");

/// Writes exact cover's package into `engine`'s store, everything named
/// under `exact-cover`.
pub fn write(engine: &Engine) -> Result<(), EngineError> {
    let view = |function| Some(call("exact-cover/view", function));
    let call = |function| call("exact-cover", function);
    engine.keep_script(&path!("exact-cover/algorithm-x"), ALGORITHM_X)?;
    engine.keep_script(&path!("exact-cover"), SCRIPT)?;
    engine.keep_script(&path!("exact-cover/view"), VIEW)?;
    engine.keep_view(
        &OntologyPath::of::<Ontology>(),
        &View {
            title: "Exact cover".into(),
            labels: view("label"),
            formats: None,
            carry: None,
            scene: view("scene"),
            leaderboards: None,
        },
    )?;
    engine.keep_goal(
        &path!("exact-cover/covered"),
        &written(&exact_cover::goal()),
    )?;
    engine.keep_candidates(
        &path!("exact-cover/selections"),
        &CandidateSet::Script(call("candidates")),
    )?;
    let strategies = [
        (
            "exact-cover/breadth-first",
            Strategy::BreadthFirst {
                candidates: path!("exact-cover/selections"),
            },
        ),
        (
            "exact-cover/algorithm-x",
            Strategy::Plan {
                plan: call("algorithm_x"),
            },
        ),
    ];
    for (name, strategy) in &strategies {
        engine.keep_strategy(&name.parse().expect("a path"), strategy)?;
    }
    for (instance, initial, unsatisfiable) in [
        (
            "micro",
            exact_cover::initial(),
            exact_cover::unsatisfiable(),
        ),
        (
            "showcase",
            exact_cover::initial_showcase(),
            exact_cover::unsatisfiable_showcase(),
        ),
        (
            "stress",
            exact_cover::initial_stress(),
            exact_cover::unsatisfiable_stress(),
        ),
    ] {
        for (name, economy) in [
            (format!("exact-cover/{instance}"), initial),
            (
                format!("exact-cover/{instance}/unsatisfiable"),
                unsatisfiable,
            ),
        ] {
            let start = engine
                .store()
                .record::<Ontology>(economy, Checkpoints::Ends)
                .finish()?;
            engine.refer(&name.parse().expect("a path"), start)?;
            engine.keep_experiment(
                &name.parse().expect("a path"),
                &Experiment {
                    start: name.parse().expect("a path"),
                    goal: path!("exact-cover/covered"),
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
    }
    Ok(())
}
