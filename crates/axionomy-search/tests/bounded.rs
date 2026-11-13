//! Branch-and-bound, beam search and minimax on small economies whose best
//! answers are known.

use axionomy::{Quantity, Rate, basket, path::Path, paths};
use axionomy_search::bounded::{Tie, beam, branch_and_bound, minimax};

fn path<K>(text: &str) -> Path<K> {
    text.parse().unwrap()
}

/// A worker buying progress: a small step gives one for one coin, a big one
/// two for three coins. Four progress costs four coins at best.
fn workshop() -> paths::Economy {
    paths::EconomyBuilder::new()
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
        .unwrap()
}

fn step(rate: &str) -> paths::Exchange {
    paths::Exchange::new(path(rate), Quantity::new(1)).bind(path("worker"), path("worker"))
}

fn steps(_: &paths::Economy) -> Vec<paths::Exchange> {
    vec![step("big"), step("small")]
}

fn spent(economy: &paths::Economy) -> u64 {
    economy.balance(&path("worker"), &path("cost")).to_u64()
}

fn done() -> paths::Goal {
    paths::Goal::new().require(path("worker"), basket([(path("progress"), 4)]))
}

#[test]
fn branch_and_bound_finds_the_cheapest_goal_and_says_when_it_is_sure() {
    let economy = workshop();
    let found = branch_and_bound(&economy, &done(), steps, spent, spent, 1_000);
    assert!(found.exhausted);
    let solution = found.solution.unwrap();
    assert_eq!(solution.cost(), 4);
    assert!(
        solution
            .trace()
            .exchanges()
            .iter()
            .all(|exchange| *exchange == step("small"))
    );

    // Two expansions find a goal, but not the cheapest, and know it.
    let early = branch_and_bound(&economy, &done(), steps, spent, spent, 3);
    assert!(!early.exhausted);
    assert!(early.solution.is_none_or(|solution| solution.cost() > 4));
}

#[test]
fn beam_search_keeps_the_best_states_of_each_depth() {
    let economy = workshop();
    let score = |economy: &paths::Economy| -i64::try_from(spent(economy)).unwrap();
    let found = beam(&economy, &done(), steps, score, 1, 10, Tie::First);
    let best = found.best.unwrap();
    assert_eq!(best.score, -4);
    assert_eq!(best.trace.exchanges().len(), 4);

    // Too shallow to reach the goal.
    assert!(
        beam(&economy, &done(), steps, score, 4, 1, Tie::First)
            .best
            .is_none()
    );

    // Scored by progress alone, five progress is best: big, small and big
    // reach it first, at the third depth; three smalls and a big last, at
    // the fourth.
    let progress = |economy: &paths::Economy| {
        i64::try_from(economy.balance(&path("worker"), &path("progress")).to_u64()).unwrap()
    };
    let rates = |tie| {
        let best = beam(&economy, &done(), steps, progress, 4, 10, tie)
            .best
            .unwrap();
        assert_eq!(best.score, 5);
        best.trace
            .exchanges()
            .iter()
            .map(|exchange| exchange.rate().to_string())
            .collect::<Vec<_>>()
    };
    assert_eq!(rates(Tie::First), ["big", "small", "big"]);
    assert_eq!(rates(Tie::Last), ["small", "small", "small", "big"]);
}

/// Take one or two tokens from a pile in turn; whoever takes the last wins.
fn nim(tokens: u64) -> paths::Economy {
    let take = |count: u64| {
        Rate::new()
            .consume(path("pile"), basket([(path("token"), count)]))
            .produce(path("counter"), basket([(path("move"), 1)]))
    };
    paths::EconomyBuilder::new()
        .account(path("pile"), basket([(path("token"), tokens)]).into())
        .account(path("counter"), basket([]).into())
        .rate(path("take-one"), take(1))
        .rate(path("take-two"), take(2))
        .build()
        .unwrap()
}

fn take(rate: &str) -> paths::Exchange {
    paths::Exchange::new(path(rate), Quantity::new(1))
        .bind(path("pile"), path("pile"))
        .bind(path("counter"), path("counter"))
}

#[test]
fn minimax_takes_the_winning_move() {
    let moves =
        |economy: &paths::Economy| economy.balance(&path("counter"), &path("move")).to_u64();
    let actor = |economy: &paths::Economy| usize::from(moves(economy) % 2 == 1);
    // An empty pile: whoever moved last, the player not to move, won.
    let evaluate = |economy: &paths::Economy| {
        if economy.balance(&path("pile"), &path("token")).is_zero() {
            if actor(economy) == 1 { 1 } else { -1 }
        } else {
            0
        }
    };
    let candidates = |_: &paths::Economy| vec![take("take-one"), take("take-two")];
    // From four, taking one leaves three, which loses for whoever must move.
    let chosen = minimax(&nim(4), 4, candidates, actor, evaluate).unwrap();
    assert_eq!(chosen.exchange, take("take-one"));
    assert_eq!(chosen.value, 1);
    // From three, every move loses against best play.
    let chosen = minimax(&nim(3), 3, candidates, actor, evaluate).unwrap();
    assert_eq!(chosen.value, -1);
}
