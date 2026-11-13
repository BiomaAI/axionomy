//! A ref names the point an application is at; moving it forward is what
//! makes a step count, and only the steps it moves over are delivered.

use axionomy::{Quantity, Rate, basket, path::Path, paths};
use axionomy_engine::{Engine, EngineError, Following, Measure, Objective, ScriptCall};
use axionomy_search::pareto::ObjectiveDirection;
use axionomy_store::{Durability, Point, Store};
use std::{
    sync::{Arc, mpsc},
    thread,
    time::Duration,
};

fn path<K>(text: &str) -> Path<K> {
    text.parse().unwrap()
}

fn pay(units: u64) -> paths::Exchange {
    paths::Exchange::new(path("pay"), Quantity::new(units))
        .bind(path("from"), path("bank"))
        .bind(path("to"), path("alice"))
}

/// A bank paying alice, a goal of alice holding ten, and two objectives:
/// what she holds, and how many payments it took.
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
    engine
        .keep_script(
            &path("bank"),
            r#"
            fn paid(state) { state.balance("alice", "cash") }
            fn payment(before, exchange, after) { 1 }
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
            &path("payments"),
            &Objective {
                measure: Measure::Steps(call("payment")),
                direction: ObjectiveDirection::Minimize,
            },
        )
        .unwrap();
    engine
        .keep_goal(
            &path("rich"),
            &paths::Goal::new().require(path("alice"), basket([(path("cash"), 10)])),
        )
        .unwrap();
    let start = engine.author(economy).unwrap();
    (engine, start)
}

#[test]
fn a_ref_moves_only_forward_from_where_its_mover_expects() {
    let (engine, start) = bank();
    let live = path("live");
    engine.refer(&live, start).unwrap();
    // A ref is named once.
    assert!(matches!(
        engine.refer(&live, start),
        Err(EngineError::Moved { current: Some(at), .. }) if at == start
    ));

    // Two proposers from the same point: the first moves the ref, the
    // second is told where it went and looks again.
    let first = engine.apply(start, &[pay(5)]).unwrap();
    let second = engine.apply(start, &[pay(3)]).unwrap();
    engine.advance(&live, start, first).unwrap();
    assert!(matches!(
        engine.advance(&live, start, second),
        Err(EngineError::Moved { current: Some(at), .. }) if at == first
    ));
    // Nor does it move back, or sideways onto another history.
    assert!(matches!(
        engine.advance(&live, first, start),
        Err(EngineError::Backward { .. })
    ));
    assert!(matches!(
        engine.advance(&live, first, second),
        Err(EngineError::Backward { .. })
    ));
    assert_eq!(engine.referred(&live).unwrap(), first);
}

#[test]
fn a_move_already_made_succeeds_again() {
    let (engine, start) = bank();
    let live = path("live");
    engine.refer(&live, start).unwrap();
    let first = engine.apply(start, &[pay(5)]).unwrap();
    engine.advance(&live, start, first).unwrap();
    // A mover that never heard whether its move counted asks again: the
    // ref is at `first`, so the move asked for is the move made.
    engine.advance(&live, start, first).unwrap();
    assert_eq!(engine.referred(&live).unwrap(), first);
    let following = Following::default();
    assert_eq!(
        engine.frames(&live, start, &following, 10).unwrap().len(),
        1
    );

    // Once the ref has moved on, the same move is refused with where it is.
    let then = engine.apply(first, &[pay(1)]).unwrap();
    engine.advance(&live, first, then).unwrap();
    assert!(matches!(
        engine.advance(&live, start, first),
        Err(EngineError::Moved { current: Some(at), .. }) if at == then
    ));
}

