//! Stochastic logistics as a package. Micro is a `logistics` economy: one
//! vehicle with one package left to deliver, its direct and reliable route
//! policies, its planner, and completion, delivery and elapsed-time
//! objectives. The planner is a game as the study's receding-horizon run
//! plays it: Nature draws each leg's weather from the run's seed, the
//! vehicle takes the one move it has when it has one, and otherwise the
//! dispatcher replans by Monte Carlo tree search over its moves and
//! Nature's draws, simulating the reliable policy. Showcase and Stress are
//! `logistics/fleet` economies, an ontology of their own whose orders and
//! route tickets the study's seed sets, so their goal, policies and
//! objectives are named under `logistics/fleet`: the fastest-route,
//! reliable-route and hub-relay dispatch policies, each named for the
//! document it gives. Every instance has an experiment comparing its
//! strategies. Each ontology has a view of its own.

use crate::{
    logistics::{self, fleet},
    package::{call, written},
};
use axionomy::{Economy, OntologyPath, path};
use axionomy_engine::{
    Arm, CandidateSet, Chooser, Effort, Engine, EngineError, Experiment, GoalPath, Measure,
    Objective, ObjectivePath, Strategy, StrategyPath, View,
};
use axionomy_search::pareto::ObjectiveDirection::{self, Maximize, Minimize};
use axionomy_store::{Checkpoints, Recordable};

const SCRIPT: &str = include_str!("../../scripts/logistics.rhai");
const FLEET: &str = include_str!("../../scripts/fleet.rhai");
/// The views of Micro's vehicle and of the fleet.
const VIEW: &str = include_str!("../../scripts/logistics-view.rhai");
const FLEET_VIEW: &str = include_str!("../../scripts/fleet-view.rhai");

/// The seed of the study's default request: the weather Micro's runs meet,
/// and the fleet's route tickets.
const SEED: u64 = 17;
/// What the study's seed is XORed with to seed the planner's searches,
/// `LOGISTIC` in ASCII, so search randomness is apart from the weather.
const PLANNING_SALT: u64 = 0x4c4f_4749_5354_4943;
/// The planner's iterations per decision: the study's samples at its default
/// budget, 128 clamped to at most 16.
const ITERATIONS: u64 = 16;
/// The most exchanges a Micro run takes, and how deep the planner's
/// simulations look.
const HORIZON: u64 = logistics::ROLLOUT_HORIZON as u64;

/// Writes stochastic logistics' package into `engine`'s store, everything
/// named under `logistics`.
pub fn write(engine: &Engine) -> Result<(), EngineError> {
    let micro = |function: &str| call("logistics", function);
    engine.keep_script(&path!("logistics"), SCRIPT)?;
    engine.keep_script(&path!("logistics/fleet"), FLEET)?;
    for (ontology, script, source, title) in [
        (
            OntologyPath::of::<logistics::Ontology>(),
            "logistics/view",
            VIEW,
            "Stochastic logistics",
        ),
        (
            OntologyPath::of::<fleet::Ontology>(),
            "logistics/fleet/view",
            FLEET_VIEW,
            "Heterogeneous delivery fleet",
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
    engine.keep_goal(
        &path!("logistics/all-delivered"),
        &written(&logistics::goal()),
    )?;
    engine.keep_goal(&path!("logistics/fleet/closed"), &written(&fleet::goal()))?;
    engine.keep_candidates(
        &path!("logistics/moves"),
        &CandidateSet::Script(micro("moves")),
    )?;

    // Micro: the fixed routes turn by turn, and the planner.
    let route = |function: &str| Strategy::Policy {
        policy: micro(function),
    };
    let planner = Strategy::Game {
        nature: Some(micro("nature")),
        actor: micro("dispatcher"),
        players: vec![Chooser::Mcts {
            candidates: path!("logistics/moves"),
            payoffs: micro("payoff"),
            estimate: micro("estimated"),
            iterations: ITERATIONS,
            depth: HORIZON,
            chance: Some(micro("weather")),
            rollout: Some(micro("simulated")),
            salt: PLANNING_SALT,
        }],
    };
    let mut strategies = Vec::new();
    for (name, strategy) in [
        ("logistics/direct", route("direct")),
        ("logistics/reliable", route("reliable")),
        ("logistics/mcts", planner),
    ] {
        let name: StrategyPath = name.parse().expect("a path");
        engine.keep_strategy(&name, &strategy)?;
        strategies.push(name);
    }
    let objectives = measures(
        engine,
        "logistics",
        &[
            ("completed", "completed", Maximize),
            ("delivered", "delivered", Maximize),
            ("time", "elapsed", Minimize),
        ],
    )?;
    recorded(engine, "logistics/micro", logistics::initial_micro())?;
    experiment(
        engine,
        "logistics/micro",
        (path!("logistics/all-delivered"), HORIZON),
        &strategies,
        &objectives,
    )?;

    // Showcase and Stress: the fleet's dispatch policies, named for the
    // catalog's strategies whose documents they give.
    let mut dispatch = Vec::new();
    for (document, function) in [
        ("direct", "fastest"),
        ("reliable", "reliable"),
        ("mcts", "relay"),
    ] {
        let name: StrategyPath = format!("logistics/fleet/{document}")
            .parse()
            .expect("a path");
        engine.keep_strategy(
            &name,
            &Strategy::Policy {
                policy: call("logistics/fleet", function),
            },
        )?;
        dispatch.push(name);
    }
    let outcomes = measures(
        engine,
        "logistics/fleet",
        &[
            ("delivered", "delivered", Maximize),
            ("on-time", "on_time", Maximize),
            ("expired", "expired", Minimize),
            ("cash", "cash", Maximize),
            ("fuel", "spent_fuel", Minimize),
            ("time", "elapsed", Minimize),
        ],
    )?;
    for (instance, stress) in [("showcase", false), ("stress", true)] {
        let name = format!("logistics/{instance}");
        recorded(engine, &name, fleet::initial(stress, SEED))?;
        experiment(
            engine,
            &name,
            (path!("logistics/fleet/closed"), fleet::HORIZON as u64),
            &dispatch,
            &outcomes,
        )?;
    }
    Ok(())
}

/// Records `economy` as a run of its own ontology, named by the ref `name`.
fn recorded<O: Recordable>(
    engine: &Engine,
    name: &str,
    economy: Economy<O>,
) -> Result<(), EngineError> {
    let start = engine
        .store()
        .record::<O>(economy, Checkpoints::Ends)
        .finish()?;
    engine.refer(&name.parse().expect("a path"), start)
}

/// Keeps the experiment `name`, every strategy run once toward `goal` from
/// the ref of the same name with the study's seed, playing at most `turns`
/// turns, measured by `objectives`.
fn experiment(
    engine: &Engine,
    name: &str,
    (goal, turns): (GoalPath, u64),
    strategies: &[StrategyPath],
    objectives: &[ObjectivePath],
) -> Result<(), EngineError> {
    engine.keep_experiment(
        &name.parse().expect("a path"),
        &Experiment {
            start: name.parse().expect("a path"),
            goal,
            arms: strategies
                .iter()
                .map(|strategy| Arm {
                    strategy: strategy.clone(),
                    start: None,
                })
                .collect(),
            objectives: objectives.to_vec(),
            effort: Effort {
                seed: SEED,
                budget: 10_000_000,
                turns,
            },
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
