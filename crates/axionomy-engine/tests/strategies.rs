//! How a strategy's run ends, and that only what it reaches is recorded.

use axionomy::{Quantity, Rate, basket, path::Path, paths};
use axionomy_engine::{CandidateSet, Effort, Engine, Outcome, ScriptCall, Strategy};
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
