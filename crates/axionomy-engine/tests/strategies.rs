//! How a strategy's run ends, and that only what it reaches is recorded.

use axionomy::{Quantity, Rate, basket, path::Path, paths};
use axionomy_engine::{CandidateSet, Effort, Engine, Outcome, ScriptCall, Strategy};
use axionomy_search::{GraphSearchProgress, bounded::Tie};
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
    turns: 10,
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

/// `strategy`'s run from `start` toward the goal at `goal`.
fn run(
    engine: &Engine,
    start: Point,
    strategy: &Strategy,
    goal: &str,
    effort: Effort,
) -> axionomy_engine::Ran {
    engine
        .run_strategy(start, strategy, &path(goal), effort, &mut |_| {
            ControlFlow::Continue(())
        })
        .unwrap()
}

fn breadth_first() -> Strategy {
    Strategy::BreadthFirst {
        candidates: path("payments"),
    }
}

#[test]
fn a_search_reaches_the_goal_or_says_why_not() {
    let (engine, start) = bank(11);
    // Five, five and one: the fewest payments to eleven.
    let ran = run(&engine, start, &breadth_first(), "rich", EFFORT);
    assert!(matches!(ran.outcome, Outcome::Reached));
    assert_eq!(ran.end.step, start.step + 3);
    assert!(ran.thinking.unwrap().expanded() > 0);

    // The bank holds twenty: thirty is out of reach.
    let (engine, start) = bank(30);
    let ran = run(&engine, start, &breadth_first(), "rich", EFFORT);
    assert!(matches!(ran.outcome, Outcome::Exhausted));
    assert_eq!(ran.end, start);

    // A budget of one expansion is spent before the goal.
    let (engine, start) = bank(11);
    let ran = run(
        &engine,
        start,
        &breadth_first(),
        "rich",
        Effort {
            budget: 1,
            ..EFFORT
        },
    );
    assert!(matches!(ran.outcome, Outcome::Spent));
    assert_eq!(ran.end, start);

    // The caller stops it.
    let interrupted = engine
        .run_strategy(start, &breadth_first(), &path("rich"), EFFORT, &mut |_| {
            ControlFlow::Break(())
        })
        .unwrap();
    assert!(matches!(interrupted.outcome, Outcome::Interrupted));
}

