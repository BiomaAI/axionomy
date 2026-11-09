//! Every canonical problem's economies can be stored and read back as they
//! were: each identifier type has one JSON encoding, which the store checks.

use axionomy::Economy;
use axionomy_problems::{
    amm, bridge, connect_four, exact_cover, lending, logistics, marketplace, maze, mission,
    perishables, rescue, scheduling, sokoban, work_league, workshop,
};
use axionomy_store::{Checkpoints, Point, Recordable, Store, state};

fn round_trip<O: Recordable>(worlds: impl IntoIterator<Item = Economy<O>>) {
    let store = Store::memory().unwrap();
    for world in worlds {
        let end = store
            .record::<O>(world.clone(), Checkpoints::Ends)
            .finish()
            .unwrap();
        let stored = store
            .read()
            .unwrap()
            .state::<O>(Point {
                run: end.run,
                step: 0,
            })
            .unwrap()
            .unwrap();
        assert_eq!(stored.economy.state_key(), world.state_key(), "{}", O::NAME);
        assert_eq!(
            state::tree::<O>(&stored.economy).hash(),
            stored.root(),
            "{}",
            O::NAME
        );
    }
}

#[test]
fn every_problem_economy_is_stored_and_read_back_unchanged() {
    round_trip::<amm::Ontology>([
        amm::initial(),
        amm::initial_showcase(),
        amm::initial_stress(),
    ]);
    round_trip::<bridge::Ontology>([
        bridge::initial(),
        bridge::initial_showcase(),
        bridge::initial_stress(),
    ]);
    round_trip::<connect_four::Ontology>([connect_four::initial()]);
    round_trip::<exact_cover::Ontology>([
        exact_cover::initial(),
        exact_cover::initial_showcase(),
        exact_cover::initial_stress(),
    ]);
    round_trip::<lending::Ontology>([
        lending::initial(),
        lending::initial_showcase(),
        lending::initial_stress(),
    ]);
    round_trip::<logistics::Ontology>([logistics::initial(), logistics::initial_micro()]);
    round_trip::<logistics::fleet::Ontology>([
        logistics::fleet::initial(false, 17),
        logistics::fleet::initial(true, 17),
    ]);
    round_trip::<marketplace::Ontology>([
        marketplace::initial(),
        marketplace::initial_showcase(),
        marketplace::initial_stress(),
    ]);
    round_trip::<maze::Ontology>([
        maze::initial(),
        maze::initial_showcase(),
        maze::initial_stress(),
    ]);
    round_trip::<mission::Ontology>([mission::initial()]);
    round_trip::<mission::expedition::Ontology>([
        mission::expedition::initial(false, 17),
        mission::expedition::initial(true, 17),
    ]);
    round_trip::<perishables::Ontology>([perishables::initial()]);
    round_trip::<perishables::cold_chain::Ontology>([
        perishables::cold_chain::initial(false, 17),
        perishables::cold_chain::initial(true, 17),
    ]);
    round_trip::<rescue::Ontology>([
        rescue::uniform_uncertain(),
        rescue::uniform_uncertain_showcase(),
        rescue::uniform_uncertain_stress(),
    ]);
    round_trip::<rescue::incident::Ontology>([
        rescue::incident::initial(false, 17),
        rescue::incident::initial(true, 17),
    ]);
    round_trip::<scheduling::Ontology>([
        scheduling::initial(),
        scheduling::initial_showcase(),
        scheduling::initial_stress(),
    ]);
    round_trip::<sokoban::Ontology>([
        sokoban::initial(),
        sokoban::initial_showcase(),
        sokoban::initial_stress(),
    ]);
    round_trip::<work_league::Ontology>(
        [
            work_league::Profile::Micro,
            work_league::Profile::Showcase,
            work_league::Profile::Stress,
        ]
        .map(|profile| {
            work_league::league(
                profile,
                [
                    work_league::Policy::Sprinter,
                    work_league::Policy::Steward,
                    work_league::Policy::ValueHunter,
                    work_league::Policy::Resilient,
                ],
            )
            .initial()
            .clone()
        }),
    );
    round_trip::<workshop::Ontology>([
        workshop::initial(),
        workshop::initial_showcase(),
        workshop::initial_stress(),
    ]);
}