#[test]
fn of_two_moves_committed_together_one_counts() {
    let (engine, start) = bank();
    let live = path("live");
    engine.refer(&live, start).unwrap();
    let rivals = [
        engine.apply(start, &[pay(5)]).unwrap(),
        engine.apply(start, &[pay(3)]).unwrap(),
    ];
    // The store's writer held while both movers queue their moves, so that
    // one transaction commits them together.
    let (release, held) = mpsc::channel::<()>();
    let gate = engine.store().transact(Durability::Immediate, move |_| {
        let _ = held.recv();
        Ok(())
    });
    let movers: Vec<_> = rivals
        .iter()
        .map(|&to| {
            let engine = engine.clone();
            let live = live.clone();
            thread::spawn(move || engine.advance(&live, start, to))
        })
        .collect();
    thread::sleep(Duration::from_millis(100));
    release.send(()).unwrap();
    gate.wait().unwrap();
    let moved: Vec<_> = movers
        .into_iter()
        .map(|mover| mover.join().unwrap())
        .collect();

    // The ref is where the move that counted took it, and the other mover
    // was told so.
    let at = engine.referred(&live).unwrap();
    for (to, moved) in rivals.into_iter().zip(moved) {
        if to == at {
            moved.unwrap();
        } else {
            assert!(matches!(
                moved,
                Err(EngineError::Moved { current: Some(current), .. }) if current == at
            ));
        }
    }
    assert_ne!(at, start);
}

#[test]
fn only_the_steps_a_ref_moves_over_are_delivered() {
    let (engine, start) = bank();
    let live = path("live");
    engine.refer(&live, start).unwrap();
    let first = engine.apply(start, &[pay(5)]).unwrap();
    // Tried out, never moved over: never delivered.
    engine.apply(first, &[pay(15)]).unwrap();
    engine.advance(&live, start, first).unwrap();
    let then = engine.apply(first, &[pay(1), pay(4)]).unwrap();
    engine.advance(&live, first, then).unwrap();

    let following = Following {
        objectives: vec![path("paid"), path("payments")],
        goal: Some(path("rich")),
    };
    let frames = engine.frames(&live, start, &following, 10).unwrap();
    let delivered: Vec<_> = frames
        .iter()
        .map(|frame| {
            (
                frame.point.step,
                frame.exchange.units().to_u64(),
                frame
                    .values
                    .iter()
                    .map(|value| value.to_u64())
                    .collect::<Vec<_>>(),
                frame.holds,
            )
        })
        .collect();
    assert_eq!(
        delivered,
        [
            (1, 5, vec![5, 1], Some(false)),
            (2, 1, vec![6, 2], Some(false)),
            (3, 4, vec![10, 3], Some(true)),
        ]
    );
    assert_eq!(frames[2].point, then);
    assert_eq!(frames[0].receipt.exchange().units(), Quantity::new(5));

    // A follower pulls from the last point it handled, a page at a time.
    let page = engine.frames(&live, start, &following, 2).unwrap();
    assert_eq!(page.len(), 2);
    let rest = engine.frames(&live, page[1].point, &following, 2).unwrap();
    assert_eq!(rest.len(), 1);
    assert_eq!(rest[0].point, then);
    assert!(
        engine
            .frames(&live, then, &following, 2)
            .unwrap()
            .is_empty()
    );
    // A point that is not on the ref's history is not where anyone follows
    // from.
    let elsewhere = engine.apply(start, &[pay(2)]).unwrap();
    assert!(matches!(
        engine.frames(&live, elsewhere, &following, 2),
        Err(EngineError::NotOnHistory { .. })
    ));
}

#[test]
fn a_follower_waits_for_the_ref_to_move() {
    let (engine, start) = bank();
    let engine = Arc::new(engine);
    let live = path("live");
    engine.refer(&live, start).unwrap();
    // Nothing moves: the wait ends with the time, where the ref is.
    assert_eq!(
        engine
            .wait(&live, start, Duration::from_millis(20))
            .unwrap(),
        start
    );
    let mover = {
        let engine = Arc::clone(&engine);
        let live = live.clone();
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(50));
            let next = engine.apply(start, &[pay(1)]).unwrap();
            engine.advance(&live, start, next).unwrap();
            next
        })
    };
    let moved = engine.wait(&live, start, Duration::from_secs(10)).unwrap();
    assert_eq!(moved, mover.join().unwrap());
}

