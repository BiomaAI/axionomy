//! The multi-party marketplace as a package: its instances, the script its
//! searches call, each instance's goal of settling its orders, the
//! participants' utility, on-time deliveries and elapsed time as objectives,
//! its strategies, and an experiment comparing them on each instance.
//!
//! Each instance has orders of its own to settle, so each has its own goal,
//! which its experiment names. Micro settles an order in one exchange, and
//! its strategies are named under `marketplace/settlement`: its clearing is
//! branch-and-bound over the gross value a clearing forgoes, and its Pareto
//! picks rank the exact front of the participants' utility. Showcase and
//! Stress fund contracts that are dispatched and delivered over time,
//! planned by the same strategies at either size: a beam of 24 states scored
//! for volume, or weighted toward buyers or sellers.

use crate::{
    marketplace::{self, Ontology},
    package::{call, written},
};
use axionomy::{OntologyPath, path};
use axionomy_engine::{
    Arm, CandidateSet, Effort, Engine, EngineError, Experiment, GoalPath, Measure, Objective,
    ObjectivePath, Strategy, StrategyPath, View,
};
use axionomy_search::{
    bounded::Tie,
    pareto::ObjectiveDirection::{Maximize, Minimize},
};
use axionomy_store::Checkpoints;

/// The script the searches call, and the view's.
const SCRIPT: &str = include_str!("../../scripts/marketplace.rhai");
const VIEW: &str = include_str!("../../scripts/marketplace-view.rhai");

/// The states each depth of the fulfillment beam keeps.
const WIDTH: u64 = 24;
/// The beam runs forty rounds, each taking the goal states of its depth and
/// expanding the others, so a plan it keeps settles every order within 39
/// exchanges.
const DEPTH: u64 = 39;
/// Of the plans scoring best alike, the beam keeps the last it finds.
const TIE: Tie = Tie::Last;