#[test]
fn a_policy_runs_until_the_goal_nothing_a_refusal_or_its_turns() {
    let (engine, start) = bank(10);
    let policy = |function: &str| Strategy::Policy {
        policy: call(function),
    };
    let ran = run(&engine, start, &policy("five"), "rich", EFFORT);
    assert!(matches!(ran.outcome, Outcome::Reached));
    assert_eq!(ran.end.step, start.step + 2);
    assert!(engine.holds(ran.end, &path("rich")).unwrap());

    // One turn is not enough for two payments.
    let ran = run(
        &engine,
        start,
        &policy("five"),
        "rich",
        Effort { turns: 1, ..EFFORT },
    );
    assert!(matches!(ran.outcome, Outcome::Horizon));
    assert_eq!(ran.end.step, start.step + 1);

    let ran = run(&engine, start, &policy("nothing"), "rich", EFFORT);
    assert!(matches!(ran.outcome, Outcome::Stopped));
    assert_eq!(ran.end, start);

    // Fifty overdraws the bank: refused on the first turn, nothing recorded.
    let ran = run(&engine, start, &policy("too_much"), "rich", EFFORT);
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
            // As many payments of one as the run's seed draws, below four.
            fn drawn(state, random) {
                let plan = [];
                for paid in 0..random.next(4) { plan.push(pay(1)); }
                plan
            }
            "#,
        )
        .unwrap();
    let plan = |function: &str| Strategy::Plan {
        plan: ScriptCall::new(path("planner"), function.parse().unwrap()),
    };
    let ran = run(&engine, start, &plan("enough"), "rich", EFFORT);
    assert!(matches!(ran.outcome, Outcome::Reached));
    assert_eq!(ran.end.step, start.step + 2);

    // A plan short of the goal ends where it ends.
    let ran = run(&engine, start, &plan("short"), "rich", EFFORT);
    assert!(matches!(ran.outcome, Outcome::Stopped));
    assert_eq!(ran.end.step, start.step + 1);

    // The second payment overdraws the bank: nothing of the plan is applied.
    let ran = run(&engine, start, &plan("overdrawn"), "rich", EFFORT);
    assert!(matches!(ran.outcome, Outcome::Refused { turn: 1, .. }));
    assert_eq!(ran.end, start);

    let ran = run(&engine, start, &plan("none"), "rich", EFFORT);
    assert!(matches!(ran.outcome, Outcome::Stopped));
    assert_eq!(ran.end, start);

    // A plan that draws chance takes the run's random, seeded by its effort.
    let drawn = |seed| {
        let ran = run(
            &engine,
            start,
            &plan("drawn"),
            "rich",
            Effort { seed, ..EFFORT },
        );
        ran.end.step - start.step
    };
    let steps: Vec<u64> = (0..16).map(drawn).collect();
    assert_eq!(steps, (0..16).map(drawn).collect::<Vec<_>>());
    assert!(steps.iter().all(|steps| *steps < 4));
    assert!(steps.windows(2).any(|pair| pair[0] != pair[1]));
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
        cost: workshop_call("cost"),
        bound: workshop_call("cost"),
    };
    engine
        .keep_strategy(&path("workshop/bounded"), &bounded)
        .unwrap();
    let ran = run(&engine, start, &bounded, "done", EFFORT);
    assert!(matches!(ran.outcome, Outcome::Reached));
    assert_eq!(ran.cost, Some(4));
    assert_eq!(ran.end.step, start.step + 4);

    let narrow = Strategy::Beam {
        candidates: path("steps"),
        score: workshop_call("score"),
        width: 1,
        depth: 10,
        tie: Tie::First,
    };
    let ran = run(&engine, start, &narrow, "done", EFFORT);
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
    let dijkstra = Strategy::Cheapest {
        candidates: path("chores"),
        cost: cost.clone(),
        heuristic: None,
    };
    engine
        .keep_strategy(&path("chores/dijkstra"), &dijkstra)
        .unwrap();
    let ran = run(&engine, start, &dijkstra, "done", EFFORT);
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
        &dijkstra,
        "done",
        Effort {
            budget: 2,
            ..EFFORT
        },
    );
    assert!(matches!(ran.outcome, Outcome::Spent));
    assert_eq!(ran.end, start);

    // No chore is `e`: every order is tried, and none reaches it.
    let ran = run(&engine, start, &dijkstra, "impossible", EFFORT);
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
        score: workshop_call("progress"),
        width: 4,
        depth: 10,
        tie,
    };
    let rates = |tie| {
        let ran = run(&engine, start, &beam(tie), "done", EFFORT);
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
    let ran = run(&engine, start, &minimax, "over", EFFORT);
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
        chance: None,
        rollout: None,
        salt: 0,
    });
    engine.keep_strategy(&path("nim/mcts"), &searched).unwrap();
    let ran = run(&engine, start, &searched, "over", EFFORT);
    assert!(matches!(ran.outcome, Outcome::Reached));
    // The same seed plays the same game.
    let again = run(&engine, start, &searched, "over", EFFORT);
    assert_eq!(
        engine.history(ran.end).unwrap().1,
        engine.history(again.end).unwrap().1
    );
}

