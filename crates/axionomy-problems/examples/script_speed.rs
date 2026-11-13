//! How long the samples' scripts take beside their Rust code, per call:
//! maze's heuristic and sokoban's candidates on their stress instances'
//! solution states, and a whole stress incident turn by turn.
//!
//! `cargo run --release -p axionomy-problems --example script_speed`

use axionomy::paths;
use axionomy_engine::{Engine, ScriptPath, ScriptState, rhai};
use axionomy_problems::{maze, rescue::incident, sokoban};
use axionomy_store::Store;
use serde::{Serialize, de::DeserializeOwned};
use std::time::{Duration, Instant};

fn written<T: DeserializeOwned>(value: &impl Serialize) -> T {
    postcard::from_bytes(&postcard::to_allocvec(value).unwrap()).unwrap()
}

fn engine(path: &str, source: &str) -> (Engine, ScriptPath) {
    let engine = Engine::new(Store::memory().unwrap()).unwrap();
    let script: ScriptPath = path.parse().unwrap();
    engine.keep_script(&script, source).unwrap();
    (engine, script)
}

/// The mean time of one call of `work`, over `calls` calls.
fn per_call(calls: usize, mut work: impl FnMut()) -> Duration {
    let start = Instant::now();
    for _ in 0..calls {
        work();
    }
    start.elapsed() / u32::try_from(calls).unwrap()
}

fn report(what: &str, rust: Duration, script: Duration) {
    println!(
        "{what:<34} rust {:>9.2?}  script {:>9.2?}  ×{:.0}",
        rust,
        script,
        script.as_secs_f64() / rust.as_secs_f64()
    );
}

fn states<O: axionomy::Ontology>(
    initial: &axionomy::Economy<O>,
    trace: &[axionomy::Exchange<O>],
) -> Vec<axionomy::Economy<O>> {
    let mut states = vec![initial.clone()];
    for exchange in trace {
        let mut next = states.last().unwrap().clone();
        next.apply(exchange.clone()).unwrap();
        states.push(next);
    }
    states
}

fn main() {
    let rounds = 200;

    let (engine_, script) = engine("maze", include_str!("../scripts/maze.rhai"));
    let initial = maze::initial_stress();
    let solution = maze::solve_astar(&initial).unwrap();
    let typed = states(&initial, solution.trace().exchanges());
    let scripted: Vec<ScriptState> = typed
        .iter()
        .map(|state| ScriptState::new(written(state)))
        .collect();
    let rust = per_call(rounds, || {
        typed.iter().for_each(|state| {
            std::hint::black_box(maze::heuristic(state));
        })
    }) / u32::try_from(typed.len()).unwrap();
    let script_time = per_call(rounds, || {
        scripted.iter().for_each(|state| {
            let _: i64 = engine_
                .call(&script, "heuristic", (state.clone(),))
                .unwrap();
        })
    }) / u32::try_from(typed.len()).unwrap();
    report("maze heuristic (stress)", rust, script_time);

    let (engine_, script) = engine("sokoban", include_str!("../scripts/sokoban.rhai"));
    let initial = sokoban::initial_stress();
    let solution = sokoban::solve(&initial).unwrap();
    let typed = states(&initial, solution.trace().exchanges());
    let scripted: Vec<ScriptState> = typed
        .iter()
        .map(|state| ScriptState::new(written(state)))
        .collect();
    let rounds = 20;
    let rust = per_call(rounds, || {
        typed.iter().for_each(|state| {
            std::hint::black_box(sokoban::candidates(state));
        })
    }) / u32::try_from(typed.len()).unwrap();
    let script_time = per_call(rounds, || {
        scripted.iter().for_each(|state| {
            let _: rhai::Array = engine_
                .call(&script, "candidates", (state.clone(),))
                .unwrap();
        })
    }) / u32::try_from(typed.len()).unwrap();
    report("sokoban candidates (stress)", rust, script_time);

    let (engine_, script) = engine("incident", include_str!("../scripts/incident.rhai"));
    let initial = incident::initial(true, 0);
    let mut world = initial.clone();
    let trace = incident::run(&mut world, incident::Policy::Triage)
        .trace()
        .unwrap()
        .exchanges()
        .to_vec();
    let typed = states(&initial, &trace);
    let scripted: Vec<ScriptState> = typed
        .iter()
        .map(|state| ScriptState::new(written(state)))
        .collect();
    let rust = per_call(rounds, || {
        typed.iter().for_each(|state| {
            let decided = incident::nature_response(state).or_else(|| {
                incident::decision(&incident::responder_view(state), incident::Policy::Triage)
            });
            std::hint::black_box(decided);
        })
    }) / u32::try_from(typed.len()).unwrap();
    let script_time = per_call(rounds, || {
        scripted.iter().for_each(|state| {
            let _: paths::Exchange = engine_
                .call(&script, "turn", (state.clone(), "triage".to_owned()))
                .unwrap();
        })
    }) / u32::try_from(typed.len()).unwrap();
    report("incident turn (stress, triage)", rust, script_time);
}
