//! Perishable inventory as a package. Micro is a `perishables` economy:
//! the outage played turn by turn until it is resolved, each timed
//! condition spoiling its cohort once its moment comes; and the exact front
//! of storage plans, ranked for usable stock or for cooling energy, toward
//! a sealed plan, a question of its own, so an experiment of its own,
//! `perishables/micro/storage-plans`; with claims, usable claims and
//! cooling energy as objectives. Showcase and Stress are
//! `perishables/cold-chain` economies, an ontology of their own with its own
//! goal, so its strategies and objectives are named under
//! `perishables/cold-chain`: the four daily policies on the study's outage
//! schedule, each a plan for every day, with food saved, delivered, revenue,
//! energy, fuel and missed obligations as objectives. Each instance has an
//! experiment comparing its strategies, and each ontology a view of its own.

use crate::{
    package::{call, written},
    perishables::{self, cold_chain},
};
use axionomy::{Economy, OntologyPath, path};
use axionomy_engine::{
    Arm, CandidateSet, Engine, EngineError, Experiment, GoalPath, Measure, Objective,
    ObjectivePath, Strategy, StrategyPath, View,
};
use axionomy_search::pareto::ObjectiveDirection::{self, Maximize, Minimize};
use axionomy_store::{Checkpoints, Recordable};

const SCRIPT: &str = include_str!("../../scripts/perishables.rhai");
const COLD_CHAIN: &str = include_str!("../../scripts/cold-chain.rhai");
/// The views of Micro's inventory and of the cold chains.
const VIEW: &str = include_str!("../../scripts/perishables-view.rhai");
const COLD_CHAIN_VIEW: &str = include_str!("../../scripts/cold-chain-view.rhai");

/// The seed of the study's default request, which picks the day the cold
/// chain's outage begins.
const SEED: u64 = 17;

/// The turns Micro's outage may take: the transfer, the clock's three
/// advances, each timed condition's spoilage, the power loss and the
/// finish.
const OUTAGE_TURNS: u64 = 1 + 3 + 3 + 1 + 1;

