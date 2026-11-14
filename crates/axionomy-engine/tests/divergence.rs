//! Two histories compared: where they part, the exchange each side applied
//! at every step after, the balances the two states differ in, and where
//! they meet again.

use axionomy::{Quantity, Rate, basket, path::Path, paths};
use axionomy_engine::{
    AccountDifference, BalanceDifference, Engine, EngineError, Measure, Objective, ScriptCall, Side,
};
use axionomy_search::pareto::ObjectiveDirection;
use axionomy_store::{Point, Store};

fn path<K>(text: &str) -> Path<K> {
    text.parse().unwrap()
}

fn pay(units: u64) -> paths::Exchange {
    paths::Exchange::new(path("pay"), Quantity::new(units))
        .bind(path("from"), path("bank"))
        .bind(path("to"), path("alice"))
}

fn give(units: u64) -> paths::Exchange {
    paths::Exchange::new(path("give"), Quantity::new(units))
        .bind(path("from"), path("bank"))
        .bind(path("to"), path("alice"))
}

fn open(account: &str) -> paths::Exchange {
    paths::Exchange::new(path("open"), Quantity::new(1)).bind(path("new"), path(account))
}

/// A bank holding cash and gold that pays and gives to alice, and opens
/// accounts; two objectives: the cash alice holds, and how many exchanges
/// the history applied.
fn bank() -> (Engine, Point) {
    let engine = Engine::new(Store::memory().unwrap()).unwrap();
    let economy = paths::EconomyBuilder::new()
        .account(
            path("bank"),
            basket([(path("cash"), 100), (path("gold"), 10)]).into(),
        )
        .account(path("alice"), basket([]).into())
        .rate(
            path("pay"),
            Rate::new()
                .consume(path("from"), basket([(path("cash"), 1)]))
                .produce(path("to"), basket([(path("cash"), 1)])),
        )
        .rate(
            path("give"),
            Rate::new()
                .consume(path("from"), basket([(path("gold"), 1)]))
                .produce(path("to"), basket([(path("gold"), 1)])),
        )
        .rate(path("open"), Rate::new().opens(path("new")))
        .build()
        .unwrap();
    engine
        .keep_script(
            &path("bank"),
            r#"
            fn paid(state) { state.balance("alice", "cash") }
            fn exchanged(before, exchange, after) { 1 }
            "#,
        )
        .unwrap();
    let call = |function: &str| ScriptCall::new(path("bank"), function.parse().unwrap());
    engine
        .keep_objective(
            &path("paid"),
            &Objective {
                measure: Measure::State(call("paid")),
                direction: ObjectiveDirection::Maximize,
            },
        )
        .unwrap();
    engine
        .keep_objective(
            &path("exchanges"),
            &Objective {
                measure: Measure::Steps(call("exchanged")),
                direction: ObjectiveDirection::Minimize,
            },
        )
        .unwrap();
    let start = engine.author(economy).unwrap();
    (engine, start)
}

fn differs(account: &str, asset: &str, left: u64, right: u64) -> BalanceDifference {
    BalanceDifference {
        account: path(account),
        asset: path(asset),
        left: Quantity::new(left),
        right: Quantity::new(right),
    }
}

#[test]
fn a_branch_parts_from_its_parent_where_it_forked() {
    let (engine, start) = bank();
    let parent = engine.apply(start, &[pay(1), pay(2), pay(3)]).unwrap();
    let fork = engine.ancestor(parent, 1).unwrap();
    let branch = engine.apply(fork, &[give(4), pay(2)]).unwrap();
    let objectives = [path("paid"), path("exchanges")];

    let compared = engine
        .divergence(parent, branch, None, 100, &objectives)
        .unwrap();
    // The branch forked at step 1 of the parent's run, which names it.
    assert_eq!(
        compared.shared,
        Point {
            run: parent.run,
            step: 1
        }
    );
    assert_eq!(compared.shared, fork);
    assert_eq!(engine.shared(branch, parent).unwrap(), fork);
    let steps: Vec<_> = compared.steps.iter().map(|step| step.step).collect();
    assert_eq!(steps, [2, 3]);
    assert_eq!(compared.steps[0].left, Some(pay(2)));
    assert_eq!(compared.steps[0].right, Some(give(4)));
    // After step 2, the parent paid alice 3 cash and the branch 1 and 4 gold.
    assert_eq!(
        compared.steps[0].balances,
        [
            differs("alice", "cash", 3, 1),
            differs("alice", "gold", 0, 4),
            differs("bank", "cash", 97, 99),
            differs("bank", "gold", 10, 6),
        ]
    );
    assert!(compared.steps[0].accounts.is_empty());
    assert_eq!(compared.steps[1].left, Some(pay(3)));
    assert_eq!(compared.steps[1].right, Some(pay(2)));
    assert_eq!(
        compared.steps[1].balances,
        [
            differs("alice", "cash", 6, 3),
            differs("alice", "gold", 0, 4),
            differs("bank", "cash", 94, 97),
            differs("bank", "gold", 10, 6),
        ]
    );
    assert_eq!(compared.converged, None);
    // Each tip's objectives, exactly: the cash alice holds, and how many
    // exchanges each history applied.
    assert_eq!(compared.left.point, parent);
    assert_eq!(compared.left.values, [Quantity::new(6), Quantity::new(3)]);
    assert_eq!(compared.right.point, branch);
    assert_eq!(compared.right.values, [Quantity::new(3), Quantity::new(3)]);
    assert_eq!(compared.last(), 3);

    // The other way round, the sides swap.
    let swapped = engine.divergence(branch, parent, None, 100, &[]).unwrap();
    assert_eq!(swapped.shared, fork);
    assert_eq!(swapped.steps[0].left, Some(give(4)));
    assert_eq!(swapped.steps[0].balances[0], differs("alice", "cash", 1, 3));
    assert!(swapped.left.values.is_empty());
}

