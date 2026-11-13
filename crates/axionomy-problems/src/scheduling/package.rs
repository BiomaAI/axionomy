//! Job-shop scheduling as a package: its instances and each one's horizon too
//! short for the work, the script its searches call, its goal, each job's
//! completion and the makespan as objectives, its strategies, and an
//! experiment comparing them on every instance.

use crate::{
    package::{call, written},
    scheduling::{self, Ontology},
};
use axionomy::path;
use axionomy_engine::{
    Arm, CandidateSet, Engine, EngineError, Experiment, Measure, Objective, ObjectivePath, Strategy,
};
use axionomy_search::pareto::ObjectiveDirection;
use axionomy_store::Checkpoints;

const SCRIPT: &str = include_str!("../../scripts/scheduling.rhai");

/// Writes job-shop scheduling's package into `engine`'s store, everything
/// named under `scheduling`.
pub fn write(engine: &Engine) -> Result<(), EngineError> {
    let call = |function| call("scheduling", function);
    engine.keep_script(&path!("scheduling"), SCRIPT)?;
    engine.keep_goal(&path!("scheduling/solved"), &written(&scheduling::goal()))?;
    engine.keep_candidates(
        &path!("scheduling/events"),
        &CandidateSet::Script(call("candidates")),
    )?;
    let objectives: Vec<ObjectivePath> = vec![
        path!("scheduling/job-one"),
        path!("scheduling/job-two"),
        path!("scheduling/makespan"),
    ];
    for (objective, function) in objectives.iter().zip(["job_one", "job_two", "makespan"]) {
        engine.keep_objective(
            objective,
            &Objective {
                measure: Measure::State(call(function)),
                direction: ObjectiveDirection::Minimize,
            },
        )?;
    }
    // The front spans when each job completes.
    let (job_one, job_two) = (&objectives[0], &objectives[1]);
    let pareto = |by: [&ObjectivePath; 2]| Strategy::Pareto {
        candidates: path!("scheduling/events"),
        goal: path!("scheduling/solved"),
        objectives: vec![job_one.clone(), job_two.clone()],
        by: by.into_iter().cloned().collect(),
    };
    let strategies = [
        (
            "scheduling/best-first",
            Strategy::BestFirst {
                candidates: path!("scheduling/events"),
                goal: path!("scheduling/solved"),
                cost: call("makespan"),
                heuristic: None,
            },
        ),
        // The clock never runs back, so the makespan a schedule has reached
        // bounds every schedule that continues it; a planned schedule records
        // its makespan only when it finishes.
        (
            "scheduling/bounded-optimizer",
            Strategy::BranchAndBound {
                candidates: path!("scheduling/events"),
                goal: path!("scheduling/solved"),
                cost: call("makespan"),
                bound: call("makespan"),
            },
        ),
        ("scheduling/pareto-job-one", pareto([job_one, job_two])),
        ("scheduling/pareto-job-two", pareto([job_two, job_one])),
    ];
    for (name, strategy) in &strategies {
        engine.keep_strategy(&name.parse().expect("a path"), strategy)?;
    }
    for (instance, initial, impossible) in [
        ("micro", scheduling::initial(), scheduling::impossible()),
        (
            "showcase",
            scheduling::initial_showcase(),
            scheduling::impossible_showcase(),
        ),
        (
            "stress",
            scheduling::initial_stress(),
            scheduling::impossible_showcase(),
        ),
    ] {
        for (name, economy) in [
            (format!("scheduling/{instance}"), initial),
            (
                format!("scheduling/{instance}/infeasible-horizon"),
                impossible,
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
    }
    Ok(())
}