/// Writes the perishables' package into `engine`'s store, everything named
/// under `perishables`.
pub fn write(engine: &Engine) -> Result<(), EngineError> {
    engine.keep_script(&path!("perishables"), SCRIPT)?;
    engine.keep_script(&path!("perishables/cold-chain"), COLD_CHAIN)?;
    for (ontology, script, source, title) in [
        (
            OntologyPath::of::<perishables::Ontology>(),
            "perishables/view",
            VIEW,
            "Perishable inventory",
        ),
        (
            OntologyPath::of::<cold_chain::Ontology>(),
            "perishables/cold-chain/view",
            COLD_CHAIN_VIEW,
            "Cold-chain operations",
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
    micro(engine)?;
    cold_chain(engine)
}

/// Micro: the outage policy and the storage plans' Pareto picks.
fn micro(engine: &Engine) -> Result<(), EngineError> {
    let call = |function| call("perishables", function);
    engine.keep_goal(&path!("perishables/solved"), &written(&perishables::goal()))?;
    engine.keep_goal(
        &path!("perishables/plan-sealed"),
        &written(&perishables::storage_plan_goal()),
    )?;
    engine.keep_candidates(
        &path!("perishables/storage-plans"),
        &CandidateSet::Script(call("storage_plans")),
    )?;
    // Named by the keys the study's documents report them under: the outage
    // reports claims and usable claims; the storage plans usable stock and
    // cooling energy, where usable claims are the refrigerated ones a sealed
    // plan keeps.
    let objectives = measures(
        engine,
        "perishables",
        &[
            ("claims", "claims", Maximize),
            ("usable", "usable_claims", Maximize),
            ("energy", "cooling_energy", Minimize),
        ],
    )?;
    let front = objectives[1..].to_vec();
    let pareto = |by: [&ObjectivePath; 2]| Strategy::Pareto {
        candidates: path!("perishables/storage-plans"),
        objectives: front.clone(),
        by: by.map(ObjectivePath::clone).to_vec(),
    };
    let kept = |name: &str, strategy: &Strategy| -> Result<StrategyPath, EngineError> {
        let path: StrategyPath = name.parse().expect("a path");
        engine.keep_strategy(&path, strategy)?;
        Ok(path)
    };
    let outage = kept(
        "perishables/outage",
        &Strategy::Policy {
            policy: call("outage"),
            horizon: OUTAGE_TURNS,
        },
    )?;
    let plans = vec![
        kept(
            "perishables/pareto-inventory",
            &pareto([&objectives[1], &objectives[2]]),
        )?,
        kept(
            "perishables/pareto-energy",
            &pareto([&objectives[2], &objectives[1]]),
        )?,
    ];
    // Micro's study holds seventy claims in the warehouse and thirty in the
    // fridge, and moves ten at a time.
    recorded(engine, "micro", perishables::initial_with_inventory(70, 30))?;
    // The outage plays until it is resolved; the storage plans are searched
    // for a sealed one.
    experiment(
        engine,
        "micro",
        path!("perishables/solved"),
        vec![outage],
        objectives.clone(),
    )?;
    experiment(
        engine,
        "micro/storage-plans",
        path!("perishables/plan-sealed"),
        plans,
        objectives,
    )
}

/// Showcase and Stress: the cold chain's daily policies, each planned whole.
fn cold_chain(engine: &Engine) -> Result<(), EngineError> {
    let call = |function| call("perishables/cold-chain", function);
    engine.keep_goal(
        &path!("perishables/cold-chain/closed"),
        &written(&cold_chain::goal()),
    )?;
    let objectives = measures(
        engine,
        "perishables/cold-chain",
        &[
            ("saved", "saved", Maximize),
            ("delivered", "delivered_food", Maximize),
            ("revenue", "revenue", Maximize),
            ("energy", "energy_used", Minimize),
            ("fuel", "fuel_used", Minimize),
            ("missed", "missed_demand", Minimize),
        ],
    )?;
    let mut arms = Vec::new();
    for (document, function) in [
        ("outage", "protect_cold"),
        ("pareto-inventory", "process_first"),
        ("pareto-energy", "conserve_fuel"),
        ("storage-buffer", "rebalance_cold"),
    ] {
        let path: StrategyPath = format!("perishables/cold-chain/{document}")
            .parse()
            .expect("a path");
        engine.keep_strategy(
            &path,
            &Strategy::Plan {
                plan: call(function),
            },
        )?;
        arms.push(path);
    }
    for (instance, stress) in [("showcase", false), ("stress", true)] {
        recorded(engine, instance, cold_chain::initial(stress, SEED))?;
        experiment(
            engine,
            instance,
            path!("perishables/cold-chain/closed"),
            arms.clone(),
            objectives.clone(),
        )?;
    }
    Ok(())
}

/// Records `start` as a run of its own ontology under the ref
/// `perishables/<instance>`.
fn recorded<O: Recordable>(
    engine: &Engine,
    instance: &str,
    start: Economy<O>,
) -> Result<(), EngineError> {
    let start = engine
        .store()
        .record::<O>(start, Checkpoints::Ends)
        .finish()?;
    engine.refer(
        &format!("perishables/{instance}").parse().expect("a path"),
        start,
    )
}

/// Keeps the experiment `perishables/<name>` comparing `strategies` toward
/// `goal` from the instance its first segment names.
fn experiment(
    engine: &Engine,
    name: &str,
    goal: GoalPath,
    strategies: Vec<StrategyPath>,
    objectives: Vec<ObjectivePath>,
) -> Result<(), EngineError> {
    let instance = name.split('/').next().unwrap_or(name);
    engine.keep_experiment(
        &format!("perishables/{name}").parse().expect("a path"),
        &Experiment {
            start: format!("perishables/{instance}").parse().expect("a path"),
            goal,
            arms: strategies
                .into_iter()
                .map(|strategy| Arm {
                    strategy,
                    start: None,
                })
                .collect(),
            objectives,
            seed: SEED,
            budget: 10_000_000,
            trials: 1,
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
