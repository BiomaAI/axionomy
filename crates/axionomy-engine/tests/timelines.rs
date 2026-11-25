//! The timeline tree: every run growing from a timeline's root, each a segment
//! from the point it forked at to its tip, and every change to it told to
//! whoever subscribed.

use axionomy::{Quantity, Rate, basket, path::Path, paths};
use axionomy_engine::{
    Asked, CandidateSet, Change, Effort, Engine, Segment, Strategy, StrategyPath,
};
use axionomy_store::{Point, Store};
use std::{
    ops::ControlFlow,
    sync::{Arc, Mutex},
};

fn path<K>(text: &str) -> Path<K> {
    text.parse().unwrap()
}

fn pay(units: u64) -> paths::Exchange {
    paths::Exchange::new(path("pay"), Quantity::new(units))
        .bind(path("from"), path("bank"))
        .bind(path("to"), path("alice"))
}

/// A bank that pays alice, as a run of its own.
fn bank() -> (Engine, Point) {
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
    let start = engine.author(economy).unwrap();
    (engine, start)
}

#[test]
fn the_tree_lists_every_branch_from_the_root_parents_first() {
    let (engine, start) = bank();
    let trunk = engine.apply(start, &[pay(1), pay(1), pay(1)]).unwrap();
    let middle = engine.ancestor(trunk, 2).unwrap();
    let branch = engine.apply(middle, &[pay(5)]).unwrap();
    let twig = engine.apply(branch, &[pay(2), pay(2)]).unwrap();
    let early = engine.apply(start, &[pay(3)]).unwrap();

    // Asked from any point of it, the tree is the same.
    for asked in [start, trunk, twig, early] {
        let tree = engine.timelines(&[asked]).unwrap();
        assert_eq!(
            tree,
            [
                Segment {
                    from: None,
                    tip: start,
                    asked: None,
                },
                Segment {
                    from: Some(start),
                    tip: trunk,
                    asked: None,
                },
                Segment {
                    from: Some(start),
                    tip: early,
                    asked: None,
                },
                Segment {
                    from: Some(middle),
                    tip: branch,
                    asked: None,
                },
                Segment {
                    from: Some(branch),
                    tip: twig,
                    asked: None,
                },
            ]
        );
    }
}

#[test]
fn separate_roots_make_separate_trees() {
    let (engine, start) = bank();
    let (_, other) = bank();
    // Another engine's store holds `other`; this one does not.
    assert!(engine.timelines(&[other]).is_err());
    let second = engine
        .author(engine.reach(start).unwrap().state.economy.clone())
        .unwrap();
    let grown = engine.apply(second, &[pay(4)]).unwrap();
    assert_eq!(
        engine.timelines(&[start, grown]).unwrap(),
        [
            Segment {
                from: None,
                tip: start,
                asked: None,
            },
            Segment {
                from: None,
                tip: second,
                asked: None,
            },
            Segment {
                from: Some(second),
                tip: grown,
                asked: None,
            },
        ]
    );
}

#[test]
fn subscribers_are_told_of_every_run_ref_and_definition() {
    let (engine, start) = bank();
    let told = Arc::new(Mutex::new(Vec::new()));
    let heard = told.clone();
    engine.subscribe(Arc::new(move |change: &Change| {
        heard.lock().unwrap().push(change.clone());
    }));
    let paid = engine.apply(start, &[pay(1)]).unwrap();
    engine.refer(&path("mine"), paid).unwrap();
    let more = engine.apply(paid, &[pay(2)]).unwrap();
    engine.advance(&path("mine"), paid, more).unwrap();
    engine.delete_ref(&path("mine"), more).unwrap();
    engine
        .keep_script(&path("note"), "fn nothing() { 0 }")
        .unwrap();
    assert_eq!(
        *told.lock().unwrap(),
        [
            Change::Run(Segment {
                from: Some(start),
                tip: paid,
                asked: None,
            }),
            Change::Ref {
                name: path("mine"),
                point: Some(paid),
            },
            Change::Run(Segment {
                from: Some(paid),
                tip: more,
                asked: None,
            }),
            Change::Ref {
                name: path("mine"),
                point: Some(more),
            },
            Change::Ref {
                name: path("mine"),
                point: None,
            },
            Change::Definitions,
        ]
    );
}

#[test]
fn a_run_a_kept_strategy_made_says_what_it_was_asked() {
    let (engine, start) = bank();
    engine
        .keep_candidates(
            &path("payments"),
            &CandidateSet::Listed(vec![pay(1), pay(5)]),
        )
        .unwrap();
    engine
        .keep_goal(
            &path("rich"),
            &paths::Goal::new().require(path("alice"), basket([(path("cash"), 6)])),
        )
        .unwrap();
    let strategy = Strategy::BreadthFirst {
        candidates: path("payments"),
    };
    let kept: StrategyPath = path("fewest");
    engine.keep_strategy(&kept, &strategy).unwrap();
    let asked = Asked {
        strategy: kept,
        goal: path("rich"),
        effort: Effort {
            seed: u64::MAX,
            budget: 100,
            turns: 0,
        },
    };
    let told = Arc::new(Mutex::new(Vec::new()));
    let heard = told.clone();
    engine.subscribe(Arc::new(move |change: &Change| {
        heard.lock().unwrap().push(change.clone());
    }));
    let ran = engine
        .ask(start, &asked, &mut |_| ControlFlow::Continue(()))
        .unwrap();
    let made = Segment {
        from: Some(start),
        tip: ran.end,
        asked: Some(asked.clone()),
    };
    // As it is recorded, and read back from the store.
    assert_eq!(*told.lock().unwrap(), [Change::Run(made.clone())]);
    assert_eq!(engine.timelines(&[start]).unwrap()[1], made);

    // A strategy kept nowhere, and an exchange applied, say nothing of one.
    let unkept = engine
        .run_strategy(start, &strategy, &asked.goal, asked.effort, &mut |_| {
            ControlFlow::Continue(())
        })
        .unwrap();
    let applied = engine.apply(start, &[pay(1)]).unwrap();
    for tip in [unkept.end, applied] {
        assert!(engine.timelines(&[start]).unwrap().contains(&Segment {
            from: Some(start),
            tip,
            asked: None,
        }));
    }
}
