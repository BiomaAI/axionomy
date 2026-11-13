//! How a strategy's run ends, and that only what it reaches is recorded.

use axionomy::{Quantity, Rate, basket, path::Path, paths};
use axionomy_engine::{CandidateSet, Effort, Engine, Outcome, ScriptCall, Strategy};
use axionomy_search::bounded::Tie;
use axionomy_store::{Point, Store};
use std::ops::ControlFlow;

fn path<K>(text: &str) -> Path<K> {
    text.parse().unwrap()
}

fn pay(units: u64) -> paths::Exchange {
    paths::Exchange::new(path("pay"), Quantity::new(units))
        .bind(path("from"), path("bank"))
        .bind(path("to"), path("alice"))
}

fn call(function: &str) -> ScriptCall {
    ScriptCall::new(path("bank"), function.parse().unwrap())
}

const EFFORT: Effort = Effort {
    seed: 7,
    budget: 10_000,
};

/// A bank paying alice, a goal of alice holding `wanted`, payments of one
/// and of five, and the point the economy starts at.
fn bank(wanted: u64) -> (Engine, Point) {
    let engine = Engine::new(Store::memory().unwrap()).unwrap();
    let economy = paths::EconomyBuilder::new()
        .account(path("bank"), basket([(path("cash"), 20)]).into())
        .account(path("alice"), basket([]).into())
        .rate(
            path("pay"),
            Rate::new()
                .consume(path("from"), basket([(path("cash"), 1)]))
                .produce(path("to"), basket([(path("cash"), 1)])),
        )
        .build()
        .unwrap();
    engine
        .keep_script(
            &path("bank"),
            r#"
            fn five(state, random) { exchange("pay", #{ from: "bank", to: "alice" }, 5) }
            fn nothing(state, random) { () }
            fn too_much(state, random) { exchange("pay", #{ from: "bank", to: "alice" }, 50) }
            "#,
        )
        .unwrap();
    engine
        .keep_goal(
            &path("rich"),
            &paths::Goal::new().require(path("alice"), basket([(path("cash"), wanted)])),
        )
        .unwrap();
    engine
        .keep_candidates(
            &path("payments"),
            &CandidateSet::Listed(vec![pay(1), pay(5)]),
        )
        .unwrap();
    let start = engine.author(economy).unwrap();
    (engine, start)
}

fn run(engine: &Engine, start: Point, strategy: &Strategy, effort: Effort) -> axionomy_engine::Ran {
    engine
        .run_strategy(start, strategy, effort, &mut |_| ControlFlow::Continue(()))
        .unwrap()
}

fn breadth_first() -> Strategy {
    Strategy::BreadthFirst {
        candidates: path("payments"),
        goal: path("rich"),
    }
}

#[test]
fn a_search_reaches_the_goal_or_says_why_not() {
    let (engine, start) = bank(11);
    // Five, five and one: the fewest payments to eleven.
    let ran = run(&engine, start, &breadth_first(), EFFORT);
    assert!(matches!(ran.outcome, Outcome::Reached));
    assert_eq!(ran.end.step, start.step + 3);
    assert!(ran.thinking.unwrap().expanded() > 0);

    // The bank holds twenty: thirty is out of reach.
    let (engine, start) = bank(30);
    let ran = run(&engine, start, &breadth_first(), EFFORT);
    assert!(matches!(ran.outcome, Outcome::Exhausted));
    assert_eq!(ran.end, start);

    // A budget of one expansion is spent before the goal.
    let (engine, start) = bank(11);
    let ran = run(
        &engine,
        start,
        &breadth_first(),
        Effort {
            budget: 1,
            ..EFFORT
        },
    );
    assert!(matches!(ran.outcome, Outcome::Spent));
    assert_eq!(ran.end, start);

    // The caller stops it.
    let interrupted = engine
        .run_strategy(start, &breadth_first(), EFFORT, &mut |_| {
            ControlFlow::Break(())
        })
        .unwrap();
    assert!(matches!(interrupted.outcome, Outcome::Interrupted));
}

#[test]
fn a_policy_runs_until_the_goal_nothing_a_refusal_or_its_horizon() {
    let (engine, start) = bank(10);
    let policy = |function: &str, horizon| Strategy::Policy {
        policy: call(function),
        goal: path("rich"),
        horizon,
    };
    let ran = run(&engine, start, &policy("five", 10), EFFORT);
    assert!(matches!(ran.outcome, Outcome::Reached));
    assert_eq!(ran.end.step, start.step + 2);
    assert!(engine.holds(ran.end, &path("rich")).unwrap());

    let ran = run(&engine, start, &policy("five", 1), EFFORT);
    assert!(matches!(ran.outcome, Outcome::Horizon));
    assert_eq!(ran.end.step, start.step + 1);

    let ran = run(&engine, start, &policy("nothing", 10), EFFORT);
    assert!(matches!(ran.outcome, Outcome::Stopped));
    assert_eq!(ran.end, start);

    // Fifty overdraws the bank: refused on the first turn, nothing recorded.
    let ran = run(&engine, start, &policy("too_much", 10), EFFORT);
    assert!(matches!(ran.outcome, Outcome::Refused { turn: 0, .. }));
    assert_eq!(ran.end, start);
}

#[test]
fn a_plan_is_applied_whole_or_not_at_all() {
    let (engine, start) = bank(10);
    engine
        .keep_script(
            &path("planner"),
            r#"
            fn pay(units) { exchange("pay", #{ from: "bank", to: "alice" }, units) }
            fn enough(state) { [pay(5), pay(5)] }
            fn short(state) { [pay(3)] }
            fn overdrawn(state) { [pay(5), pay(50)] }
            fn none(state) { () }
            "#,
        )
        .unwrap();
    let plan = |function: &str| Strategy::Plan {
        plan: ScriptCall::new(path("planner"), function.parse().unwrap()),
        goal: path("rich"),
    };
    let ran = run(&engine, start, &plan("enough"), EFFORT);
    assert!(matches!(ran.outcome, Outcome::Reached));
    assert_eq!(ran.end.step, start.step + 2);

    // A plan short of the goal ends where it ends.
    let ran = run(&engine, start, &plan("short"), EFFORT);
    assert!(matches!(ran.outcome, Outcome::Stopped));
    assert_eq!(ran.end.step, start.step + 1);

    // The second payment overdraws the bank: nothing of the plan is applied.
    let ran = run(&engine, start, &plan("overdrawn"), EFFORT);
    assert!(matches!(ran.outcome, Outcome::Refused { turn: 1, .. }));
    assert_eq!(ran.end, start);

    let ran = run(&engine, start, &plan("none"), EFFORT);
    assert!(matches!(ran.outcome, Outcome::Stopped));
    assert_eq!(ran.end, start);
}

/// A worker buying progress: a small step gives one for one coin, a big one
/// two for three. Four progress costs four coins at best.
fn workshop() -> (Engine, Point) {
    let engine = Engine::new(Store::memory().unwrap()).unwrap();
    let economy = paths::EconomyBuilder::new()
        .account(path("worker"), basket([]).into())
        .rate(
            path("small"),
            Rate::new().produce(
                path("worker"),
                basket([(path("progress"), 1), (path("cost"), 1)]),
            ),
        )
        .rate(
            path("big"),
            Rate::new().produce(
                path("worker"),
                basket([(path("progress"), 2), (path("cost"), 3)]),
            ),
        )
        .build()
        .unwrap();
    engine
        .keep_script(
            &path("workshop"),
            r#"
            fn cost(state) { state.balance("worker", "cost") }
            fn score(state) { -state.balance("worker", "cost").to_int() }
            fn progress(state) { state.balance("worker", "progress").to_int() }
            "#,
        )
        .unwrap();
    engine
        .keep_goal(
            &path("done"),
            &paths::Goal::new().require(path("worker"), basket([(path("progress"), 4)])),
        )
        .unwrap();
    let step = |rate: &str| {
        paths::Exchange::new(path(rate), Quantity::new(1)).bind(path("worker"), path("worker"))
    };
    engine
        .keep_candidates(
            &path("steps"),
            &CandidateSet::Listed(vec![step("big"), step("small")]),
        )
        .unwrap();
    let start = engine.author(economy).unwrap();
    (engine, start)
}

fn workshop_call(function: &str) -> ScriptCall {
    ScriptCall::new(path("workshop"), function.parse().unwrap())
}

#[test]
fn branch_and_bound_and_beam_find_the_cheapest_progress() {
    let (engine, start) = workshop();
    let bounded = Strategy::BranchAndBound {
        candidates: path("steps"),
        goal: path("done"),
        cost: workshop_call("cost"),
        bound: workshop_call("cost"),
    };
    engine
        .keep_strategy(&path("workshop/bounded"), &bounded)
        .unwrap();
    let ran = run(&engine, start, &bounded, EFFORT);
    assert!(matches!(ran.outcome, Outcome::Reached));
    assert_eq!(ran.cost, Some(4));
    assert_eq!(ran.end.step, start.step + 4);

    let narrow = Strategy::Beam {
        candidates: path("steps"),
        goal: path("done"),
        score: workshop_call("score"),
        width: 1,
        depth: 10,
        tie: Tie::First,
    };
    let ran = run(&engine, start, &narrow, EFFORT);
    assert!(matches!(ran.outcome, Outcome::Reached));
    assert!(engine.holds(ran.end, &path("done")).unwrap());
    assert_eq!(ran.end.step, start.step + 4);
}

/// One of each asset, `<asset>/<chore>`, of `chores`.
fn one_each(asset: &str, chores: &[&str]) -> paths::Basket {
    chores
        .iter()
        .map(|chore| (path(&format!("{asset}/{chore}")), Quantity::new(1)))
        .collect()
}

/// Four chores a worker does once each, in any order: three cost nothing
/// and one costs a unit of effort, so every order is equally cheap.
fn chores() -> (Engine, Point, paths::Economy, Vec<paths::Exchange>) {
    let engine = Engine::new(Store::memory().unwrap()).unwrap();
    let chores = ["a", "b", "c", "d"];
    let mut economy =
        paths::EconomyBuilder::new().account(path("worker"), one_each("todo", &chores).into());
    for chore in chores {
        let mut done = one_each("done", &[chore]);
        if chore == "d" {
            done.insert(path("effort"), Quantity::new(1));
        }
        economy = economy.rate(
            path(&format!("chore/{chore}")),
            Rate::new()
                .consume(path("worker"), one_each("todo", &[chore]))
                .produce(path("worker"), done),
        );
    }
    let economy = economy.build().unwrap();
    engine
        .keep_script(
            &path("chores"),
            r#"
            fn effort(before, exchange, after) {
                after.balance("worker", "effort") - before.balance("worker", "effort")
            }
            "#,
        )
        .unwrap();
    for (goal, chores) in [("done", &chores[..]), ("impossible", &["a", "e"][..])] {
        engine
            .keep_goal(
                &path(goal),
                &paths::Goal::new().require(path("worker"), one_each("done", chores)),
            )
            .unwrap();
    }
    let listed: Vec<paths::Exchange> = chores
        .iter()
        .map(|chore| {
            paths::Exchange::new(path(&format!("chore/{chore}")), Quantity::new(1))
                .bind(path("worker"), path("worker"))
        })
        .collect();
    engine
        .keep_candidates(&path("chores"), &CandidateSet::Listed(listed.clone()))
        .unwrap();
    let start = engine.author(economy.clone()).unwrap();
    (engine, start, economy, listed)
}

#[test]
fn the_cheapest_search_without_a_heuristic_is_the_samples_dijkstra() {
    let (engine, start, economy, listed) = chores();
    let cost = ScriptCall::new(path("chores"), "effort".parse().unwrap());
    let dijkstra = |goal: &str| Strategy::Cheapest {
        candidates: path("chores"),
        goal: path(goal),
        cost: cost.clone(),
        heuristic: None,
    };
    engine
        .keep_strategy(&path("chores/dijkstra"), &dijkstra("done"))
        .unwrap();
    let ran = run(&engine, start, &dijkstra("done"), EFFORT);
    assert!(matches!(ran.outcome, Outcome::Reached));
    assert_eq!(ran.cost, Some(1));
    // Of the equally cheap orders, the one `axionomy_search::dijkstra`
    // reaches, which need not be the first a search finds.
    let effort = |economy: &paths::Economy| economy.balance(&path("worker"), &path("effort"));
    let expected = axionomy_search::dijkstra(
        &economy,
        &engine.goal(&path("done")).unwrap(),
        |economy: &paths::Economy| economy.applicable(listed.clone()),
        |before: &paths::Economy, _: &paths::Exchange, after: &paths::Economy| {
            u64::try_from(effort(after).checked_sub(effort(before)).unwrap()).unwrap()
        },
    )
    .unwrap();
    assert_eq!(
        engine.history(ran.end).unwrap().1,
        expected.trace().exchanges()
    );

    // A budget of two expansions is spent before the fourth chore.
    let ran = run(
        &engine,
        start,
        &dijkstra("done"),
        Effort {
            budget: 2,
            ..EFFORT
        },
    );
    assert!(matches!(ran.outcome, Outcome::Spent));
    assert_eq!(ran.end, start);

    // No chore is `e`: every order is tried, and none reaches it.
    let ran = run(&engine, start, &dijkstra("impossible"), EFFORT);
    assert!(matches!(ran.outcome, Outcome::Exhausted));
    assert_eq!(ran.end, start);
}

#[test]
fn beam_keeps_the_first_or_the_last_of_goals_scoring_alike() {
    let (engine, start) = workshop();
    // Scored by progress alone, five progress is best: big, small and big
    // reach it at the third depth, three smalls and a big at the fourth.
    let beam = |tie| Strategy::Beam {
        candidates: path("steps"),
        goal: path("done"),
        score: workshop_call("progress"),
        width: 4,
        depth: 10,
        tie,
    };
    let rates = |tie| {
        let ran = run(&engine, start, &beam(tie), EFFORT);
        assert!(matches!(ran.outcome, Outcome::Reached));
        let (_, exchanges) = engine.history(ran.end).unwrap();
        exchanges
            .iter()
            .map(|exchange| exchange.rate().to_string())
            .collect::<Vec<_>>()
    };
    assert_eq!(rates(Tie::First), ["big", "small", "big"]);
    assert_eq!(rates(Tie::Last), ["small", "small", "small", "big"]);
}

/// Take one or two of four tokens in turn; whoever takes the last wins.
fn nim() -> (Engine, Point) {
    let engine = Engine::new(Store::memory().unwrap()).unwrap();
    let take = |count: u64| {
        Rate::new()
            .consume(path("pile"), basket([(path("token"), count)]))
            .produce(
                path("counter"),
                basket([(path("move"), 1), (path("taken"), count)]),
            )
    };
    let economy = paths::EconomyBuilder::new()
        .account(path("pile"), basket([(path("token"), 4)]).into())
        .account(path("counter"), basket([]).into())
        .rate(path("take-one"), take(1))
        .rate(path("take-two"), take(2))
        .build()
        .unwrap();
    engine
        .keep_script(
            &path("nim"),
            r#"
            fn player(state) { state.balance("counter", "move").to_int() % 2 }
            // An empty pile: whoever moved last, the player not to move, won.
            fn evaluate(state) {
                if !state.balance("pile", "token").is_zero() { return 0; }
                if player(state) == 1 { 1 } else { -1 }
            }
            fn payoffs(state) {
                if !state.balance("pile", "token").is_zero() { return (); }
                if player(state) == 1 { [1.0, 0.0] } else { [0.0, 1.0] }
            }
            fn even(state) { [0.5, 0.5] }
            "#,
        )
        .unwrap();
    engine
        .keep_goal(
            &path("over"),
            &paths::Goal::new().require(path("counter"), basket([(path("taken"), 4)])),
        )
        .unwrap();
    let take = |rate: &str| {
        paths::Exchange::new(path(rate), Quantity::new(1))
            .bind(path("pile"), path("pile"))
            .bind(path("counter"), path("counter"))
    };
    engine
        .keep_candidates(
            &path("takes"),
            &CandidateSet::Listed(vec![take("take-one"), take("take-two")]),
        )
        .unwrap();
    let start = engine.author(economy).unwrap();
    (engine, start)
}

fn nim_call(function: &str) -> ScriptCall {
    ScriptCall::new(path("nim"), function.parse().unwrap())
}

fn game(chooser: axionomy_engine::Chooser) -> Strategy {
    Strategy::Game {
        goal: path("over"),
        horizon: 10,
        nature: None,
        actor: nim_call("player"),
        players: vec![chooser.clone(), chooser],
    }
}

#[test]
fn players_choose_by_minimax_and_by_tree_search() {
    let (engine, start) = nim();
    let minimax = game(axionomy_engine::Chooser::Minimax {
        candidates: path("takes"),
        evaluate: nim_call("evaluate"),
        depth: 6,
    });
    engine
        .keep_strategy(&path("nim/minimax"), &minimax)
        .unwrap();
    let ran = run(&engine, start, &minimax, EFFORT);
    assert!(matches!(ran.outcome, Outcome::Reached));
    // The first player takes one, and takes the last token on the third move.
    let (_, exchanges) = engine.history(ran.end).unwrap();
    assert_eq!(exchanges.len(), 3);
    assert_eq!(exchanges[0].rate().as_str(), "take-one");

    let searched = game(axionomy_engine::Chooser::Mcts {
        candidates: path("takes"),
        payoffs: nim_call("payoffs"),
        estimate: nim_call("even"),
        iterations: 400,
        depth: 10,
    });
    engine.keep_strategy(&path("nim/mcts"), &searched).unwrap();
    let ran = run(&engine, start, &searched, EFFORT);
    assert!(matches!(ran.outcome, Outcome::Reached));
    // The same seed plays the same game.
    let again = run(&engine, start, &searched, EFFORT);
    assert_eq!(
        engine.history(ran.end).unwrap().1,
        engine.history(again.end).unwrap().1
    );
}
