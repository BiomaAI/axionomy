//! The hidden-information mission as a package. Micro is a `mission`
//! economy: Nature's weighted prior over where the victim is, the sensor's
//! seed and the hazard, the scenario its study plays, a step on from it,
//! and success, time and medical-kit objectives. Its planner is a game as
//! the study's receding-horizon run plays it: Nature answers the team, and
//! the Scout chooses every public action by information-set tree search
//! over what it sees, each iteration in a world its observations since the
//! prior leave possible; the direct-north policy commits without looking.
//! An experiment on the study's scenario compares them, and a Monte Carlo
//! experiment over the prior samples the fixed policies the study's front
//! compares, share-and-coordinate and direct north.
//! Showcase and Stress are `mission/expedition` economies, an ontology of
//! its own whose hidden sites and hazard the study's seed sets, so their
//! goal, policies and objectives are named under `mission/expedition`: radio
//! coordination, north-first search and rendezvous, each named for the
//! document it gives. Every instance has an experiment comparing its
//! strategies. Each ontology has a view of its own.

use crate::{
    mission::{self, expedition},
    package::{call, written},
};
use axionomy::{Economy, OntologyPath, path};
use axionomy_engine::{
    Arm, CandidateSet, Chooser, Effort, Engine, EngineError, Experiment, GoalPath, Measure,
    Objective, ObjectivePath, Strategy, StrategyPath, View,
};
use axionomy_search::pareto::ObjectiveDirection::{self, Maximize, Minimize};
use axionomy_store::{Checkpoints, Point, Recordable, RefPath};

const SCRIPT: &str = include_str!("../../scripts/mission.rhai");
const EXPEDITION: &str = include_str!("../../scripts/expedition.rhai");
/// The views of the mission and of the expeditions.
const VIEW: &str = include_str!("../../scripts/mission-view.rhai");
const EXPEDITION_VIEW: &str = include_str!("../../scripts/expedition-view.rhai");

/// The seed of the study's default request: Micro's scenario and planning
/// seeds, and the expeditions' hidden sites and hazard.
const SEED: u64 = 17;
/// The study's samples at its default budget, 128 clamped to at most 16:
/// the planner's iterations per decision, and the scenarios its Monte Carlo
/// samples for each fixed policy.
const SAMPLES: u64 = 16;
/// The most exchanges a Micro run takes, Nature's draw among them, and how
/// deep the planner's simulations look. No mission comes near it: one scan,
/// one move and the time the mission holds end it within nine.
const HORIZON: u64 = mission::HORIZON as u64;

