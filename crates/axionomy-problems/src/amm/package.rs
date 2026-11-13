//! The Living Market as a package: its instances, the script its plans and
//! policies call, the needs, utility, depth, unmet-need and default
//! objectives, a strategy for each of the market day and its no-whale and
//! thin-liquidity counterfactuals, and an experiment comparing them on each
//! instance.
//!
//! Micro is one market day without a clock: every actor's errands in a fixed
//! order, each quoted against the reserves the errands before it leave. What
//! its economy holds does not say which errand comes next, so its strategies
//! are plans, `amm/day/<document>`, that follow the reserves by the pool's
//! own law and end where the actors' goals are met. Showcase and Stress are
//! seasons, their sunlight seeded as the study seeds it, played turn by turn
//! by `amm/<document>` until the market closes. Its view carries each
//! actor's direct contribution to the price over the trace, and ranks them.

use crate::{
    amm::{self, Ontology},
    package::{call, written},
};
use axionomy::{OntologyPath, path};
use axionomy_engine::{
    Arm, Effort, Engine, EngineError, Experiment, GoalPath, Measure, Objective, ObjectivePath,
    Strategy, StrategyPath, View,
};
use axionomy_search::pareto::ObjectiveDirection::{Maximize, Minimize};
use axionomy_store::Checkpoints;

/// The script the plans and policies call, and the view's.
const SCRIPT: &str = include_str!("../../scripts/amm.rhai");
const VIEW: &str = include_str!("../../scripts/amm-view.rhai");

/// The seed of the study's default request, which seeds a season's
/// sunlight.
const SEED: u64 = 17;

/// The most turns a season takes: thin liquidity's opening withdrawal, and
/// on each of at most twelve days commissioning, every actor's six
/// decisions and the end of its trading, and the day's close.
const TURNS: u64 = 1 + 12 * (1 + amm::ACTORS.len() as u64 * 7 + 1);

/// Each document the study gives, and the script function that plays it,
/// a season's turn, or with `_plan` Micro's whole day.
const DOCUMENTS: [(&str, &str); 3] = [
    ("market-day", "market_day"),
    ("no-whale", "no_whale"),
    ("thin-liquidity", "thin_liquidity"),
];

/// Writes the Living Market's package into `engine`'s store, everything
/// named under `amm`.
pub fn write(engine: &Engine) -> Result<(), EngineError> {
    let view = |function| Some(call("amm/view", function));
    let call = |function: &str| call("amm", function);
    engine.keep_script(&path!("amm"), SCRIPT)?;
    engine.keep_script(&path!("amm/view"), VIEW)?;
    engine.keep_view(
        &OntologyPath::of::<Ontology>(),
        &View {
            title: "Closed endogenous market".into(),
            labels: view("label"),
            formats: view("format"),
            carry: view("carry"),
            scene: view("scene"),
            leaderboards: view("leaderboards"),
        },
    )?;
    engine.keep_goal(&path!("amm/day/met"), &written(&amm::goal()))?;
    let season = amm::initial_showcase_with_seed(SEED);
    engine.keep_goal(&path!("amm/closed"), &written(&amm::market_goal(&season)))?;
    // Named by the keys the study's documents report them under.
    let mut objectives: Vec<ObjectivePath> = Vec::new();
    for (key, function, direction) in [
        ("needs", "needs", Maximize),
        ("utility", "utility", Maximize),
        ("depth", "depth", Maximize),
        ("unmet", "unmet", Minimize),
        ("defaults", "defaulted", Minimize),
    ] {
        let objective: ObjectivePath = format!("amm/{key}").parse().expect("a path");
        engine.keep_objective(
            &objective,
            &Objective {
                measure: Measure::State(call(function)),
                direction,
            },
        )?;
        objectives.push(objective);
    }
    let mut day = Vec::new();
    let mut seasonal = Vec::new();
    for (document, function) in DOCUMENTS {
        let plan: StrategyPath = format!("amm/day/{document}").parse().expect("a path");
        engine.keep_strategy(
            &plan,
            &Strategy::Plan {
                plan: call(&format!("{function}_plan")),
            },
        )?;
        day.push(plan);
        let policy: StrategyPath = format!("amm/{document}").parse().expect("a path");
        engine.keep_strategy(
            &policy,
            &Strategy::Policy {
                policy: call(function),
            },
        )?;
        seasonal.push(policy);
    }
    // Micro's day ends where the actors' goals are met, its plans playing
    // no turns; a season, when the market closes, within its turns.
    let (met, closed): (GoalPath, GoalPath) = (path!("amm/day/met"), path!("amm/closed"));
    for (instance, initial, (goal, turns), strategies) in [
        ("micro", amm::initial(), (&met, 0), &day),
        ("showcase", season, (&closed, TURNS), &seasonal),
        (
            "stress",
            amm::initial_stress_with_seed(SEED),
            (&closed, TURNS),
            &seasonal,
        ),
    ] {
        let start = engine
            .store()
            .record::<Ontology>(initial, Checkpoints::Ends)
            .finish()?;
        let name = format!("amm/{instance}");
        engine.refer(&name.parse().expect("a path"), start)?;
        engine.keep_experiment(
            &name.parse().expect("a path"),
            &Experiment {
                start: name.parse().expect("a path"),
                goal: goal.clone(),
                arms: strategies
                    .iter()
                    .map(|strategy| Arm {
                        strategy: strategy.clone(),
                        start: None,
                    })
                    .collect(),
                objectives: objectives.clone(),
                effort: Effort {
                    seed: SEED,
                    budget: 10_000_000,
                    turns,
                },
                trials: 1,
            },
        )?;
    }
    Ok(())
}
