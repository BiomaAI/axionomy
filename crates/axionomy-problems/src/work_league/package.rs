//! The Autonomous Work League as a package. Every worker holds its policy,
//! so each field the league plays is an economy of its own: an instance's
//! ref, `work-league/<instance>`, holds the mixed field, the catalog's
//! default, and `work-league/<instance>/<document>` the throughput and
//! sustainable fields, from which those documents' arms start. Contract
//! value, jobs completed and residual waste, summed over the workers, are
//! the objectives, and each instance has an experiment comparing its fields.
//! Its view ranks the workers on leaderboards, replayed from each step.
//!
//! Micro is the serial fixture: each turn runs until its job is done, which
//! takes a count of attempts the economy does not hold, and a steward's
//! recycling comes after the last job's completion, its goal. So its
//! strategies are plans, `work-league/serial/<document>`, made whole with
//! Nature's outcome of each attempt drawn by its event, `attempt/<job>/<n>`,
//! from the run's seed. Showcase and Stress are seasons on a shared clock,
//! played turn by turn by `work-league/<document>` until the season ends,
//! Nature settling each attempt by the same event. Whoever makes an attempt
//! and in whatever mode, every field meets the same luck at it.

use crate::{
    package::{call, written},
    work_league::{self, Ontology, Policy, Profile},
};
use axionomy::{OntologyPath, path};
use axionomy_engine::{
    Arm, Effort, Engine, EngineError, Experiment, GoalPath, Measure, Objective, ObjectivePath,
    Strategy, StrategyPath, View,
};
use axionomy_search::pareto::ObjectiveDirection::{Maximize, Minimize};
use axionomy_store::{Checkpoints, RefPath};

const SCRIPT: &str = include_str!("../../scripts/work-league.rhai");
const VIEW: &str = include_str!("../../scripts/work-league-view.rhai");

/// The seed the sample's Rust league runs with, from which Nature's events
/// are drawn.
const SEED: u64 = 17;

/// The most turns a season takes: at each of Stress's 79 clock readings,
/// an event for each of its 24 contracts, four services ending, four
/// decisions and the tick, and then the season's end.
const TURNS: u64 = 79 * (24 + 4 + 4 + 1) + 1;

/// The league's fields, each the policies its workers hold.
fn fields() -> [(&'static str, [Policy; 4]); 3] {
    [
        ("mixed-field", work_league::mixed_lineup()),
        ("throughput-field", work_league::throughput_lineup()),
        ("sustainable-field", work_league::sustainable_lineup()),
    ]
}

/// The strategy giving `document` at `profile`: Micro's plan or a season's
/// policy.
fn strategy(profile: Profile, document: &str) -> StrategyPath {
    match profile {
        Profile::Micro => format!("work-league/serial/{document}"),
        Profile::Showcase | Profile::Stress => format!("work-league/{document}"),
    }
    .parse()
    .expect("a path")
}

/// What a run at `profile` works toward: Micro's serial match completed,
/// or a season ended.
fn goal(profile: Profile) -> GoalPath {
    match profile {
        Profile::Micro => path!("work-league/serial/completed"),
        Profile::Showcase | Profile::Stress => path!("work-league/season-ended"),
    }
}

/// The most turns a run at `profile` plays: none for Micro's match, planned
/// whole; a season's, turn by turn.
fn turns(profile: Profile) -> u64 {
    match profile {
        Profile::Micro => 0,
        Profile::Showcase | Profile::Stress => TURNS,
    }
}

/// Writes the work league's package into `engine`'s store, everything
/// named under `work-league`.
pub fn write(engine: &Engine) -> Result<(), EngineError> {
    let view = |function| Some(call("work-league/view", function));
    let call = |function| call("work-league", function);
    engine.keep_script(&path!("work-league"), SCRIPT)?;
    engine.keep_script(&path!("work-league/view"), VIEW)?;
    engine.keep_view(
        &OntologyPath::of::<Ontology>(),
        &View {
            title: "Competitive multi-agent work allocation".into(),
            labels: view("label"),
            formats: view("format"),
            carry: None,
            scene: view("scene"),
            leaderboards: view("leaderboards"),
        },
    )?;
    let micro = work_league::league(Profile::Micro, work_league::mixed_lineup());
    let season = work_league::league(Profile::Showcase, work_league::mixed_lineup());
    engine.keep_goal(&goal(Profile::Micro), &written(micro.goal()))?;
    engine.keep_goal(&goal(Profile::Showcase), &written(season.goal()))?;
    // Named by the keys the samples' objectives were reported under.
    let mut objectives: Vec<ObjectivePath> = Vec::new();
    for (key, function, direction) in [
        ("value", "contract_value", Maximize),
        ("completed", "completed", Maximize),
        ("waste", "residual_waste", Minimize),
    ] {
        let objective: ObjectivePath = format!("work-league/{key}").parse().expect("a path");
        engine.keep_objective(
            &objective,
            &Objective {
                measure: Measure::State(call(function)),
                direction,
            },
        )?;
        objectives.push(objective);
    }
    // One script plays every field, which its workers hold: each document's
    // strategy is the same play under the document's name.
    for (document, _) in fields() {
        engine.keep_strategy(
            &strategy(Profile::Micro, document),
            &Strategy::Plan {
                plan: call("serial"),
            },
        )?;
        engine.keep_strategy(
            &strategy(Profile::Showcase, document),
            &Strategy::Policy {
                policy: call("season"),
            },
        )?;
    }
    for (instance, profile) in [
        ("micro", Profile::Micro),
        ("showcase", Profile::Showcase),
        ("stress", Profile::Stress),
    ] {
        let name = format!("work-league/{instance}");
        let mut arms = Vec::new();
        for (index, (document, lineup)) in fields().into_iter().enumerate() {
            // The default field is the instance's ref; the others start
            // from refs of their own.
            let reference: RefPath = if index == 0 {
                name.clone()
            } else {
                format!("{name}/{document}")
            }
            .parse()
            .expect("a path");
            let start = engine
                .store()
                .record::<Ontology>(
                    work_league::league(profile, lineup).initial().clone(),
                    Checkpoints::Ends,
                )
                .finish()?;
            engine.refer(&reference, start)?;
            arms.push(Arm {
                strategy: strategy(profile, document),
                start: (index > 0).then_some(reference),
            });
        }
        engine.keep_experiment(
            &name.parse().expect("a path"),
            &Experiment {
                start: name.parse().expect("a path"),
                goal: goal(profile),
                arms,

                objectives: objectives.clone(),
                effort: Effort {
                    seed: SEED,
                    budget: 10_000_000,
                    turns: turns(profile),
                },
                trials: 1,
            },
        )?;
    }
    Ok(())
}