/// Writes the mission's package into `engine`'s store, everything named
/// under `mission`.
pub fn write(engine: &Engine) -> Result<(), EngineError> {
    let micro = |function: &str| call("mission", function);
    engine.keep_script(&path!("mission"), SCRIPT)?;
    engine.keep_script(&path!("mission/expedition"), EXPEDITION)?;
    for (ontology, script, source, title) in [
        (
            OntologyPath::of::<mission::Ontology>(),
            "mission/view",
            VIEW,
            "Hidden-information mission",
        ),
        (
            OntologyPath::of::<expedition::Ontology>(),
            "mission/expedition/view",
            EXPEDITION_VIEW,
            "Cooperative expedition",
        ),
    ] {
        let view = |function| Some(call(script, function));
        engine.keep_script(&script.parse().expect("a path"), source)?;
        engine.keep_view(
            &ontology,
            &View {
                title: title.into(),
                labels: view("label"),
                formats: view("format"),
                carry: None,
                scene: view("scene"),
                leaderboards: None,
            },
        )?;
    }
    engine.keep_goal(&path!("mission/solved"), &written(&mission::goal()))?;
    engine.keep_goal(
        &path!("mission/expedition/closed"),
        &written(&expedition::goal()),
    )?;
    engine.keep_candidates(
        &path!("mission/decisions"),
        &CandidateSet::Script(micro("decisions")),
    )?;

    // Micro: the planner, and both agents going north.
    let planner = Strategy::Game {
        nature: Some(micro("nature")),
        actor: micro("planner"),
        players: vec![Chooser::Ismcts {
            candidates: path!("mission/decisions"),
            observe: micro("scout_view"),
            chance: micro("draws"),
            payoffs: micro("payoff"),
            estimate: micro("unsolved"),
            iterations: SAMPLES,
            depth: HORIZON,
            rollout: Some(micro("informed")),
            // The study seeds its kth decision with its seed, plus k.
            salt: 0,
        }],
    };
    let policy = |function: &str| Strategy::Policy {
        policy: micro(function),
    };
    let kept = |name: &str, strategy: &Strategy| -> Result<StrategyPath, EngineError> {
        let name: StrategyPath = name.parse().expect("a path");
        engine.keep_strategy(&name, strategy)?;
        Ok(name)
    };
    let coordinated = kept("mission/coordinated", &planner)?;
    let direct_north = kept("mission/direct-north", &policy("direct_north"))?;
    // The fixed coordinating policy the study's front samples beside direct
    // north; no document plays it.
    let share_and_coordinate = kept(
        "mission/share-and-coordinate",
        &policy("share_and_coordinate"),
    )?;
    let objectives = measures(
        engine,
        "mission",
        &[
            ("success", "succeeded", Maximize),
            ("time", "elapsed", Minimize),
            ("medical", "kit_used", Minimize),
        ],
    )?;
    // The study plays the scenario its seed's systematic ticket draws from
    // the prior, sample `seed` of the sixteen.
    let model = mission::initial();
    let scenario = mission::systematic_scenario(&model, (SEED % 16) as usize);
    let start = recorded(engine, "mission/micro", model)?;
    let played = engine.apply(start, &[written(&scenario)])?;
    engine.refer(&path!("mission/micro/scenario"), played)?;
    experiment(
        engine,
        "mission/micro",
        (path!("mission/solved"), HORIZON),
        Arms {
            start: Some(path!("mission/micro/scenario")),
            strategies: &[coordinated, direct_north.clone()],
            trials: 1,
        },
        &objectives,
    )?;
    // The fixed policies' front, as the study samples it: each trial draws
    // its scenario from the prior with the ticket the study's Monte Carlo
    // draws for the same sample.
    experiment(
        engine,
        "mission/micro/monte-carlo",
        (path!("mission/solved"), HORIZON),
        Arms {
            start: None,
            strategies: &[share_and_coordinate, direct_north],
            trials: SAMPLES,
        },
        &objectives,
    )?;

    // Showcase and Stress: the expedition's policies, named for the
    // catalog's strategies whose documents they give.
    let mut policies = Vec::new();
    for (document, function) in [
        ("coordinated", "radio_coordination"),
        ("direct-north", "north_first"),
        ("rendezvous", "rendezvous"),
    ] {
        let name: StrategyPath = format!("mission/expedition/{document}")
            .parse()
            .expect("a path");
        engine.keep_strategy(
            &name,
            &Strategy::Policy {
                policy: call("mission/expedition", function),
            },
        )?;
        policies.push(name);
    }
    let outcomes = measures(
        engine,
        "mission/expedition",
        &[
            ("completed", "completed", Maximize),
            ("failed", "lost", Minimize),
            ("vitality", "vitality", Maximize),
            ("radio", "radio_spent", Minimize),
            ("energy", "energy_spent", Minimize),
            ("time", "elapsed", Minimize),
        ],
    )?;
    for (instance, stress) in [("showcase", false), ("stress", true)] {
        let name = format!("mission/{instance}");
        recorded(engine, &name, expedition::initial(stress, SEED))?;
        experiment(
            engine,
            &name,
            (
                path!("mission/expedition/closed"),
                expedition::HORIZON as u64,
            ),
            Arms {
                start: None,
                strategies: &policies,
                trials: 1,
            },
            &outcomes,
        )?;
    }
    Ok(())
}

/// An experiment's arms: its strategies, each run `trials` times from
/// `start`, or from the instance's ref without one.
struct Arms<'a> {
    start: Option<RefPath>,
    strategies: &'a [StrategyPath],
    trials: u64,
}

/// Records `economy` as a run of its own ontology, named by the ref `name`.
fn recorded<O: Recordable>(
    engine: &Engine,
    name: &str,
    economy: Economy<O>,
) -> Result<Point, EngineError> {
    let start = engine
        .store()
        .record::<O>(economy, Checkpoints::Ends)
        .finish()?;
    engine.refer(&name.parse().expect("a path"), start)?;
    Ok(start)
}

/// Keeps the experiment `name` on the instance its first two segments name,
/// toward `goal` within `turns`, with `arms`, the study's seed, and
/// `objectives`.
fn experiment(
    engine: &Engine,
    name: &str,
    (goal, turns): (GoalPath, u64),
    arms: Arms<'_>,
    objectives: &[ObjectivePath],
) -> Result<(), EngineError> {
    let instance = name.split('/').take(2).collect::<Vec<_>>().join("/");
    engine.keep_experiment(
        &name.parse().expect("a path"),
        &Experiment {
            start: instance.parse().expect("a path"),
            goal,
            arms: arms
                .strategies
                .iter()
                .map(|strategy| Arm {
                    strategy: strategy.clone(),
                    start: arms.start.clone(),
                })
                .collect(),
            objectives: objectives.to_vec(),
            effort: Effort {
                seed: SEED,
                budget: 10_000_000,
                turns,
            },
            trials: arms.trials,
        },
    )
}

/// Keeps `<script>/<key>` for each measure, the function of `script` that
/// measures a state, in its direction.
fn measures(
    engine: &Engine,
    script: &str,
    measures: &[(&str, &str, ObjectiveDirection)],
) -> Result<Vec<ObjectivePath>, EngineError> {
    measures
        .iter()
        .map(|(key, function, direction)| {
            let objective: ObjectivePath = format!("{script}/{key}").parse().expect("a path");
            engine.keep_objective(
                &objective,
                &Objective {
                    measure: Measure::State(call(script, function)),
                    direction: *direction,
                },
            )?;
            Ok(objective)
        })
        .collect()
}