/// A ref moved over three applies, the last of two steps: `start`, then
/// `first` and `second` a step each, then `fourth` two steps on.
fn moved_three_times(engine: &Engine, start: Point) -> [Point; 4] {
    let live = path("live");
    engine.refer(&live, start).unwrap();
    let first = engine.apply(start, &[pay(1)]).unwrap();
    engine.advance(&live, start, first).unwrap();
    let second = engine.apply(first, &[pay(2)]).unwrap();
    engine.advance(&live, first, second).unwrap();
    let fourth = engine.apply(second, &[pay(3), pay(4)]).unwrap();
    engine.advance(&live, second, fourth).unwrap();
    let third = Point {
        run: fourth.run,
        step: 3,
    };
    [first, second, third, fourth]
}

#[test]
fn a_follower_far_behind_receives_each_step_as_its_mover_named_it() {
    let (engine, start) = bank();
    let [first, second, third, fourth] = moved_three_times(&engine, start);
    let live = path("live");
    let following = Following::default();

    // Asked once the ref is three moves on, each step is the point the ref
    // was advanced to, not a name of it in a later run.
    let frames = engine.frames(&live, start, &following, 10).unwrap();
    let points: Vec<_> = frames.iter().map(|frame| frame.point).collect();
    assert_eq!(points, [first, second, third, fourth]);
    // Page by page, from each frame delivered.
    let mut after = start;
    let mut paged = Vec::new();
    while let [frame] = engine
        .frames(&live, after, &following, 1)
        .unwrap()
        .as_slice()
    {
        paged.push(frame.point);
        after = frame.point;
    }
    assert_eq!(paged, points);
}

#[test]
fn a_point_has_one_name() {
    let (engine, start) = bank();
    let [first, second, third, fourth] = moved_three_times(&engine, start);

    // A history names each of its points by the run whose step reached it.
    let history: Vec<_> = (0..=4)
        .map(|step| engine.ancestor(fourth, step).unwrap())
        .collect();
    assert_eq!(history, [start, first, second, third, fourth]);
    assert!(matches!(
        engine.ancestor(fourth, 5),
        Err(EngineError::NotOnHistory { step: 5, .. })
    ));

    // A fork's first state is the point it forked from, and has that name
    // alone: nothing takes it under the fork's.
    let unnamed = Point {
        run: second.run,
        step: first.step,
    };
    assert_eq!(engine.reach(second).unwrap().from, Some(first));
    let refused = |result: Result<_, EngineError>| matches!(result, Err(EngineError::UnknownPoint(at)) if at == unnamed);
    assert!(refused(engine.reach(unnamed).map(|_| ())));
    assert!(refused(engine.apply(unnamed, &[pay(1)]).map(|_| ())));
    assert!(refused(engine.apply(unnamed, &[]).map(|_| ())));
    assert!(refused(engine.ancestor(unnamed, 0).map(|_| ())));
    assert!(refused(engine.history(unnamed).map(|_| ())));
    assert!(refused(engine.refer(&path("other"), unnamed)));
    let live = path("live");
    let following = Following::default();
    assert!(refused(
        engine.frames(&live, unnamed, &following, 10).map(|_| ())
    ));
    let next = engine.apply(fourth, &[pay(1)]).unwrap();
    assert!(refused(engine.advance(&live, unnamed, next)));
}

/// A follower that cannot block, as in a browser's Worker, looks again
/// whenever the engine has moved a ref: making one or moving one forward,
/// never applying an exchange, a move refused or a move already made.
#[test]
fn the_engine_counts_every_move_of_a_ref() {
    let (engine, start) = bank();
    let live = path("live");
    let before = engine.moves();
    engine.refer(&live, start).unwrap();
    assert_eq!(engine.moves(), before + 1);
    let next = engine.apply(start, &[pay(1)]).unwrap();
    assert_eq!(engine.moves(), before + 1);
    assert!(matches!(
        engine.advance(&live, next, next),
        Err(EngineError::Moved { .. })
    ));
    assert_eq!(engine.moves(), before + 1);
    engine.advance(&live, start, next).unwrap();
    assert_eq!(engine.moves(), before + 2);
    engine.advance(&live, start, next).unwrap();
    assert_eq!(engine.moves(), before + 2);
}
