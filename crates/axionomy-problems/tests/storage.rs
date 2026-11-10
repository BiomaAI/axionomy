//! Every canonical problem's economies can be stored and read back as they
//! were: each identifier type has one JSON encoding, which the store checks.

use axionomy::Economy;
use axionomy_problems::{
    amm, bridge, connect_four, exact_cover, lending, logistics, marketplace, maze, mission,
    perishables, rescue, scheduling, sokoban, work_league, workshop,
};
use axionomy_search::rollout::{Episodes, Unrecorded};
use axionomy_store::{
    Checkpoints, Group, GroupId, GroupRecorder, Point, Recordable, Recorder, Store, state,
};
use std::{collections::BTreeMap, num::NonZeroU64};

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

/// Where a store keeps a Monte Carlo estimate's episodes: one group per
/// policy, one run per rollout, with the rollout's elapsed time as its metric.
struct Recorded {
    store: Store,
    groups: BTreeMap<logistics::Policy, GroupRecorder>,
    ends: Vec<(logistics::Policy, Point, logistics::MissionRollout)>,
}

impl Episodes<logistics::Ontology, logistics::Policy, logistics::MissionRollout> for Recorded {
    type World = Recorder<logistics::Ontology>;

    fn begin(
        &mut self,
        policy: &logistics::Policy,
        _: usize,
        initial: Economy<logistics::Ontology>,
    ) -> Self::World {
        let store = &self.store;
        self.groups
            .entry(*policy)
            .or_insert_with(|| {
                store.group(Group {
                    kind: "monte-carlo".parse().unwrap(),
                    source: None,
                    tags: BTreeMap::from([("policy".parse().unwrap(), format!("{policy:?}"))]),
                })
            })
            .record(initial, Checkpoints::Every(NonZeroU64::new(4).unwrap()))
    }

    fn end(
        &mut self,
        policy: &logistics::Policy,
        sample: usize,
        world: Self::World,
        rollout: &logistics::MissionRollout,
    ) {
        let end = self.groups.get_mut(policy).unwrap().add(
            sample as u64,
            Some(sample as u64),
            world,
            BTreeMap::from([(
                "elapsed-time".parse().unwrap(),
                rollout.elapsed_time() as f64,
            )]),
        );
        self.ends.push((*policy, end.unwrap(), rollout.clone()));
    }
}

/// A canonical problem's Monte Carlo estimate records its rollouts where it
/// runs them: recording changes nothing it observes, every rollout is a run
/// that replays, each policy's group summarizes its samples, and a run's
/// stored states answer the questions asked of it afterwards.
#[test]
fn monte_carlo_records_each_policy_as_a_group_of_runs() {
    use logistics::{AccountId, Asset, Policy};

    let model = logistics::initial_micro();
    let mut recorded = Recorded {
        store: Store::memory().unwrap(),
        groups: BTreeMap::new(),
        ends: Vec::new(),
    };
    let estimate = logistics::monte_carlo(&model, 12, &mut recorded).unwrap();
    let unrecorded = logistics::monte_carlo(&model, 12, &mut Unrecorded).unwrap();
    assert_eq!(format!("{estimate:?}"), format!("{unrecorded:?}"));

    let Recorded {
        store,
        groups,
        ends,
    } = recorded;
    let groups: BTreeMap<Policy, GroupId> = groups
        .into_iter()
        .map(|(policy, group)| (policy, group.finish().unwrap()))
        .collect();
    assert_eq!(ends.len(), 24);

    let reader = store.read().unwrap();
    for (policy, group) in &groups {
        let elapsed: Vec<f64> = ends
            .iter()
            .filter(|(of, _, _)| of == policy)
            .map(|(_, _, rollout)| rollout.elapsed_time() as f64)
            .collect();
        let summary =
            &reader.group(*group).unwrap().unwrap().summaries[&"elapsed-time".parse().unwrap()];
        assert_eq!(summary.count, 12);
        assert!((summary.mean - elapsed.iter().sum::<f64>() / 12.0).abs() < 1e-9);
        let mut sorted = elapsed.clone();
        sorted.sort_by(f64::total_cmp);
        assert_eq!(
            reader
                .quantile(*group, &"elapsed-time".parse().unwrap(), 1.0)
                .unwrap(),
            sorted.last().copied()
        );
    }
    for (_, end, rollout) in &ends {
        reader.verify::<logistics::Ontology>(end.run).unwrap();
        let stored = reader.state::<logistics::Ontology>(*end).unwrap().unwrap();
        let replayed = model.replayed(rollout.trace()).unwrap();
        assert_eq!(stored.economy.state_key(), replayed.state_key());
    }

    // A time index: the vehicle's elapsed time only grows, so the first step
    // at which it reaches t is a bisection over the run's stored states.
    let (_, end, rollout) = &ends[0];
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
