//! The samples' own logic, written as Rhai scripts (D-049), proposes exactly
//! what their Rust code does: maze's A* heuristic and sokoban's candidates on
//! every state of each instance's solution and the states one step from it.

use axionomy::{Economy, Ontology, paths};
use axionomy_engine::{Engine, ScriptPath, ScriptState, exchanges_of, rhai};
use axionomy_problems::{maze, rescue::incident, sokoban};
use axionomy_store::Store;
use serde::{Serialize, de::DeserializeOwned};

/// `value` written in binary and read back as `T`: a typed economy or
/// exchange read as paths, which D-044 makes the same bytes.
fn written<T: DeserializeOwned>(value: &impl Serialize) -> T {
    postcard::from_bytes(&postcard::to_allocvec(value).unwrap()).unwrap()
}

fn engine(path: &str, source: &str) -> (Engine, ScriptPath) {
    let engine = Engine::new(Store::memory().unwrap()).unwrap();
    let script: ScriptPath = path.parse().unwrap();
    engine.keep_script(&script, source).unwrap();
    (engine, script)
}

/// Every state of the trace from `initial`, and every state one candidate
/// away from one of them.
fn states<O: Ontology>(
    initial: &Economy<O>,
    trace: &[axionomy::Exchange<O>],
    candidates: impl Fn(&Economy<O>) -> Vec<axionomy::Exchange<O>>,
) -> Vec<Economy<O>> {
    let mut on_trace = vec![initial.clone()];
    for exchange in trace {
        let mut next = on_trace.last().unwrap().clone();
        next.apply(exchange.clone()).unwrap();
        on_trace.push(next);
    }
    let mut states = on_trace.clone();
    for state in &on_trace {
        for exchange in candidates(state) {
            let mut next = state.clone();
            next.apply(exchange).unwrap();
            states.push(next);
        }
    }
    states
}

#[test]
fn the_maze_heuristic_as_a_script_is_the_rust_heuristic() {
    let (engine, script) = engine("maze", include_str!("../scripts/maze.rhai"));
    for initial in [
        maze::initial(),
        maze::initial_showcase(),
        maze::initial_stress(),
    ] {
        let solution = maze::solve_astar(&initial).expect("every maze instance is solvable");
        for state in states(&initial, solution.trace().exchanges(), maze::candidates) {
            let scripted: i64 = engine
                .call(&script, "heuristic", (ScriptState::new(written(&state)),))
                .unwrap();
            assert_eq!(scripted as u64, maze::heuristic(&state));
        }
    }
}

#[test]
fn sokobans_candidates_as_a_script_are_the_rust_candidates() {
    let (engine, script) = engine("sokoban", include_str!("../scripts/sokoban.rhai"));
    for initial in [
        sokoban::initial(),
        sokoban::initial_showcase(),
        sokoban::initial_stress(),
    ] {
        let solution = sokoban::solve(&initial).expect("every sokoban instance is solvable");
        for state in states(&initial, solution.trace().exchanges(), sokoban::candidates) {
            let scripted: rhai::Array = engine
                .call(&script, "candidates", (ScriptState::new(written(&state)),))
                .unwrap();
            let expected: Vec<paths::Exchange> =
                sokoban::candidates(&state).iter().map(written).collect();
            assert_eq!(exchanges_of(scripted).unwrap(), expected);
        }
    }
}

/// What a script returned where it may return nothing: `()` or an exchange.
fn proposal(returned: rhai::Dynamic) -> Option<paths::Exchange> {
    (!returned.is_unit()).then(|| returned.cast::<paths::Exchange>())
}

#[test]
fn incidents_nature_view_and_triage_as_a_script_are_the_rust_ones() {
    let (engine, script) = engine("incident", include_str!("../scripts/incident.rhai"));
    let policies = [
        (incident::Policy::Triage, "triage"),
        (incident::Policy::RapidNorth, "rapid-north"),
        (incident::Policy::StabilizeFirst, "stabilize-first"),
    ];
    let instances = (0..4)
        .map(|seed| incident::initial(false, seed))
        .chain((0..2).map(|seed| incident::initial(true, seed)));
    for initial in instances {
        for (policy, name) in policies {
            let mut world = initial.clone();
            let ran = incident::run(&mut world, policy);
            let trace = ran
                .trace()
                .expect("a run keeps its trace")
                .exchanges()
                .to_vec();

            // At every state, the same Nature response, view and decision.
            let mut world = initial.clone();
            for exchange in trace.iter().cloned().chain(std::iter::once_with(|| {
                incident::action(incident::RateId::Wait)
            })) {
                let state = ScriptState::new(written(&world));
                let nature: rhai::Dynamic =
                    engine.call(&script, "nature", (state.clone(),)).unwrap();
                let expected = incident::nature_response(&world).map(|e| written(&e));
                assert_eq!(proposal(nature), expected);

                let seen: rhai::Array = engine.call(&script, "visible", (state.clone(),)).unwrap();
                let seen: std::collections::BTreeSet<String> = seen
                    .into_iter()
                    .map(|account| account.into_string().unwrap())
                    .collect();
                let view = incident::responder_view(&world);
                let visible: std::collections::BTreeSet<String> = view
                    .observation_key()
                    .visible_accounts()
                    .iter()
                    .map(|account| written::<axionomy::AccountPath>(account).to_string())
                    .collect();
                assert_eq!(seen, visible);

                let decided: rhai::Dynamic = engine
                    .call(&script, "decide_seen", (state, name.to_owned()))
                    .unwrap();
                let expected = incident::decision(&view, policy).map(|e| written(&e));
                assert_eq!(proposal(decided), expected);

                if world.matches(&incident::goal()) {
                    break;
                }
                world.apply(exchange).unwrap();
            }

            // Turn by turn, the script runs the same incident.
            let mut world: paths::Economy = written(&initial);
            let goal: paths::Goal = written(&incident::goal());
            let mut scripted = Vec::new();
            while !world.matches(&goal) && scripted.len() < incident::HORIZON {
                let turn: paths::Exchange = engine
                    .call(
                        &script,
                        "turn",
                        (ScriptState::new(world.clone()), name.to_owned()),
                    )
                    .unwrap();
                world.apply(turn.clone()).unwrap();
                scripted.push(turn);
            }
            let expected: Vec<paths::Exchange> = trace.iter().map(written).collect();
            assert_eq!(scripted, expected, "{name}");
        }
    }
}
