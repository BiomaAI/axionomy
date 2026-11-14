//! The uncertain rescue as a package. Micro is a `rescue` economy: Nature's
//! weighted prior, the scenario Micro plays, sense-then-follow and
//! direct-north policies, success, sensor and energy objectives, an
//! experiment on that scenario and a Monte Carlo experiment over the prior.
//! Showcase and Stress are `rescue/incident` economies, an ontology of their
//! own with its own goal, so its strategies and objectives are named under
//! `rescue/incident`: triage, rapid-north and stabilize-first policies, and
//! an experiment per instance. Each ontology has a view of its own.

use crate::{
    package::{call, written},
    rescue::{self, Location, incident},
};
use axionomy::{Economy, OntologyPath, path};
use axionomy_engine::{
    Arm, Effort, Engine, EngineError, Experiment, GoalPath, Measure, Objective, ObjectivePath,
    Strategy, StrategyPath, View,
};
use axionomy_search::pareto::ObjectiveDirection::{self, Maximize, Minimize};
use axionomy_store::{Checkpoints, Point, Recordable, RefPath};

const SCRIPT: &str = include_str!("../../scripts/rescue.rhai");
const INCIDENT: &str = include_str!("../../scripts/incident.rhai");
/// The views of the rescue and of the incidents.
const VIEW: &str = include_str!("../../scripts/rescue-view.rhai");
const INCIDENT_VIEW: &str = include_str!("../../scripts/incident-view.rhai");

/// The seed the sample's Rust runs play: the incidents' scenario, and the Monte Carlo's
/// samples.
const SEED: u64 = 17;
/// How many scenarios Micro's Monte Carlo samples.
const SAMPLES: u64 = 128;
/// The turns a Micro run may take: Nature's draw and the eight a Rust
/// rollout may take after it. None takes more than six.
const MICRO_TURNS: u64 = 1 + 8;

/// Writes the rescue's package into `engine`'s store, everything named
/// under `rescue`.
pub fn write(engine: &Engine) -> Result<(), EngineError> {
    engine.keep_script(&path!("rescue"), SCRIPT)?;
    engine.keep_script(&path!("rescue/incident"), INCIDENT)?;
    for (ontology, script, source, title) in [
        (
            OntologyPath::of::<rescue::Ontology>(),
            "rescue/view",
            VIEW,
            "Uncertain rescue",
        ),
        (
            OntologyPath::of::<incident::Ontology>(),
            "rescue/incident/view",
            INCIDENT_VIEW,
            "Multi-patient incident",
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
    engine.keep_goal(&path!("rescue/solved"), &written(&rescue::goal()))?;
    engine.keep_goal(
        &path!("rescue/incident/closed"),
        &written(&incident::goal()),
    )?;
    let micro = Family {
        goal: path!("rescue/solved"),
        turns: MICRO_TURNS,
        objectives: measures(
            engine,
            "rescue",
            &[
                ("success", "succeeded", Maximize),
                ("sensor", "sensed", Minimize),
                ("energy", "spent_energy", Minimize),
            ],
        )?,
        strategies: policies(
            engine,
            "rescue",
            &[
                ("observe-then-follow", "observe_then_follow"),
                ("direct-north", "direct_north"),
            ],
        )?,
    };
    let incidents = Family {
        goal: path!("rescue/incident/closed"),
        turns: incident::HORIZON as u64,
        objectives: measures(
            engine,
            "rescue/incident",
            &[
                ("saved", "saved", Maximize),
                ("vitality", "saved_vitality", Maximize),
                ("deaths", "deaths", Minimize),
                ("fuel", "spent_fuel", Minimize),
                ("medicine", "kits_used", Minimize),
                ("time", "elapsed", Minimize),
            ],
        )?,
        strategies: policies(
            engine,
            "rescue/incident",
            &[
                ("observe-then-follow", "triage"),
                ("direct-north", "rapid_north"),
                ("stabilize-first", "stabilize_first"),
            ],
        )?,
    };

    // Micro: the weighted model, and the scenario it plays, the
    // survivor south and the sensor's seed 1, a step on from it.
    let model = rescue::uniform_uncertain();
    let scenario = rescue::instantiate(&model, Location::South, 1)
        .expect("the scenario Micro plays is one Nature encodes");
    let start = recorded(engine, "rescue/micro", model)?;
    let played = engine.apply(start, &[written(&scenario)])?;
    engine.refer(&path!("rescue/micro/scenario"), played)?;
    micro.experiment(
        engine,
        "rescue/micro",
        Some(path!("rescue/micro/scenario")),
        1,
    )?;
    // Each trial draws its scenario from the prior by the event `scenario`
    // with its seed, as the Rust Monte Carlo draws it for the same sample.
    micro.experiment(engine, "rescue/micro/monte-carlo", None, SAMPLES)?;

    for (instance, stress) in [("showcase", false), ("stress", true)] {
        let name = format!("rescue/{instance}");
        recorded(engine, &name, incident::initial(stress, SEED))?;
        incidents.experiment(engine, &name, None, 1)?;
    }
    Ok(())
}

/// An ontology's goal, objectives and strategies, which its experiments
/// compare toward the goal, each within `turns`.
struct Family {
    goal: GoalPath,
    turns: u64,
    objectives: Vec<ObjectivePath>,
    strategies: Vec<StrategyPath>,
}

impl Family {
    /// Keeps the experiment `name` on the instance its first two segments
    /// name, every strategy from there or from `start`, over `trials`
    /// trials.
    fn experiment(
        &self,
        engine: &Engine,
        name: &str,
        start: Option<RefPath>,
        trials: u64,
    ) -> Result<(), EngineError> {
        let instance = name.split('/').take(2).collect::<Vec<_>>().join("/");
        engine.keep_experiment(
            &name.parse().expect("a path"),
            &Experiment {
                start: instance.parse().expect("a path"),
                goal: self.goal.clone(),
                arms: self
                    .strategies
                    .iter()
                    .map(|strategy| Arm {
                        strategy: strategy.clone(),
                        start: start.clone(),
                    })
                    .collect(),
                objectives: self.objectives.clone(),
                effort: Effort {
                    seed: SEED,
                    budget: 10_000_000,
                    turns: self.turns,
                },
                trials,
            },
        )
    }
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

/// Keeps `<script>/<key>` for each policy, the function of `script` that
/// plays a turn, until the run's goal or its turns.
fn policies(
    engine: &Engine,
    script: &str,
    policies: &[(&str, &str)],
) -> Result<Vec<StrategyPath>, EngineError> {
    policies
        .iter()
        .map(|(key, function)| {
            let strategy: StrategyPath = format!("{script}/{key}").parse().expect("a path");
            engine.keep_strategy(
                &strategy,
                &Strategy::Policy {
                    policy: call(script, function),
                },
            )?;
            Ok(strategy)
        })
        .collect()
}
