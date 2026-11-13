//! The samples solved by the engine's strategies over their scripts: maze's
//! A* and Dijkstra find the cheapest routes the Rust solvers find, sokoban's
//! breadth-first search the shortest solutions, and incident's policies run
//! the incidents the Rust code runs.

use axionomy::{Ontology, Trace, paths};
use axionomy_engine::{CandidateSet, Effort, Engine, Outcome, ScriptCall, Strategy, StrategyPath};
use axionomy_problems::{maze, rescue::incident, sokoban};
use axionomy_store::Store;
use serde::{Serialize, de::DeserializeOwned};
use std::ops::ControlFlow;

fn written<T: DeserializeOwned>(value: &impl Serialize) -> T {
    postcard::from_bytes(&postcard::to_allocvec(value).unwrap()).unwrap()
}

fn path<K>(text: &str) -> axionomy::path::Path<K> {
    text.parse().unwrap()
}

fn call(script: &str, function: &str) -> ScriptCall {
    ScriptCall::new(path(script), function.parse().unwrap())
}

/// The exchanges a run recorded from `start` to `end`.
fn trace(engine: &Engine, end: axionomy_store::Point, from: u64) -> Vec<paths::Exchange> {
    let (_, exchanges) = engine.history(end).unwrap();
    exchanges[usize::try_from(from).unwrap()..].to_vec()
}

fn typed_trace<O: Ontology>(trace: &Trace<O>) -> Vec<paths::Exchange>
where
    O::Rate: Serialize,
    O::Role: Serialize,
    O::Account: Serialize,
{
    trace.exchanges().iter().map(written).collect()
}

/// What `strategy` reaches from `initial` toward the goal at `goal`.
fn solve(
    engine: &Engine,
    initial: paths::Economy,
    strategy: &StrategyPath,
    goal: &str,
) -> axionomy_engine::Ran {
    let start = engine.author(initial).unwrap();
    let strategy = engine.strategy(strategy).unwrap();
    let effort = Effort {
        seed: 17,
        budget: 1_000_000,
    };
    engine
        .run_strategy(start, &strategy, &path(goal), effort, &mut |_| {
            ControlFlow::Continue(())
        })
        .unwrap()
}

#[test]
fn mazes_cheapest_routes_are_the_rust_solvers() {
    let engine = Engine::new(Store::memory().unwrap()).unwrap();
    engine
        .keep_script(&path("maze"), include_str!("../scripts/maze.rhai"))
        .unwrap();
    engine
        .keep_goal(&path("maze/solved"), &written(&maze::goal()))
        .unwrap();
    engine
        .keep_candidates(
            &path("maze/moves"),
            &CandidateSet::Script(call("maze", "candidates")),
        )
        .unwrap();
    for (name, heuristic) in [
        ("maze/a-star", Some(call("maze", "heuristic"))),
        ("maze/dijkstra", None),
    ] {
        engine
            .keep_strategy(
                &path(name),
                &Strategy::Cheapest {
                    candidates: path("maze/moves"),
                    cost: call("maze", "energy"),
                    heuristic,
                },
            )
            .unwrap();
    }
    for initial in [
        maze::initial(),
        maze::initial_showcase(),
        maze::initial_stress(),
    ] {
        let expected = maze::solve_astar(&initial).unwrap();
        for strategy in ["maze/a-star", "maze/dijkstra"] {
            let ran = solve(&engine, written(&initial), &path(strategy), "maze/solved");
            assert!(matches!(ran.outcome, Outcome::Reached), "{strategy}");
            assert_eq!(ran.cost, Some(expected.cost()), "{strategy}");
            assert!(engine.holds(ran.end, &path("maze/solved")).unwrap());
        }
        let ran = solve(
            &engine,
            written(&initial),
            &path("maze/a-star"),
            "maze/solved",
        );
        assert_eq!(trace(&engine, ran.end, 0), typed_trace(expected.trace()));
    }
}

#[test]
fn sokobans_breadth_first_solutions_are_the_shortest() {
    let engine = Engine::new(Store::memory().unwrap()).unwrap();
    engine
        .keep_script(&path("sokoban"), include_str!("../scripts/sokoban.rhai"))
        .unwrap();
    engine
        .keep_goal(&path("sokoban/solved"), &written(&sokoban::goal()))
        .unwrap();
    engine
        .keep_candidates(
            &path("sokoban/moves"),
            &CandidateSet::Script(call("sokoban", "candidates")),
        )
        .unwrap();
    engine
        .keep_strategy(
            &path("sokoban/breadth-first"),
            &Strategy::BreadthFirst {
                candidates: path("sokoban/moves"),
            },
        )
        .unwrap();
    for initial in [sokoban::initial(), sokoban::initial_showcase()] {
        let expected = sokoban::solve(&initial).unwrap();
        let ran = solve(
            &engine,
            written(&initial),
            &path("sokoban/breadth-first"),
            "sokoban/solved",
        );
        assert!(matches!(ran.outcome, Outcome::Reached));
        assert_eq!(
            trace(&engine, ran.end, 0).len(),
            expected.trace().exchanges().len()
        );
        assert!(engine.holds(ran.end, &path("sokoban/solved")).unwrap());
    }
}

#[test]
fn incidents_policies_run_the_rust_incidents() {
    let engine = Engine::new(Store::memory().unwrap()).unwrap();
    engine
        .keep_script(&path("incident"), include_str!("../scripts/incident.rhai"))
        .unwrap();
    engine
        .keep_goal(&path("incident/closed"), &written(&incident::goal()))
        .unwrap();
    let policies = [
        (incident::Policy::Triage, "triage"),
        (incident::Policy::RapidNorth, "rapid_north"),
        (incident::Policy::StabilizeFirst, "stabilize_first"),
    ];
    for (_, function) in policies {
        engine
            .keep_strategy(
                &path(&format!("incident/{}", function.replace('_', "-"))),
                &Strategy::Policy {
                    policy: call("incident", function),
                    horizon: incident::HORIZON as u64,
                },
            )
            .unwrap();
    }
    for seed in 0..3 {
        let initial = incident::initial(false, seed);
        for (policy, function) in policies {
            let mut world = initial.clone();
            let expected = incident::run(&mut world, policy);
            let ran = solve(
                &engine,
                written(&initial),
                &path(&format!("incident/{}", function.replace('_', "-"))),
                "incident/closed",
            );
            assert!(matches!(ran.outcome, Outcome::Reached));
            assert_eq!(
                trace(&engine, ran.end, 0),
                typed_trace(expected.trace().unwrap())
            );
        }
    }
}
