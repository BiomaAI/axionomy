//! Scripts through the engine's host API: exact quantities, exchanges they
//! propose, views, seeded randomness, and scripts kept in the store.

use axionomy::{Quantity, Rate, basket, path::Path, paths};
use axionomy_engine::{Engine, EngineError, Random, ScriptPath, ScriptState, exchanges_of, rhai};
use axionomy_store::Store;

fn path<K>(text: &str) -> Path<K> {
    text.parse().unwrap()
}

fn economy() -> paths::Economy {
    paths::EconomyBuilder::new()
        .account(path("bank"), basket([(path("cash"), 100)]).into())
        .account(path("alice"), basket([(path("cash"), 5)]).into())
        .account(path("bob"), basket([]).into())
        .rate(
            path("pay"),
            Rate::new()
                .consume(path("from"), basket([(path("cash"), 1)]))
                .produce(path("to"), basket([(path("cash"), 1)])),
        )
        .build()
        .unwrap()
}

const SCRIPT: &str = r#"
    fn total(state) {
        state.balance("bank", "cash") + state.balance("alice", "cash")
    }

    // Pay each account the bank can afford to pay ten, in path order.
    fn candidates(state) {
        let found = [];
        for account in state.accounts("") {
            if account == "bank" { continue; }
            let paying = exchange("pay", #{ from: "bank", to: account }, 10);
            if state.applicable(paying) { found.push(paying); }
        }
        found
    }

    fn hidden(state) {
        state.view(["alice"]).balance("bank", "cash")
    }

    fn peek(state) {
        state.view(["alice"]).applicable(exchange("pay", #{ from: "bank", to: "bob" }, 1))
    }

    fn overdraw() {
        quantity(1) - quantity(2)
    }

    fn draws(random) {
        [random.next(1000), random.next(1000), random.next(1000)]
    }

    fn road(from, to) {
        let roads = [["base", "north"], ["north", "east"], ["base", "south"]];
        distances(roads, from)[to]
    }

    fn forever() {
        loop { }
    }
"#;

fn engine() -> (Engine, ScriptPath) {
    let engine = Engine::new(Store::memory().unwrap()).unwrap();
    let script: ScriptPath = path("bank/policy");
    engine.keep_script(&script, SCRIPT).unwrap();
    (engine, script)
}

#[test]
fn scripts_read_exact_quantities_and_propose_exchanges() {
    let (engine, script) = engine();
    let state = ScriptState::new(economy());
    let total: Quantity = engine.call(&script, "total", (state.clone(),)).unwrap();
    assert_eq!(total, Quantity::new(105));

    let proposed: rhai::Array = engine.call(&script, "candidates", (state,)).unwrap();
    let proposed = exchanges_of(proposed).unwrap();
    let expected: Vec<_> = ["alice", "bob"]
        .into_iter()
        .map(|to| {
            paths::Exchange::new(path("pay"), Quantity::new(10))
                .bind(path("from"), path("bank"))
                .bind(path("to"), path(to))
        })
        .collect();
    assert_eq!(proposed, expected);

    // Arithmetic is exact: a quantity never goes below zero.
    let overdrawn = engine.call::<Quantity>(&script, "overdraw", ());
    assert!(matches!(overdrawn, Err(EngineError::Script { .. })));
}

#[test]
fn a_view_shows_only_its_accounts_and_hides_applicability() {
    let (engine, script) = engine();
    let state = ScriptState::new(economy());
    let hidden: Quantity = engine.call(&script, "hidden", (state.clone(),)).unwrap();
    assert_eq!(hidden, Quantity::ZERO);
    let peek = engine.call::<bool>(&script, "peek", (state,));
    assert!(matches!(peek, Err(EngineError::Script { message, .. }) if message.contains("view")));
}

#[test]
fn randomness_is_the_hosts_and_replays_from_its_seed() {
    let (engine, script) = engine();
    let draws = |seed| -> Vec<i64> {
        let drawn: rhai::Array = engine.call(&script, "draws", (Random::new(seed),)).unwrap();
        drawn
            .into_iter()
            .map(|value| value.as_int().unwrap())
            .collect()
    };
    assert_eq!(draws(7), draws(7));
    assert_ne!(draws(7), draws(8));
    assert!(draws(7).iter().all(|value| (0..1000).contains(value)));
}

#[test]
fn distances_follow_the_roads_given() {
    let (engine, script) = engine();
    let far: i64 = engine.call(&script, "road", ("south", "east")).unwrap();
    assert_eq!(far, 3);
}

#[test]
fn scripts_are_kept_compiled_bounded_and_read_back() {
    let store = Store::memory().unwrap();
    let engine = Engine::new(store.clone()).unwrap();
    let script: ScriptPath = path("bank/policy");
    engine.keep_script(&script, SCRIPT).unwrap();

    // Another engine over the same store reads the script from it.
    let other = Engine::new(store).unwrap();
    assert_eq!(other.script(&script).unwrap().as_deref(), Some(SCRIPT));
    let total: Quantity = other
        .call(&script, "total", (ScriptState::new(economy()),))
        .unwrap();
    assert_eq!(total, Quantity::new(105));

    // A script that does not compile is refused and nothing is kept.
    let broken: ScriptPath = path("bank/broken");
    assert!(matches!(
        engine.keep_script(&broken, "fn total( {"),
        Err(EngineError::Script { .. })
    ));
    assert!(engine.script(&broken).unwrap().is_none());
    assert!(matches!(
        engine.call::<Quantity>(&broken, "total", ()),
        Err(EngineError::UnknownScript(_))
    ));

    // A script that never ends is stopped.
    assert!(matches!(
        engine.call::<()>(&script, "forever", ()),
        Err(EngineError::Script { .. })
    ));

    // What a script returns must be what the caller expects.
    assert!(matches!(
        engine.call::<bool>(&script, "total", (ScriptState::new(economy()),)),
        Err(EngineError::Returned { .. })
    ));
}
