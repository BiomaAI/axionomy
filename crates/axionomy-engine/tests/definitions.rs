//! Goals, objectives and candidate sets kept as definitions and evaluated at
//! points, across the runs a history forked through.

use axionomy::{Quantity, Rate, basket, path::Path, paths};
use axionomy_engine::{
    CandidateSet, CandidatesPath, Engine, EngineError, GoalPath, Measure, Objective, ObjectivePath,
    ScriptCall, ScriptFunction, ScriptPath,
};
use axionomy_search::pareto::ObjectiveDirection;
use axionomy_store::Store;

fn path<K>(text: &str) -> Path<K> {
    text.parse().unwrap()
}

fn call(function: &str) -> ScriptCall {
    ScriptCall::new(path("bank"), function.parse().unwrap())
}

fn pay(units: u64) -> paths::Exchange {
    paths::Exchange::new(path("pay"), Quantity::new(units))
        .bind(path("from"), path("bank"))
        .bind(path("to"), path("alice"))
}

fn engine() -> Engine {
    let engine = Engine::new(Store::memory().unwrap()).unwrap();
    let script: ScriptPath = path("bank");
    engine
        .keep_script(
            &script,
            r#"
            fn alice(state) { state.balance("alice", "cash") }

            // What each payment gave alice.
            fn paid(before, exchange, after) {
                after.balance(exchange.account("to"), "cash")
                    - before.balance(exchange.account("to"), "cash")
            }

            fn payments(state) {
                [1, 10, 1000].map(|units| exchange("pay", #{ from: "bank", to: "alice" }, units))
                    .filter(|payment| state.applicable(payment))
            }
            "#,
        )
        .unwrap();
    engine
}

#[test]
fn goals_objectives_and_candidates_are_evaluated_at_points() {
    let engine = engine();
    let economy = paths::EconomyBuilder::new()
        .account(path("bank"), basket([(path("cash"), 100)]).into())
        .account(path("alice"), basket([(path("cash"), 5)]).into())
        .rate(
            path("pay"),
            Rate::new()
                .consume(path("from"), basket([(path("cash"), 1)]))
                .produce(path("to"), basket([(path("cash"), 1)])),
        )
        .build()
        .unwrap();
    let start = engine.author(economy).unwrap();
    let paid_ten = engine.apply(start, &[pay(10)]).unwrap();
    // A run forked from the first: the history crosses both.
    let paid_twenty = engine.apply(paid_ten, &[pay(4), pay(6)]).unwrap();

    let rich: GoalPath = path("alice/rich");
    engine
        .keep_goal(
            &rich,
            &paths::Goal::new().require(path("alice"), basket([(path("cash"), 25)])),
        )
        .unwrap();
    assert!(!engine.holds(paid_ten, &rich).unwrap());
    assert!(engine.holds(paid_twenty, &rich).unwrap());

    let holding: ObjectivePath = path("alice/cash");
    let received: ObjectivePath = path("alice/received");
    engine
        .keep_objective(
            &holding,
            &Objective {
                measure: Measure::State(call("alice")),
                direction: ObjectiveDirection::Maximize,
            },
        )
        .unwrap();
    engine
        .keep_objective(
            &received,
            &Objective {
                measure: Measure::Steps(call("paid")),
                direction: ObjectiveDirection::Maximize,
            },
        )
        .unwrap();
    assert_eq!(
        engine.measure(paid_twenty, &holding).unwrap(),
        Quantity::new(25)
    );
    assert_eq!(
        engine.measure(paid_twenty, &received).unwrap(),
        Quantity::new(20)
    );
    assert_eq!(engine.measure(start, &received).unwrap(), Quantity::ZERO);
    let economy = engine.reach(paid_twenty).unwrap().state.economy.clone();
    assert_eq!(
        engine.measure_economy(&holding, &economy).unwrap(),
        Quantity::new(25)
    );
    assert!(matches!(
        engine.measure_economy(&received, &economy),
        Err(EngineError::NeedsHistory(_))
    ));

    let payments: CandidatesPath = path("bank/payments");
    engine
        .keep_candidates(&payments, &CandidateSet::Script(call("payments")))
        .unwrap();
    assert_eq!(
        engine.candidates(start, &payments).unwrap(),
        vec![pay(1), pay(10)]
    );
    assert_eq!(
        engine.candidates(paid_twenty, &payments).unwrap(),
        vec![pay(1), pay(10)]
    );
}

#[test]
fn definitions_name_scripts_that_exist_and_are_read_back() {
    let engine = engine();
    let missing = ScriptCall::new(path("nowhere"), "alice".parse().unwrap());
    assert!(matches!(
        engine.keep_candidates(&path("bank/none"), &CandidateSet::Script(missing)),
        Err(EngineError::UnknownScript(_))
    ));
    assert!(matches!(
        engine.goal(&path("nobody/rich")),
        Err(EngineError::Undefined(name)) if name.as_str() == "goal/nobody/rich"
    ));
    let objective = Objective {
        measure: Measure::State(call("alice")),
        direction: ObjectiveDirection::Maximize,
    };
    engine
        .keep_objective(&path("alice/cash"), &objective)
        .unwrap();
    assert_eq!(engine.objective(&path("alice/cash")).unwrap(), objective);

    for name in ["decide_seen", "_private", "heuristic2"] {
        assert!(name.parse::<ScriptFunction>().is_ok(), "{name}");
    }
    for name in ["decide-seen", "9lives", "", "has space"] {
        assert!(name.parse::<ScriptFunction>().is_err(), "{name}");
    }
}