/// Monte Carlo rollouts of a canonical problem recorded as any sampler would
/// record them: one run per rollout, a group holding them with their metrics,
/// and the questions asked of them afterwards.
#[test]
fn rollouts_record_as_a_monte_carlo_group_with_a_time_index() {
    use axionomy_store::{Durability, Group, Member};
    use logistics::{AccountId, Asset, Policy};
    use std::collections::BTreeMap;

    let store = Store::memory().unwrap();
    let model = logistics::initial_micro();
    let group = store.mint_group();
    store
        .transact(Durability::Immediate, move |txn| {
            txn.create_group(
                group,
                Group {
                    kind: "monte-carlo".parse().unwrap(),
                    source: None,
                    tags: BTreeMap::from([("policy".parse().unwrap(), "reliable".into())]),
                },
            )
        })
        .wait()
        .unwrap();

    let mut rollouts = Vec::new();
    for seed in 0..12 {
        let rollout = logistics::run_policy(&model, Policy::Reliable, seed);
        let mut recorder = store.record::<logistics::Ontology>(
            model.clone(),
            Checkpoints::Every(std::num::NonZeroU64::new(4).unwrap()),
        );
        for exchange in rollout.trace().exchanges() {
            recorder.apply(exchange.clone()).unwrap();
        }
        let elapsed = rollout.elapsed_time() as f64;
        recorder
            .metric("elapsed-time".parse().unwrap(), elapsed, false)
            .unwrap();
        let end = recorder.finish().unwrap();
        let member = Member {
            run: Some(end.run),
            seed: Some(seed),
            metrics: BTreeMap::from([("elapsed-time".parse().unwrap(), elapsed)]),
        };
        store
            .transact(Durability::Eventual, move |txn| {
                txn.add_member(group, seed, &member)
            })
            .wait()
            .unwrap();
        rollouts.push((end, rollout));
    }

    let reader = store.read().unwrap();
    let elapsed: Vec<f64> = rollouts
        .iter()
        .map(|(_, rollout)| rollout.elapsed_time() as f64)
        .collect();
    let summary =
        &reader.group(group).unwrap().unwrap().summaries[&"elapsed-time".parse().unwrap()];
    assert_eq!(summary.count, 12);
    assert!((summary.mean - elapsed.iter().sum::<f64>() / 12.0).abs() < 1e-9);
    let mut sorted = elapsed.clone();
    sorted.sort_by(f64::total_cmp);
    assert_eq!(
        reader
            .quantile(group, &"elapsed-time".parse().unwrap(), 1.0)
            .unwrap(),
        sorted.last().copied()
    );

    // A time index: the vehicle's elapsed time only grows, so the first step
    // at which it reaches t is a bisection over the run's stored states.
    let (end, rollout) = &rollouts[0];
    let clock = |economy: &Economy<logistics::Ontology>| {
        economy
            .balance(&AccountId::Vehicle, &Asset::ElapsedTime)
            .to_u64()
    };
    for t in [0, 1, rollout.elapsed_time() / 2, rollout.elapsed_time()] {
        let found = reader
            .bisect::<logistics::Ontology>(end.run, 0, end.step, |economy| clock(economy) >= t)
            .unwrap();
        let scanned = (0..=end.step).find(|&step| {
            let state = reader
                .state::<logistics::Ontology>(Point { run: end.run, step })
                .unwrap()
                .unwrap();
            clock(&state.economy) >= t
        });
        assert_eq!(found, scanned, "time {t}");
    }
    let transitions = reader
        .transitions::<logistics::Ontology>(end.run, 0, usize::MAX)
        .unwrap();
    assert_eq!(transitions.len(), rollout.trace().exchanges().len());
    assert_eq!(
        transitions.last().map(|transition| transition.after),
        reader
            .state::<logistics::Ontology>(*end)
            .unwrap()
            .map(|state| state.root())
    );
}