/// Begin, then play safe for 0.6 or gamble on Nature's draw: good for 1 one
/// time in four, bad for nothing otherwise.
fn gamble() -> (Engine, Point) {
    let engine = Engine::new(Store::memory().unwrap()).unwrap();
    let rate = |from: &str, to: &str| {
        Rate::new()
            .consume(path("game"), basket([(path(from), 1)]))
            .produce(path("game"), basket([(path(to), 1)]))
    };
    let weighed = |to: &str, weight: &str| {
        rate("pending", to)
            .preserve(path("nature"), basket([(path(weight), 1)]))
            .distinct(path("game"), path("nature"))
    };
    let economy = paths::EconomyBuilder::new()
        .account(path("game"), basket([(path("idle"), 1)]).into())
        .account(
            path("nature"),
            basket([(path("good-weight"), 1), (path("bad-weight"), 3)]).into(),
        )
        .rate(path("begin"), rate("idle", "ready"))
        .rate(path("safe"), rate("ready", "safe-result"))
        .rate(path("gamble"), rate("ready", "pending"))
        .rate(path("good"), weighed("good-result", "good-weight"))
        .rate(path("bad"), weighed("bad-result", "bad-weight"))
        .build()
        .unwrap();
    engine
        .keep_script(
            &path("gamble"),
            r#"
            fn play(rate) {
                let bindings = #{ game: "game" };
                if rate == "good" || rate == "bad" { bindings.nature = "nature"; }
                exchange(rate, bindings, 1)
            }
            fn moves(state) { ["begin", "safe", "gamble", "good", "bad"].map(play) }
            fn draws(state) {
                if state.balance("game", "pending").is_zero() { return (); }
                [[play("good"), state.balance("nature", "good-weight")],
                 [play("bad"), state.balance("nature", "bad-weight")]]
            }
            fn nature(state, random) {
                let outcomes = draws(state);
                if outcomes == () { return (); }
                outcomes[random.sample(outcomes.map(|outcome| outcome[1]))][0]
            }
            fn player(state) { 0 }
            fn payoffs(state) {
                if state.balance("game", "safe-result") > 0 { return [0.6]; }
                if state.balance("game", "good-result") > 0 { return [1.0]; }
                if state.balance("game", "bad-result") > 0 { return [0.0]; }
                ()
            }
            fn nothing(state) { [0.0] }
            fn first(state, candidates, random) { candidates[0] }
            fn wrong(state, candidates, random) { play("begin") }
            "#,
        )
        .unwrap();
    engine
        .keep_goal(
            &path("over"),
            &paths::Goal::new().require(path("game"), basket([(path("good-result"), 1)])),
        )
        .unwrap();
    engine
        .keep_candidates(
            &path("moves"),
            &CandidateSet::Script(ScriptCall::new(path("gamble"), "moves".parse().unwrap())),
        )
        .unwrap();
    let start = engine.author(economy).unwrap();
    (engine, start)
}

#[test]
fn tree_search_draws_natures_outcomes_and_simulates_by_its_rollout() {
    let (engine, start) = gamble();
    let call = |function: &str| ScriptCall::new(path("gamble"), function.parse().unwrap());
    // The game's three turns: the start, a choice and Nature's result.
    let effort = Effort { turns: 3, ..EFFORT };
    let searching = |chance: Option<&str>, rollout: Option<&str>| Strategy::Game {
        nature: Some(call("nature")),
        actor: call("player"),
        players: vec![axionomy_engine::Chooser::Mcts {
            candidates: path("moves"),
            payoffs: call("payoffs"),
            estimate: call("nothing"),
            iterations: 256,
            depth: 4,
            chance: chance.map(call),
            rollout: rollout.map(call),
            salt: 5,
        }],
    };
    let rates = |ran: &axionomy_engine::Ran| -> Vec<String> {
        let (_, exchanges) = engine.history(ran.end).unwrap();
        exchanges
            .iter()
            .map(|exchange| exchange.rate().as_str().to_owned())
            .collect()
    };

    // Knowing Nature draws, the player plays safe, and then has nothing
    // left to choose: the game stops short of a good result.
    let knowing = searching(Some("draws"), Some("first"));
    engine.keep_strategy(&path("knowing"), &knowing).unwrap();
    let ran = run(&engine, start, &knowing, "over", effort);
    assert!(matches!(ran.outcome, Outcome::Stopped));
    assert_eq!(rates(&ran), ["begin", "safe"]);

    // A tree without Nature thinks the player picks the good result.
    let ran = run(&engine, start, &searching(None, None), "over", effort);
    assert_eq!(rates(&ran)[..2], ["begin", "gamble"]);

    // A rollout must pick one of the candidates.
    let refused = engine.run_strategy(
        start,
        &searching(Some("draws"), Some("wrong")),
        &path("over"),
        effort,
        &mut |_| ControlFlow::Continue(()),
    );
    assert!(matches!(
        refused,
        Err(axionomy_engine::EngineError::Returned {
            expected: "one of the candidates or nothing",
            ..
        })
    ));
}