#[test]
fn a_history_that_ends_first_stays_in_its_last_state() {
    let (engine, start) = bank();
    let long = engine.apply(start, &[pay(1), pay(1), pay(1)]).unwrap();
    let short = engine.apply(start, &[give(2)]).unwrap();
    let compared = engine.divergence(long, short, None, 100, &[]).unwrap();
    assert_eq!(compared.shared, start);
    let exchanges: Vec<_> = compared
        .steps
        .iter()
        .map(|step| (step.step, step.left.clone(), step.right.clone()))
        .collect();
    assert_eq!(
        exchanges,
        [
            (1, Some(pay(1)), Some(give(2))),
            (2, Some(pay(1)), None),
            (3, Some(pay(1)), None),
        ]
    );
    // The short side is where its one exchange left it at every step after.
    assert_eq!(
        compared.steps[2].balances,
        [
            differs("alice", "cash", 3, 0),
            differs("alice", "gold", 0, 2),
            differs("bank", "cash", 97, 100),
            differs("bank", "gold", 10, 8),
        ]
    );

    // A point on the other's history shares itself: its side applies
    // nothing after it.
    let middle = engine.ancestor(long, 1).unwrap();
    let ahead = engine.divergence(middle, long, None, 100, &[]).unwrap();
    assert_eq!(ahead.shared, middle);
    assert!(ahead.steps.iter().all(|step| step.left.is_none()));
    assert_eq!(
        ahead.steps.last().unwrap().balances,
        [
            differs("alice", "cash", 1, 3),
            differs("bank", "cash", 99, 97),
        ]
    );
    // A point compared with itself parts nowhere.
    let itself = engine.divergence(long, long, None, 100, &[]).unwrap();
    assert_eq!(itself.shared, long);
    assert!(itself.steps.is_empty());
    assert_eq!(itself.converged, None);
}

#[test]
fn histories_that_reach_the_same_state_converge() {
    let (engine, start) = bank();
    let left = engine.apply(start, &[pay(2), give(1), pay(5)]).unwrap();
    let right = engine.apply(start, &[give(1), pay(2), pay(1)]).unwrap();
    let compared = engine.divergence(left, right, None, 100, &[]).unwrap();
    // Paying and giving in either order reaches the same state after two
    // steps: the tree hashes say so, and the step lists no difference.
    assert_eq!(compared.converged, Some(2));
    assert!(compared.steps[1].balances.is_empty());
    assert!(compared.steps[1].accounts.is_empty());
    assert_eq!(
        compared.steps[2].balances,
        [
            differs("alice", "cash", 7, 3),
            differs("bank", "cash", 93, 97),
        ]
    );

    // An account one side opens, empty, is a difference of its own.
    let opened = engine.apply(start, &[open("carol")]).unwrap();
    let paid = engine.apply(start, &[pay(1)]).unwrap();
    let compared = engine.divergence(opened, paid, None, 100, &[]).unwrap();
    assert_eq!(
        compared.steps[0].accounts,
        [AccountDifference {
            account: path("carol"),
            on: Side::Left,
        }]
    );
    assert_eq!(compared.converged, None);
}

