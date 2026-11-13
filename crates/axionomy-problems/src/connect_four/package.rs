//! Connect Four as a package: its boards as `connect-four` economies, Micro
//! the compact 4×4 board and Showcase and Stress the standard 7×6 one; its
//! legal and tactical moves; Red's win as the goal; each player's terminal
//! value as an objective; and, for every instance, the study's three games
//! and an experiment comparing them.
//!
//! Every game is played turn by turn, with no Nature: every turn is a
//! choice. Red searches each of its drops by Monte Carlo tree search over
//! the tactical moves, simulating as the study's players do, with the
//! search's own tickets; Yellow, who also takes every drop's adjudication,
//! searches alike in self-play, or drops nearest the center, or as a
//! depth-five alpha-beta search scores. The search of the game's `k`th turn
//! is seeded with the study's seed plus `k`, as the study seeds it, and
//! searches as many iterations as the study gives a move at that size, so
//! each instance names its own games: `connect-four/<instance>/<document>`.
//! Its view draws the board, its pieces and the tactics of the player to
//! move.

use crate::{
    connect_four::{self, AccountId, Asset, Ontology, Player},
    package::{call, written},
};
use axionomy::{Economy, Goal, OntologyPath, basket, path};
use axionomy_engine::{
    Arm, CandidateSet, Chooser, Engine, EngineError, Experiment, Measure, Objective, ObjectivePath,
    Strategy, StrategyPath, View,
};
use axionomy_search::pareto::ObjectiveDirection::Maximize;
use axionomy_store::Checkpoints;

const SCRIPT: &str = include_str!("../../scripts/connect-four.rhai");
const VIEW: &str = include_str!("../../scripts/connect-four-view.rhai");

/// The seed of the study's default request.
const SEED: u64 = 17;
/// How deep a search's simulations look, counted from the position it
/// searches.
const DEPTH: u64 = 20;
/// The most turns an opponent game takes, on either board.
const OPPONENT_HORIZON: u64 = 85;

/// Writes Connect Four's package into `engine`'s store, everything named
/// under `connect-four`.
pub fn write(engine: &Engine) -> Result<(), EngineError> {
    let script = |function| call("connect-four", function);
    engine.keep_script(&path!("connect-four"), SCRIPT)?;
    engine.keep_script(&path!("connect-four/view"), VIEW)?;
    let view = |function| Some(call("connect-four/view", function));
    engine.keep_view(
        &OntologyPath::of::<Ontology>(),
        &View {
            title: "Connect Four".into(),
            labels: view("label"),
            formats: None,
            carry: None,
            scene: view("scene"),
            leaderboards: None,
        },
    )?;
    let red_wins = Goal::<Ontology>::new()
        .require(AccountId::Result, basket([(Asset::Winner(Player::Red), 1)]));
    engine.keep_goal(&path!("connect-four/red-wins"), &written(&red_wins))?;
    engine.keep_candidates(
        &path!("connect-four/moves"),
        &CandidateSet::Script(script("moves")),
    )?;
    engine.keep_candidates(
        &path!("connect-four/tactical"),
        &CandidateSet::Script(script("tactical")),
    )?;
    let mut objectives = Vec::new();
    for player in ["red", "yellow"] {
        let objective: ObjectivePath = format!("connect-four/{player}").parse().expect("a path");
        engine.keep_objective(
            &objective,
            &Objective {
                measure: Measure::State(script(player)),
                direction: Maximize,
            },
        )?;
        objectives.push(objective);
    }

    // The most turns self-play takes: a drop into every cell, each with its
    // adjudication on the standard board, and one more.
    let compact = u64::from(connect_four::WIDTH) * u64::from(connect_four::HEIGHT) + 1;
    let standard =
        u64::from(connect_four::STANDARD_WIDTH) * u64::from(connect_four::STANDARD_HEIGHT) * 2 + 1;
    // The study's iterations per move at its default budget of 128: Micro's
    // clamped to at most 64, Showcase's an eighth and Stress's a quarter.
    for (instance, initial, iterations, most) in [
        ("micro", connect_four::initial(), 64, compact),
        ("showcase", connect_four::initial_standard(), 16, standard),
        ("stress", connect_four::initial_standard(), 32, standard),
    ] {
        let searching = Chooser::Mcts {
            candidates: path!("connect-four/tactical"),
            payoffs: script("outcome"),
            estimate: script("estimate"),
            iterations,
            depth: DEPTH,
            chance: None,
            rollout: Some(script("playout")),
            salt: 0,
        };
        let game = |horizon, yellow| Strategy::Game {
            goal: path!("connect-four/red-wins"),
            horizon,
            nature: None,
            actor: script("mover"),
            players: vec![searching.clone(), yellow],
        };
        let mut arms = Vec::new();
        for (document, strategy) in [
            ("mcts-game", game(most, searching.clone())),
            (
                "center-opponent",
                game(OPPONENT_HORIZON, Chooser::Policy(script("center_opponent"))),
            ),
            (
                "search-opponent",
                game(OPPONENT_HORIZON, Chooser::Policy(script("search_opponent"))),
            ),
        ] {
            let name: StrategyPath = format!("connect-four/{instance}/{document}")
                .parse()
                .expect("a path");
            engine.keep_strategy(&name, &strategy)?;
            arms.push(Arm {
                strategy: name,
                start: None,
            });
        }
        let name = format!("connect-four/{instance}");
        recorded(engine, &name, initial)?;
        engine.keep_experiment(
            &name.parse().expect("a path"),
            &Experiment {
                start: name.parse().expect("a path"),
                arms,
                objectives: objectives.clone(),
                seed: SEED,
                // A game's searches spend their iterations, not a budget.
                budget: 1,
                trials: 1,
            },
        )?;
    }
    Ok(())
}

/// Records `economy` as a `connect-four` run, named by the ref `name`.
fn recorded(engine: &Engine, name: &str, economy: Economy<Ontology>) -> Result<(), EngineError> {
    let start = engine
        .store()
        .record::<Ontology>(economy, Checkpoints::Ends)
        .finish()?;
    engine.refer(&name.parse().expect("a path"), start)
}