/// A coin Nature tosses and hides, and a player who calls it: calling at
/// once pays 1 when right, inspecting first, which reveals the coin, pays
/// 0.8, and a wrong call nothing.
fn coin() -> (Engine, Point) {
    let engine = Engine::new(Store::memory().unwrap()).unwrap();
    let agent = |from: &str, to: &[&str]| {
        Rate::new()
            .consume(path("agent"), basket([(path(from), 1)]))
            .produce(
                path("agent"),
                to.iter()
                    .map(|asset| (path(asset), Quantity::new(1)))
                    .collect(),
            )
    };
    let seeing = |rate: paths::Rate, truth: &str| {
        rate.preserve(path("nature"), basket([(path(truth), 1)]))
            .distinct(path("agent"), path("nature"))
    };
    let mut economy = paths::EconomyBuilder::new()
        .account(path("agent"), basket([(path("ready"), 1)]).into())
        .account(
            path("nature"),
            basket([
                (path("unresolved"), 1),
                (path("weight/heads"), 1),
                (path("weight/tails"), 1),
            ])
            .into(),
        )
        .rate(path("inspect"), agent("ready", &["awaiting", "inspected"]));
    for side in ["heads", "tails"] {
        economy = economy
            .rate(
                path(&format!("toss/{side}")),
                Rate::new()
                    .consume(path("nature"), basket([(path("unresolved"), 1)]))
                    .preserve(
                        path("nature"),
                        basket([(path(&format!("weight/{side}")), 1)]),
                    )
                    .produce(
                        path("nature"),
                        basket([(path(&format!("truth/{side}")), 1)]),
                    ),
            )
            .rate(
                path(&format!("reveal/{side}")),
                seeing(
                    agent("awaiting", &["deciding", &format!("told/{side}")]),
                    &format!("truth/{side}"),
                ),
            )
            .rate(
                path(&format!("guess/{side}")),
                agent("ready", &[&format!("called/{side}")]),
            )
            .rate(
                path(&format!("answer/{side}")),
                agent("deciding", &[&format!("called/{side}")]),
            );
        for truth in ["heads", "tails"] {
            let result = if side == truth { "won" } else { "lost" };
            economy = economy.rate(
                path(&format!("settle/{side}/{truth}")),
                seeing(
                    agent(&format!("called/{side}"), &["settled", result]),
                    &format!("truth/{truth}"),
                ),
            );
        }
    }
    engine
        .keep_script(
            &path("coin"),
            r#"
            fn play(rate) {
                let kind = rate.split("/")[0];
                let bindings = if kind == "toss" { #{ nature: "nature" } } else { #{ agent: "agent" } };
                if kind == "reveal" || kind == "settle" { bindings.nature = "nature"; }
                exchange(rate, bindings, 1)
            }
            // Nature tosses while the coin is unresolved, by its weights,
            // then reveals the coin to an inspection and settles a call.
            fn draws(state) {
                let found = [];
                if state.balance("nature", "unresolved") > 0 {
                    for side in ["heads", "tails"] {
                        found.push([play(`toss/${side}`), state.balance("nature", `weight/${side}`)]);
                    }
                    return found;
                }
                for rate in ["reveal/heads", "reveal/tails", "settle/heads/heads",
                             "settle/heads/tails", "settle/tails/heads", "settle/tails/tails"] {
                    let outcome = play(rate);
                    if state.applicable(outcome) { found.push([outcome, 1]); }
                }
                found
            }
            fn nature(state, random) {
                let outcomes = draws(state);
                if outcomes.is_empty() { return (); }
                outcomes[random.sample(outcomes.map(|outcome| outcome[1]))][0]
            }
            fn player(state) { 0 }
            fn own(state, player) { state.view(["agent"]) }
            fn everything(state, player) { state }
            fn calls(seen) {
                if seen.balance("agent", "ready") > 0 {
                    return ["inspect", "guess/heads", "guess/tails"].map(play);
                }
                if seen.balance("agent", "deciding") > 0 {
                    return ["answer/heads", "answer/tails"].map(play);
                }
                []
            }
            // A view cannot say what applies: that may hang on the coin.
            fn peeking(seen) { if seen.applicable(play("inspect")) { [play("inspect")] } else { [] } }
            fn payoffs(state) {
                if state.balance("agent", "won") > 0 {
                    return if state.balance("agent", "inspected") > 0 { [0.8] } else { [1.0] };
                }
                if state.balance("agent", "lost") > 0 { return [0.0]; }
                ()
            }
            fn nothing(state) { [0.0] }
            "#,
        )
        .unwrap();
    engine
        .keep_goal(
            &path("settled"),
            &paths::Goal::new().require(path("agent"), basket([(path("settled"), 1)])),
        )
        .unwrap();
    for (name, function) in [("calls", "calls"), ("peeking", "peeking")] {
        engine
            .keep_candidates(&path(name), &CandidateSet::Script(coin_call(function)))
            .unwrap();
    }
    let start = engine.author(economy.build().unwrap()).unwrap();
    (engine, start)
}

fn coin_call(function: &str) -> ScriptCall {
    ScriptCall::new(path("coin"), function.parse().unwrap())
}

/// The coin's game, its one player choosing by information-set search from
/// what `observe` shows it, over the candidate set `candidates`.
fn calling(observe: &str, candidates: &str) -> Strategy {
    Strategy::Game {
        nature: Some(coin_call("nature")),
        actor: coin_call("player"),
        players: vec![axionomy_engine::Chooser::Ismcts {
            candidates: path(candidates),
            observe: coin_call(observe),
            chance: coin_call("draws"),
            payoffs: coin_call("payoffs"),
            estimate: coin_call("nothing"),
            iterations: 400,
            depth: 6,
            rollout: None,
            salt: 3,
        }],
    }
}

#[test]
fn what_a_player_observes_decides_what_it_searches() {
    let (engine, start) = coin();
    let rates = |ran: &axionomy_engine::Ran| -> Vec<String> {
        let (_, exchanges) = engine.history(ran.end).unwrap();
        exchanges
            .iter()
            .map(|exchange| exchange.rate().as_str().to_owned())
            .collect()
    };
    let blind = calling("own", "calls");
    engine.keep_strategy(&path("coin/blind"), &blind).unwrap();
    let sighted = calling("everything", "calls");
    for side in ["heads", "tails"] {
        // Nature tossed before the game started: the player's beliefs
        // begin from the history of the point it starts from.
        let toss = paths::Exchange::new(path(&format!("toss/{side}")), Quantity::new(1))
            .bind(path("nature"), path("nature"));
        let tossed = engine.apply(start, &[toss]).unwrap();

        // Seeing only its own account, the player believes either side as
        // likely, so it inspects whichever way the coin landed, and then
        // calls the side it is shown.
        let ran = run(&engine, tossed, &blind, "settled", EFFORT);
        assert!(matches!(ran.outcome, Outcome::Reached));
        assert_eq!(
            rates(&ran),
            [
                format!("toss/{side}"),
                "inspect".to_owned(),
                format!("reveal/{side}"),
                format!("answer/{side}"),
                format!("settle/{side}/{side}"),
            ]
        );
        // The same seed searches the same worlds alike.
        assert_eq!(
            rates(&run(&engine, tossed, &blind, "settled", EFFORT)),
            rates(&ran)
        );

        // Seeing the coin, it calls the side at once.
        let ran = run(&engine, tossed, &sighted, "settled", EFFORT);
        assert_eq!(
            rates(&ran),
            [
                format!("toss/{side}"),
                format!("guess/{side}"),
                format!("settle/{side}/{side}"),
            ]
        );
    }

    // From before the toss, Nature tosses in the game itself.
    let ran = run(&engine, start, &blind, "settled", EFFORT);
    assert!(matches!(ran.outcome, Outcome::Reached));
    assert_eq!(rates(&ran)[1], "inspect");

    // A candidate set reads only the view: whether an exchange applies may
    // hang on what the player cannot see.
    let refused = engine.run_strategy(
        start,
        &calling("own", "peeking"),
        &path("settled"),
        EFFORT,
        &mut |_| ControlFlow::Continue(()),
    );
    assert!(
        matches!(
            &refused,
            Err(axionomy_engine::EngineError::Script { message, .. })
                if message.contains("not observable from a view")
        ),
        "{refused:?}"
    );
}

/// Whether a rollout's random draws the search's own tickets in the games
/// `searching` makes from a rollout, played toward `goal`: one choosing
/// uniformly by a weighted sample plays every game the search's own uniform
/// simulations play, for every seed; one drawing from another stream plays
/// some game otherwise.
fn rollouts_draw_the_searchs_own_tickets(
    engine: &Engine,
    start: Point,
    goal: &str,
    searching: impl Fn(Option<ScriptCall>) -> Strategy,
) {
    engine
        .keep_script(
            &path("rollouts"),
            r#"
            fn sampled(state, candidates, random) {
                candidates[random.sample(candidates.map(|candidate| 1))]
            }
            fn apart(state, candidates, random) { candidates[random.next(candidates.len())] }
            "#,
        )
        .unwrap();
    let rollout =
        |function: &str| Some(ScriptCall::new(path("rollouts"), function.parse().unwrap()));
    let played = |strategy: &Strategy, seed| {
        let ran = run(engine, start, strategy, goal, Effort { seed, ..EFFORT });
        engine.history(ran.end).unwrap().1
    };
    let (uniform, sampled, apart) = (
        searching(None),
        searching(rollout("sampled")),
        searching(rollout("apart")),
    );
    let mut differs = false;
    for seed in 0..32 {
        let game = played(&uniform, seed);
        assert_eq!(played(&sampled, seed), game, "seed {seed}");
        differs |= played(&apart, seed) != game;
    }
    assert!(differs);
}

#[test]
fn a_rollout_draws_the_searchs_own_tickets() {
    let (engine, start) = nim();
    rollouts_draw_the_searchs_own_tickets(&engine, start, "over", |rollout| {
        game(axionomy_engine::Chooser::Mcts {
            candidates: path("takes"),
            payoffs: nim_call("payoffs"),
            estimate: nim_call("even"),
            iterations: 3,
            depth: 10,
            chance: None,
            rollout,
            salt: 0,
        })
    });
    let (engine, start) = coin();
    rollouts_draw_the_searchs_own_tickets(&engine, start, "settled", |rollout| Strategy::Game {
        nature: Some(coin_call("nature")),
        actor: coin_call("player"),
        players: vec![axionomy_engine::Chooser::Ismcts {
            candidates: path("calls"),
            observe: coin_call("own"),
            chance: coin_call("draws"),
            payoffs: coin_call("payoffs"),
            estimate: coin_call("nothing"),
            iterations: 4,
            depth: 6,
            rollout,
            salt: 3,
        }],
    });
}

/// Runs `strategy` from `start` toward `goal`, its watcher stopping it at
/// the first look for which `stop`, given every look so far, says so; and
/// every look.
fn watched(
    engine: &Engine,
    start: Point,
    strategy: &Strategy,
    goal: &str,
    stop: impl Fn(&[GraphSearchProgress]) -> bool,
) -> (axionomy_engine::Ran, Vec<GraphSearchProgress>) {
    let mut looks = Vec::new();
    let ran = engine
        .run_strategy(start, strategy, &path(goal), EFFORT, &mut |progress| {
            looks.push(*progress);
            if stop(&looks) {
                ControlFlow::Break(())
            } else {
                ControlFlow::Continue(())
            }
        })
        .unwrap();
    (ran, looks)
}

/// A run its watcher stopped ends as a stopped search does: where it
/// started, with nothing recorded.
fn stopped(ran: &axionomy_engine::Ran, start: Point) {
    assert!(matches!(ran.outcome, Outcome::Interrupted), "{ran:?}");
    assert_eq!(ran.end, start);
    assert!(ran.cost.is_none());
}

#[test]
fn a_policy_and_a_plan_look_every_turn_and_stop_between_turns() {
    // Twenty in the bank: five a turn pays four turns, and the fifth overdraws.
    let (engine, start) = bank(30);
    let fives = Strategy::Policy {
        policy: call("five"),
    };
    let (ran, looks) = watched(&engine, start, &fives, "rich", |_| false);
    assert!(matches!(ran.outcome, Outcome::Refused { turn: 4, .. }));
    // A look before every turn, counting the turns played as expansions
    // and the states reached, its start among them, as visited.
    let turns: Vec<_> = looks
        .iter()
        .map(|look| {
            (
                look.expanded(),
                look.generated(),
                look.visited(),
                look.frontier(),
            )
        })
        .collect();
    assert_eq!(
        turns,
        [
            (0, 0, 1, 0),
            (1, 1, 2, 0),
            (2, 2, 3, 0),
            (3, 3, 4, 0),
            (4, 4, 5, 0)
        ]
    );
    // Stopped before its third turn, it records neither turn it played.
    let (ran, looks) = watched(&engine, start, &fives, "rich", |looks| looks.len() == 3);
    stopped(&ran, start);
    assert_eq!(looks.len(), 3);

    let (engine, start) = bank(10);
    engine
        .keep_script(
            &path("planner"),
            r#"fn enough(state) { [pay(), pay()] }
               fn pay() { exchange("pay", #{ from: "bank", to: "alice" }, 5) }"#,
        )
        .unwrap();
    let plan = Strategy::Plan {
        plan: ScriptCall::new(path("planner"), "enough".parse().unwrap()),
    };
    let (ran, looks) = watched(&engine, start, &plan, "rich", |_| false);
    assert!(matches!(ran.outcome, Outcome::Reached));
    // Before the plan is made, and once it is, its two exchanges generated.
    let generated: Vec<_> = looks.iter().map(|look| look.generated()).collect();
    assert_eq!(generated, [0, 2]);
    // Stopped before it is made, or once it is but before it is applied.
    for stop in [1, 2] {
        let (ran, _) = watched(&engine, start, &plan, "rich", |looks| looks.len() == stop);
        stopped(&ran, start);
    }
}

#[test]
fn a_game_stops_between_turns_and_while_a_player_searches() {
    let (engine, start) = nim();
    let minimax = game(axionomy_engine::Chooser::Minimax {
        candidates: path("takes"),
        evaluate: nim_call("evaluate"),
        depth: 6,
    });
    let searched = game(axionomy_engine::Chooser::Mcts {
        candidates: path("takes"),
        payoffs: nim_call("payoffs"),
        estimate: nim_call("even"),
        iterations: 400,
        depth: 10,
        chance: None,
        rollout: None,
        salt: 0,
    });
    for strategy in [&minimax, &searched] {
        let (ran, looks) = watched(&engine, start, strategy, "over", |_| false);
        assert!(matches!(ran.outcome, Outcome::Reached));
        // Watching changes nothing the game does.
        let unwatched = run(&engine, start, strategy, "over", EFFORT);
        assert_eq!(
            engine.history(ran.end).unwrap().1,
            engine.history(unwatched.end).unwrap().1
        );
        // The first player looks again and again while it searches, what
        // its search made counted as generated.
        let first: Vec<_> = looks.iter().filter(|look| look.expanded() == 0).collect();
        assert!(first.len() > 2, "{first:?}");
        assert!(
            first
                .windows(2)
                .all(|pair| pair[0].generated() <= pair[1].generated())
        );
        assert!(first.last().unwrap().generated() > 0);

        // Stopped while the first player searches, or before the second turn.
        let (ran, looks) = watched(&engine, start, strategy, "over", |looks| {
            looks.last().unwrap().generated() > 0
        });
        stopped(&ran, start);
        assert_eq!(looks.last().unwrap().expanded(), 0);
        let (ran, _) = watched(&engine, start, strategy, "over", |looks| {
            looks.last().unwrap().expanded() == 1
        });
        stopped(&ran, start);
    }
    // Tree search looks before each of its iterations.
    let (_, looks) = watched(&engine, start, &searched, "over", |_| false);
    let first = looks.iter().filter(|look| look.expanded() == 0).count();
    assert!(first > 400, "{first}");

    // An information-set player is stopped alike, while it searches.
    let (engine, start) = coin();
    let blind = calling("own", "calls");
    let (_, looks) = watched(&engine, start, &blind, "settled", |_| false);
    let searching = looks.iter().filter(|look| look.expanded() == 1).count();
    assert!(searching > 400, "{searching}");
    let (ran, looks) = watched(&engine, start, &blind, "settled", |looks| {
        let last = looks.last().unwrap();
        last.expanded() == 1 && last.generated() > 1
    });
    stopped(&ran, start);
    assert_eq!(looks.last().unwrap().expanded(), 1);
}

#[test]
fn searches_without_a_session_of_their_own_stop_as_they_expand() {
    let (engine, start) = workshop();
    let (chores_engine, chores_start, _, _) = chores();
    let strategies = [
        Strategy::BranchAndBound {
            candidates: path("steps"),
            cost: workshop_call("cost"),
            bound: workshop_call("cost"),
        },
        Strategy::Beam {
            candidates: path("steps"),
            score: workshop_call("score"),
            width: 1,
            depth: 10,
            tie: Tie::First,
        },
        Strategy::BestFirst {
            candidates: path("steps"),
            cost: workshop_call("cost"),
            heuristic: None,
        },
    ];
    let dijkstra = Strategy::Cheapest {
        candidates: path("chores"),
        cost: ScriptCall::new(path("chores"), "effort".parse().unwrap()),
        heuristic: None,
    };
    let runs = strategies
        .iter()
        .map(|strategy| (&engine, start, strategy))
        .chain([(&chores_engine, chores_start, &dijkstra)]);
    for (engine, start, strategy) in runs {
        let (ran, looks) = watched(engine, start, strategy, "done", |_| false);
        assert!(matches!(ran.outcome, Outcome::Reached), "{strategy:?}");
        // Watching changes nothing the search finds.
        let unwatched = run(engine, start, strategy, "done", EFFORT);
        assert_eq!(
            engine.history(ran.end).unwrap().1,
            engine.history(unwatched.end).unwrap().1,
            "{strategy:?}"
        );
        // A look before every expansion, counting what each offered.
        assert!(looks.len() > 1, "{strategy:?}");
        let last = looks.last().unwrap();
        assert_eq!(last.expanded(), looks.len() - 1, "{strategy:?}");
        assert!(last.generated() >= last.expanded(), "{strategy:?}");

        let (ran, looks) = watched(engine, start, strategy, "done", |looks| looks.len() == 2);
        stopped(&ran, start);
        assert_eq!(looks.len(), 2, "{strategy:?}");
    }
}