/// Writes the marketplace's package into `engine`'s store, everything named
/// under `marketplace`.
pub fn write(engine: &Engine) -> Result<(), EngineError> {
    let view = |function| Some(call("marketplace/view", function));
    let call = |function| call("marketplace", function);
    engine.keep_script(&path!("marketplace"), SCRIPT)?;
    engine.keep_script(&path!("marketplace/view"), VIEW)?;
    engine.keep_view(
        &OntologyPath::of::<Ontology>(),
        &View {
            title: "Multi-party marketplace".into(),
            labels: view("label"),
            formats: view("format"),
            carry: None,
            scene: view("scene"),
            leaderboards: None,
        },
    )?;
    engine.keep_candidates(
        &path!("marketplace/settlements"),
        &CandidateSet::Script(call("settlements")),
    )?;
    engine.keep_candidates(
        &path!("marketplace/fulfillment"),
        &CandidateSet::Script(call("fulfillment")),
    )?;
    // Named by the keys the samples' objectives were reported under.
    let objective =
        |key: &str| -> ObjectivePath { format!("marketplace/{key}").parse().expect("a path") };
    for (key, function, direction) in [
        ("buyer-a", "buyer_a", Maximize),
        ("buyer-b", "buyer_b", Maximize),
        ("seller-a", "seller_a", Maximize),
        ("seller-b", "seller_b", Maximize),
        ("buyer-c", "buyer_c", Maximize),
        ("seller-c", "seller_c", Maximize),
        ("ontime", "on_time", Maximize),
        ("elapsed", "elapsed", Minimize),
        ("buyers", "front_buyers", Maximize),
        ("sellers", "front_sellers", Maximize),
    ] {
        engine.keep_objective(
            &objective(key),
            &Objective {
                measure: Measure::State(call(function)),
                direction,
            },
        )?;
    }
    // Micro's front spans buyer A's, buyer B's, seller A's and seller B's
    // utility; what its buyers and its sellers gain together are sums of
    // those, and leave the front as it is.
    let front = [
        "buyer-a", "buyer-b", "seller-a", "seller-b", "buyers", "sellers",
    ]
    .map(objective);
    let reported = [
        "buyer-a", "buyer-b", "seller-a", "seller-b", "buyer-c", "seller-c", "ontime", "elapsed",
    ]
    .map(objective);

    // Micro's market settles each order in one exchange; every other's
    // fulfills its contracts over time.
    let beam = |score| Strategy::Beam {
        candidates: path!("marketplace/fulfillment"),
        score: call(score),
        width: WIDTH,
        depth: DEPTH,
        tie: TIE,
    };
    // Order A's concession moves seven units of utility from seller to buyer.
    let pareto = |by: [&str; 3]| Strategy::Pareto {
        candidates: path!("marketplace/settlements"),
        objectives: front.to_vec(),
        by: by.map(objective).to_vec(),
    };
    let mut fulfilling = Vec::new();
    let mut settling = Vec::new();
    for (strategies, prefix, kept) in [
        (
            &mut fulfilling,
            "marketplace",
            [
                ("market-clearing", beam("volume")),
                ("pareto-buyers", beam("favor_buyers")),
                ("pareto-sellers", beam("favor_sellers")),
            ],
        ),
        (
            &mut settling,
            "marketplace/settlement",
            [
                (
                    "market-clearing",
                    Strategy::BranchAndBound {
                        candidates: path!("marketplace/settlements"),
                        cost: call("forgone"),
                        bound: call("forgone_settled"),
                    },
                ),
                ("pareto-buyers", pareto(["buyers", "buyer-b", "seller-b"])),
                ("pareto-sellers", pareto(["sellers", "seller-b", "buyer-b"])),
            ],
        ),
    ] {
        for (name, strategy) in kept {
            let path: StrategyPath = format!("{prefix}/{name}").parse().expect("a path");
            engine.keep_strategy(&path, &strategy)?;
            strategies.push(path);
        }
    }

    for (instance, initial, settled) in [
        ("micro", marketplace::initial(), marketplace::goal()),
        (
            "showcase",
            marketplace::initial_showcase(),
            marketplace::goal_showcase(),
        ),
        (
            "stress",
            marketplace::initial_stress(),
            marketplace::goal_stress(),
        ),
    ] {
        let goal: GoalPath = format!("marketplace/{instance}/settled")
            .parse()
            .expect("a path");
        engine.keep_goal(&goal, &written(&settled))?;
        let timed = marketplace::fulfillment::enabled(&initial);
        let strategies = if timed { &fulfilling } else { &settling };
        let arms = strategies
            .iter()
            .map(|strategy| Arm {
                strategy: strategy.clone(),
                start: None,
            })
            .collect();
        let start = engine
            .store()
            .record::<Ontology>(initial, Checkpoints::Ends)
            .finish()?;
        let name = format!("marketplace/{instance}");
        engine.refer(&name.parse().expect("a path"), start)?;
        engine.keep_experiment(
            &name.parse().expect("a path"),
            &Experiment {
                start: name.parse().expect("a path"),
                goal,
                arms,
                objectives: if timed {
                    reported.to_vec()
                } else {
                    front.to_vec()
                },
                effort: Effort {
                    seed: 17,
                    budget: 10_000_000,
                    // Its strategies search or plan; none plays turns.
                    turns: 0,
                },
                trials: 1,
            },
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axionomy_engine::ScriptState;
    use axionomy_store::Store;
    use std::{collections::BTreeSet, ops::ControlFlow};

    #[test]
    fn clearing_chooses_the_highest_reachable_settled_value() {
        let engine = Engine::new(Store::memory().unwrap()).unwrap();
        write(&engine).unwrap();
        let experiment = engine.experiment(&path!("marketplace/micro")).unwrap();
        let start = engine.referred(&experiment.start).unwrap();
        let candidates = path!("marketplace/settlements");
        let cost = |at| {
            let reached = engine.reach(at).unwrap();
            let economy = &reached.state.economy;
            let value: u64 = economy
                .accounts()
                .flat_map(|(_, account)| account.balances().iter())
                .filter_map(|(asset, count)| {
                    let price: u64 = asset
                        .as_str()
                        .strip_prefix("settled-value/")?
                        .parse()
                        .unwrap();
                    Some(price * count.to_u64())
                })
                .sum();
            let lost: i64 = engine
                .call(
                    &path!("marketplace"),
                    "forgone",
                    (ScriptState::new(economy.clone()),),
                )
                .unwrap();
            assert_eq!(lost, 190 - i64::try_from(value).unwrap());
            value
        };
        let mut values = BTreeSet::new();
        for first in engine.candidates(start, &candidates).unwrap() {
            let open = engine.apply(start, &[first]).unwrap();
            for second in engine.candidates(open, &candidates).unwrap() {
                let end = engine.apply(open, &[second]).unwrap();
                assert!(engine.holds(end, &experiment.goal).unwrap());
                values.insert(cost(end));
            }
        }
        assert_eq!(values, BTreeSet::from([183, 190]));
        let strategy = engine
            .strategy(&path!("marketplace/settlement/market-clearing"))
            .unwrap();
        let best = engine
            .run_strategy(
                start,
                &strategy,
                &experiment.goal,
                experiment.effort,
                &mut |_| ControlFlow::Continue(()),
            )
            .unwrap();
        assert!(engine.holds(best.end, &experiment.goal).unwrap());
        assert_eq!(cost(best.end), *values.last().unwrap());
    }
}