#[test]
fn histories_part_across_the_runs_they_forked_from() {
    let (engine, start) = bank();
    // Each exchange applied alone forks a run of its own, as MCP applies them.
    let first = engine.apply(start, &[pay(1)]).unwrap();
    let second = engine.apply(first, &[pay(1)]).unwrap();
    let third = engine.apply(second, &[pay(1)]).unwrap();
    let other = engine.apply(first, &[give(1)]).unwrap();
    let compared = engine.divergence(third, other, None, 100, &[]).unwrap();
    assert_eq!(compared.shared, first);
    let steps: Vec<_> = compared.steps.iter().map(|step| step.step).collect();
    assert_eq!(steps, [2, 3]);
    assert_eq!(compared.steps[0].left, Some(pay(1)));
    assert_eq!(compared.steps[0].right, Some(give(1)));
    assert_eq!(compared.steps[1].right, None);
}

#[test]
fn histories_authored_apart_share_no_point() {
    let (engine, start) = bank();
    let economy = engine.reach(start).unwrap().state.economy.clone();
    // The same economy authored again is a history of its own.
    let again = engine.author(economy).unwrap();
    assert!(matches!(
        engine.divergence(start, again, None, 100, &[]),
        Err(EngineError::Unrelated { left, right }) if left == start && right == again
    ));
    let paid = engine.apply(again, &[pay(1)]).unwrap();
    assert!(matches!(
        engine.divergence(start, paid, None, 100, &[]),
        Err(EngineError::Unrelated { .. })
    ));
    // A point the store does not have is not compared.
    let missing = Point {
        run: start.run,
        step: 7,
    };
    assert!(matches!(
        engine.divergence(start, missing, None, 100, &[]),
        Err(EngineError::UnknownPoint(point)) if point == missing
    ));
}

#[test]
fn a_window_pages_through_the_steps_and_convergence_is_found_beyond_it() {
    let (engine, start) = bank();
    let left = engine.apply(start, &vec![pay(1); 10]).unwrap();
    let fork = engine.ancestor(left, 2).unwrap();
    // Gold first, then the same cash a step behind, then the gold back:
    // equal again once it is, at step 12, the left long ended at step 10.
    let mut exchanges = vec![give(1)];
    exchanges.extend(std::iter::repeat_n(pay(1), 8));
    exchanges.push(
        paths::Exchange::new(path("give"), Quantity::new(1))
            .bind(path("from"), path("alice"))
            .bind(path("to"), path("bank")),
    );
    let right = engine.apply(fork, &exchanges).unwrap();
    assert_eq!(right.step, 12);

    let page = engine.divergence(left, right, Some(5), 2, &[]).unwrap();
    assert_eq!(page.shared, fork);
    let steps: Vec<_> = page.steps.iter().map(|step| step.step).collect();
    assert_eq!(steps, [6, 7]);
    // Alice holds a cash less on the right, a step behind, and the gold.
    assert_eq!(
        page.steps[0].balances,
        [
            differs("alice", "cash", 6, 5),
            differs("alice", "gold", 0, 1),
            differs("bank", "cash", 94, 95),
            differs("bank", "gold", 10, 9),
        ]
    );
    // Where the two meet again is known whatever the window.
    assert_eq!(page.converged, Some(12));
    assert_eq!(page.last(), 12);

    // Steps after the last one listed follow; at the end, none.
    let next = engine.divergence(left, right, Some(7), 100, &[]).unwrap();
    let steps: Vec<_> = next.steps.iter().map(|step| step.step).collect();
    assert_eq!(steps, [8, 9, 10, 11, 12]);
    assert_eq!(next.steps[2].left, Some(pay(1)));
    assert_eq!(next.steps[3].left, None);
    assert!(next.steps[4].balances.is_empty());
    assert!(
        next.steps[3]
            .balances
            .contains(&differs("alice", "gold", 0, 1))
    );
    assert!(
        engine
            .divergence(left, right, Some(12), 100, &[])
            .unwrap()
            .steps
            .is_empty()
    );
    // A window from before the shared point starts after it.
    let from_start = engine.divergence(left, right, Some(0), 1, &[]).unwrap();
    assert_eq!(from_start.steps.len(), 1);
    assert_eq!(from_start.steps[0].step, 3);
    assert_eq!(from_start.converged, Some(12));
}

#[test]
fn unknown_objectives_are_refused() {
    let (engine, start) = bank();
    let paid = engine.apply(start, &[pay(1)]).unwrap();
    assert!(matches!(
        engine.divergence(start, paid, None, 10, &[path("missing")]),
        Err(EngineError::Undefined(_))
    ));
}
