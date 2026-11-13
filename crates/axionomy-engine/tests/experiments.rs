//! Experiments run strategies as arms over the same seeds and measure them
//! alike; comparison ranks any points on objectives.

use axionomy::{Quantity, Rate, basket, path::Path, paths};
use axionomy_engine::{
    Arm, CandidateSet, Chooser, Ended, Engine, Experiment, Measure, Objective, ScriptCall, Strategy,
};
use axionomy_search::pareto::ObjectiveDirection;
use axionomy_store::{Point, Store};
use std::ops::ControlFlow;

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

/// A bank paying alice until she holds ten; three ways of paying her; two
/// objectives: fewer payments, and more paid. The start is referred to as
/// `start`.
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
            fn fives(state, random) { exchange("pay", #{ from: "bank", to: "alice" }, 5) }
            fn ones(state, random) { exchange("pay", #{ from: "bank", to: "alice" }, 1) }
            fn randomly(state, random) {
                exchange("pay", #{ from: "bank", to: "alice" }, random.next(5) + 1)
            }
            fn paid(state) { state.balance("alice", "cash") }
            fn payment(before, exchange, after) { 1 }
            "#,
        )
        .unwrap();
    engine
        .keep_goal(
            &path("rich"),
            &paths::Goal::new().require(path("alice"), basket([(path("cash"), 10)])),
        )
        .unwrap();
    engine
        .keep_candidates(
            &path("payments"),
            &CandidateSet::Listed(vec![pay(1), pay(5)]),
        )
        .unwrap();
    for function in ["fives", "ones", "randomly"] {
        engine
            .keep_strategy(
                &path(function),
                &Strategy::Policy {
                    policy: call(function),
                    horizon: 20,
                },
            )
            .unwrap();
    }
    engine
        .keep_strategy(
            &path("searched"),
            &Strategy::BreadthFirst {
                candidates: path("payments"),
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
        .keep_objective(
            &path("paid"),
            &Objective {
                measure: Measure::State(call("paid")),
                direction: ObjectiveDirection::Maximize,
            },
        )
        .unwrap();
    let start = engine.author(economy).unwrap();
    engine.refer(&path("start"), start).unwrap();
    (engine, start)
}

fn experiment(arms: &[&str]) -> Experiment {
    Experiment {
        start: path("start"),
        goal: path("rich"),
        arms: arms
            .iter()
            .map(|strategy| Arm {
                strategy: path(strategy),
                start: None,
            })
            .collect(),
        objectives: vec![path("payments"), path("paid")],
        seed: 11,
        budget: 10_000,
        trials: 4,
    }
}

fn run(engine: &Engine, name: &str) -> axionomy_engine::Results {
    engine
        .run_experiment(&path(name), &mut |_| ControlFlow::Continue(()))
        .unwrap()
}

#[test]
fn every_arm_runs_every_trial_and_is_measured_alike() {
    let (engine, start) = bank();
    engine
        .keep_experiment(&path("paying"), &experiment(&["fives", "ones", "searched"]))
        .unwrap();
    let results = run(&engine, "paying");
    assert_eq!(results.arms.len(), 3);
    let values = |arm: usize| -> Vec<Vec<u64>> {
        results.arms[arm]
            .trials
            .iter()
            .map(|trial| {
                assert_eq!(trial.ended, Ended::Reached);
                trial.values.iter().map(|value| value.to_u64()).collect()
            })
            .collect()
    };
    // Two fives, ten ones, and the fewest payments a search finds.
    assert_eq!(values(0), vec![vec![2, 10]; 4]);
    assert_eq!(values(1), vec![vec![10, 10]; 4]);
    assert_eq!(values(2), vec![vec![2, 10]; 4]);
    assert_eq!(
        results.arms[0].totals(),
        vec![Quantity::new(8), Quantity::new(40)]
    );
    for arm in &results.arms {
        assert_eq!(arm.start, start);
        let seeds: Vec<u64> = arm.trials.iter().map(|trial| trial.seed).collect();
        assert_eq!(
            seeds,
            results.arms[0]
                .trials
                .iter()
                .map(|t| t.seed)
                .collect::<Vec<_>>()
        );
    }
    // The results are kept beside the experiment.
    assert_eq!(engine.results(&path("paying")).unwrap(), results);
}

#[test]
fn the_runs_are_recorded_as_a_group() {
    let (engine, start) = bank();
    engine
        .keep_experiment(&path("paying"), &experiment(&["fives", "ones"]))
        .unwrap();
    let results = run(&engine, "paying");
    let reader = engine.store().read().unwrap();
    let group = reader.group(results.group).unwrap().unwrap();
    assert_eq!(group.group.kind.as_str(), "experiment");
    assert_eq!(group.group.source, Some(start));
    let members = reader.members(results.group, 0, usize::MAX).unwrap();
    assert_eq!(members.len(), 8);
    let trials = results.arms.iter().flat_map(|arm| &arm.trials);
    for ((_, member), trial) in members.iter().zip(trials) {
        assert_eq!(member.run, Some(trial.end.run));
        assert_eq!(member.seed, Some(trial.seed));
    }
    assert_eq!(members[0].1.metrics[&path("fives/payments")], 2.0);
    assert_eq!(members[4].1.metrics[&path("ones/payments")], 10.0);
    assert_eq!(
        group.summaries[&path("ones/paid")].mean,
        10.0,
        "every trial of ones pays ten"
    );
}

#[test]
fn the_same_seed_runs_the_same_trials() {
    let (engine, _) = bank();
    engine
        .keep_experiment(&path("random"), &experiment(&["randomly"]))
        .unwrap();
    let first = run(&engine, "random");
    let again = run(&engine, "random");
    assert_ne!(first.group, again.group);
    let histories = |results: &axionomy_engine::Results| -> Vec<Vec<paths::Exchange>> {
        results.arms[0]
            .trials
            .iter()
            .map(|trial| engine.history(trial.end).unwrap().1)
            .collect()
    };
    assert_eq!(histories(&first), histories(&again));
    // Trials draw different seeds, so random payments differ between them.
    let seeds: std::collections::BTreeSet<u64> = first.arms[0]
        .trials
        .iter()
        .map(|trial| trial.seed)
        .collect();
    assert_eq!(seeds.len(), 4);
    assert!(
        histories(&first).windows(2).any(|pair| pair[0] != pair[1]),
        "four seeds pay alike"
    );
}

#[test]
fn an_arm_can_start_from_its_own_ref() {
    let (engine, start) = bank();
    let paid_five = engine.apply(start, &[pay(5)]).unwrap();
    engine.refer(&path("half-paid"), paid_five).unwrap();
    let mut halfway = experiment(&["ones"]);
    halfway.arms.push(Arm {
        strategy: path("ones"),
        start: Some(path("half-paid")),
    });
    engine.keep_experiment(&path("halfway"), &halfway).unwrap();
    let results = run(&engine, "halfway");
    assert_eq!(results.arms[1].start, paid_five);
    // The payments objective counts the whole history: one five, five ones.
    assert_eq!(
        results.arms[1].trials[0].values,
        vec![Quantity::new(6), Quantity::new(10)]
    );
    assert_eq!(
        results.arms[0].trials[0].values,
        vec![Quantity::new(10), Quantity::new(10)]
    );
}

#[test]
fn an_experiment_stopped_keeps_no_results() {
    let (engine, _) = bank();
    engine
        .keep_experiment(&path("paying"), &experiment(&["ones", "searched"]))
        .unwrap();
    let finished = run(&engine, "paying");
    // The watcher stops the first search: the experiment stops with it,
    // and the results it kept before stay.
    let stopped = engine.run_experiment(&path("paying"), &mut |_| ControlFlow::Break(()));
    assert!(matches!(
        stopped,
        Err(axionomy_engine::EngineError::Interrupted)
    ));
    assert_eq!(engine.results(&path("paying")).unwrap(), finished);
}

#[test]
fn an_experiment_names_only_what_is_kept() {
    let (engine, _) = bank();
    assert!(
        engine
            .keep_experiment(&path("bad"), &experiment(&["unknown"]))
            .is_err()
    );
    let mut unmeasured = experiment(&["ones"]);
    unmeasured.objectives.push(path("unknown"));
    assert!(engine.keep_experiment(&path("bad"), &unmeasured).is_err());
    let aimless = Experiment {
        goal: path("unknown"),
        ..experiment(&["ones"])
    };
    assert!(engine.keep_experiment(&path("bad"), &aimless).is_err());
    assert!(engine.results(&path("never-run")).is_err());
    assert!(engine.experiments().unwrap().is_empty());
    engine
        .keep_experiment(
            &path("nowhere"),
            &Experiment {
                start: path("unreferred"),
                ..experiment(&["ones"])
            },
        )
        .unwrap();
    engine
        .keep_experiment(&path("bank/paying"), &experiment(&["ones"]))
        .unwrap();
    assert_eq!(
        engine.experiments().unwrap(),
        [path("bank/paying"), path("nowhere")]
    );
    assert!(
        engine
            .run_experiment(&path("nowhere"), &mut |_| ControlFlow::Continue(()))
            .is_err()
    );
}

#[test]
fn comparison_finds_the_front_and_pick_orders_objectives() {
    let (engine, start) = bank();
    // Paying two fives, ten ones, or one five: fewer payments against more
    // paid.
    let fives = engine.apply(start, &[pay(5), pay(5)]).unwrap();
    let ones = engine.apply(start, &vec![pay(1); 10]).unwrap();
    let five = engine.apply(start, &[pay(5)]).unwrap();
    let points = [ones, fives, five];
    let objectives = [path("payments"), path("paid")];
    let compared = engine.compare(&points, &objectives).unwrap();
    assert_eq!(
        compared.values[0],
        (ones, vec![Quantity::new(10), Quantity::new(10)])
    );
    // Two fives dominate ten ones; one five pays less in fewer payments.
    assert_eq!(compared.front, vec![fives, five]);

    assert_eq!(engine.pick(&points, &objectives).unwrap(), Some(five));
    let paid_first = [path("paid"), path("payments")];
    assert_eq!(engine.pick(&points, &paid_first).unwrap(), Some(fives));
    assert_eq!(engine.pick(&[], &objectives).unwrap(), None);
}

/// A walker taking three steps while Nature draws the weather between them,
/// rain one time in four.
fn walk() -> Engine {
    let engine = Engine::new(Store::memory().unwrap()).unwrap();
    let weather =
        |kind: &str| Rate::new().produce(path("sky"), basket([(path(kind), 1), (path("turn"), 1)]));
    let economy = paths::EconomyBuilder::new()
        .account(path("sky"), basket([]).into())
        .account(path("walker"), basket([]).into())
        .rate(path("rain"), weather("rain"))
        .rate(path("sun"), weather("sun"))
        .rate(
            path("step"),
            Rate::new()
                .produce(path("walker"), basket([(path("progress"), 1)]))
                .produce(path("sky"), basket([(path("turn"), 1)])),
        )
        .build()
        .unwrap();
    engine
        .keep_script(
            &path("walk"),
            r#"
            fn weather(state, random) {
                if state.balance("sky", "turn").to_int() % 2 == 1 { return (); }
                if random.next(4) == 0 { exchange("rain", #{ sky: "sky" }, 1) }
                else { exchange("sun", #{ sky: "sky" }, 1) }
            }
            fn walker(state) { 0 }
            fn plain(state, random) {
                exchange("step", #{ walker: "walker", sky: "sky" }, 1)
            }
            // Draws a different number of times each turn before stepping.
            fn fidgety(state, random) {
                for draw in 0..random.next(5) { random.next(2); }
                exchange("step", #{ walker: "walker", sky: "sky" }, 1)
            }
            fn rain(state) { state.balance("sky", "rain") }
            "#,
        )
        .unwrap();
    engine
        .keep_goal(
            &path("there"),
            &paths::Goal::new().require(path("walker"), basket([(path("progress"), 3)])),
        )
        .unwrap();
    let walk_call = |function: &str| ScriptCall::new(path("walk"), function.parse().unwrap());
    for player in ["plain", "fidgety"] {
        engine
            .keep_strategy(
                &path(player),
                &Strategy::Game {
                    horizon: 10,
                    nature: Some(walk_call("weather")),
                    actor: walk_call("walker"),
                    players: vec![Chooser::Policy(walk_call(player))],
                },
            )
            .unwrap();
    }
    engine
        .keep_objective(
            &path("rain"),
            &Objective {
                measure: Measure::State(walk_call("rain")),
                direction: ObjectiveDirection::Minimize,
            },
        )
        .unwrap();
    let start = engine.author(economy).unwrap();
    engine.refer(&path("start"), start).unwrap();
    engine
}

#[test]
fn nature_samples_weighted_scenarios_whatever_the_players_think() {
    let engine = walk();
    engine
        .keep_experiment(
            &path("weather"),
            &Experiment {
                start: path("start"),
                goal: path("there"),
                arms: ["plain", "fidgety"]
                    .iter()
                    .map(|strategy| Arm {
                        strategy: path(strategy),
                        start: None,
                    })
                    .collect(),
                objectives: vec![path("rain")],
                seed: 3,
                budget: 0,
                trials: 16,
            },
        )
        .unwrap();
    let results = run(&engine, "weather");
    let weather = |arm: usize| -> Vec<Vec<String>> {
        results.arms[arm]
            .trials
            .iter()
            .map(|trial| {
                assert_eq!(trial.ended, Ended::Reached);
                engine
                    .history(trial.end)
                    .unwrap()
                    .1
                    .iter()
                    .map(|exchange| exchange.rate().as_str().to_owned())
                    .filter(|rate| rate != "step")
                    .collect()
            })
            .collect()
    };
    // The players draw differently; Nature's weather is the same.
    assert_eq!(weather(0), weather(1));
    let draws: Vec<String> = weather(0).concat();
    assert_eq!(draws.len(), 48);
    let rainy = draws.iter().filter(|draw| *draw == "rain").count();
    assert!((1..24).contains(&rainy), "rain {rainy} of 48 draws");
    assert_eq!(results.arms[0].totals(), vec![Quantity::new(rainy as u64)]);
}
