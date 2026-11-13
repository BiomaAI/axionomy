//! The lending market as a package: its instances and each one's fresh
//! market, the scripts its searches, scenario, attack and view call, the
//! interest, collateral and cash objectives, its strategies, and for each
//! instance its goal and an experiment comparing the strategies toward it.
//!
//! The goal is the instance's: the bill paid and all of its collateral
//! free again. The strategies are the same at every size, since each
//! experiment names the goal its arms work toward: least interest,
//! Dijkstra's search over each closed period's interest as the study runs
//! it; fewest steps, breadth-first search; the over-borrowed scenario, a
//! policy played turn by turn; and the first-depositor attack, a plan run
//! from the instance's fresh market.

use crate::{
    lending::{self, Config, Ontology},
    package::{call, written},
};
use axionomy::{OntologyPath, path};
use axionomy_engine::{
    Arm, CandidateSet, Effort, Engine, EngineError, Experiment, GoalPath, Measure, Objective,
    ObjectivePath, ScriptCall, Strategy, StrategyPath, View,
};
use axionomy_search::pareto::ObjectiveDirection;
use axionomy_store::{Checkpoints, RefPath};

/// The script the searches, scenario and attack call, and the view's.
const SCRIPT: &str = include_str!("../../scripts/lending.rhai");
const VIEW: &str = include_str!("../../scripts/lending-view.rhai");

/// The turns the over-borrowed scenario may take: it opens the loan in
/// three, then the clock closes at most twelve periods, the price drops
/// once and the keeper liquidates once.
const HORIZON: u64 = 64;

/// The strategy whose arm starts from the instance's fresh market.
const ATTACK: &str = "first-depositor-attack";

/// Writes the lending market's package into `engine`'s store, everything
/// named under `lending`.
pub fn write(engine: &Engine) -> Result<(), EngineError> {
    let call = |function| call("lending", function);
    engine.keep_script(&path!("lending"), SCRIPT)?;
    engine.keep_script(&path!("lending/view"), VIEW)?;
    let view = |function: &str| {
        Some(ScriptCall::new(
            path!("lending/view"),
            function.parse().expect("a function name"),
        ))
    };
    engine.keep_view(
        &OntologyPath::of::<Ontology>(),
        &View {
            title: "Lending market".into(),
            labels: view("label"),
            formats: view("format"),
            carry: None,
            scene: view("scene"),
            leaderboards: None,
        },
    )?;
    engine.keep_candidates(
        &path!("lending/moves"),
        &CandidateSet::Script(call("candidates")),
    )?;
    // Named by the keys the study's documents report them under.
    let objectives: Vec<ObjectivePath> = vec![
        path!("lending/interest"),
        path!("lending/collateral"),
        path!("lending/cash"),
    ];
    for (objective, measure, direction) in [
        (
            &objectives[0],
            Measure::Steps(call("interest")),
            ObjectiveDirection::Minimize,
        ),
        (
            &objectives[1],
            Measure::State(call("collateral_kept")),
            ObjectiveDirection::Maximize,
        ),
        (
            &objectives[2],
            Measure::State(call("borrower_cash")),
            ObjectiveDirection::Maximize,
        ),
    ] {
        engine.keep_objective(objective, &Objective { measure, direction })?;
    }
    let strategies = strategies();
    for (strategy, definition) in &strategies {
        engine.keep_strategy(&strategy_path(strategy), definition)?;
    }
    for (instance, config) in [
        ("micro", Config::micro()),
        ("showcase", Config::showcase()),
        ("stress", Config::stress()),
    ] {
        let name = format!("lending/{instance}");
        let goal: GoalPath = format!("{name}/repaid").parse().expect("a path");
        engine.keep_goal(&goal, &written(&lending::goal(&config)))?;
        let genesis = format!("{name}/genesis");
        for (reference, economy) in [
            (&name, lending::world(&config)),
            (&genesis, lending::genesis(&config)),
        ] {
            let start = engine
                .store()
                .record::<Ontology>(economy, Checkpoints::Ends)
                .finish()?;
            engine.refer(&reference.parse().expect("a path"), start)?;
        }
        let genesis: RefPath = genesis.parse().expect("a path");
        engine.keep_experiment(
            &name.parse().expect("a path"),
            &Experiment {
                start: name.parse().expect("a path"),
                goal,
                arms: strategies
                    .iter()
                    .map(|(strategy, _)| Arm {
                        strategy: strategy_path(strategy),
                        // The attack needs a market no one has deposited in.
                        start: (*strategy == ATTACK).then(|| genesis.clone()),
                    })
                    .collect(),
                objectives: objectives.clone(),
                effort: Effort {
                    seed: 17,
                    budget: 10_000_000,
                    turns: HORIZON,
                },
                trials: 1,
            },
        )?;
    }
    Ok(())
}

/// The strategy named `name`, `lending/<name>`.
fn strategy_path(name: &str) -> StrategyPath {
    format!("lending/{name}").parse().expect("a path")
}

/// Every strategy, by name.
fn strategies() -> [(&'static str, Strategy); 4] {
    let call = |function| call("lending", function);
    [
        (
            "least-interest",
            Strategy::Cheapest {
                candidates: path!("lending/moves"),
                cost: call("interest"),
                heuristic: None,
            },
        ),
        (
            "fewest-steps",
            Strategy::BreadthFirst {
                candidates: path!("lending/moves"),
            },
        ),
        (
            "over-borrowed",
            Strategy::Policy {
                policy: call("over_borrowed"),
            },
        ),
        (
            ATTACK,
            Strategy::Plan {
                plan: call("first_depositor_attack"),
            },
        ),
    ]
}
