//! Every sample problem's economies can be stored and read back as they
//! were: each identifier type has one JSON encoding, its path, which the store
//! checks, and which reads as the `paths` ontology too.

use axionomy::Economy;
use axionomy_problems::{
    amm, bridge, connect_four, exact_cover, lending, logistics, marketplace, maze, mission,
    perishables, rescue, scheduling, sokoban, work_league, workshop,
};
use axionomy_store::{Checkpoints, Point, Recordable, Store, state};

/// Stores each economy and reads it back, and reads its JSON as a `Paths`
/// economy: written down, a typed economy is a `Paths` economy with the same
/// canonical tree.
fn round_trip<O: Recordable>(worlds: impl IntoIterator<Item = Economy<O>>) {
    let store = Store::memory().unwrap();
    for world in worlds {
        let untyped: axionomy::paths::Economy =
            serde_json::from_value(serde_json::to_value(&world).unwrap())
                .unwrap_or_else(|error| panic!("{}: {error}", O::NAME));
        assert_eq!(
            state::tree::<axionomy::paths::Paths>(&untyped).hash(),
            state::tree::<O>(&world).hash(),
            "{}",
            O::NAME
        );

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

/// A typed economy written down is a `Paths` economy: its identifiers are
/// written as their paths, so the same JSON reads as either, both readings
/// have the same canonical tree, and replaying the same trace read either way
/// reaches the same canonical state.
#[test]
#[cfg(feature = "packages")]
fn a_typed_economy_written_down_is_a_paths_economy() {
    use axionomy::paths::{self, Paths};

    // The written form: a variant's kebab-case name, then its fields.
    assert_eq!(serde_json::json!(maze::AccountId::Agent), "agent");
    assert_eq!(
        serde_json::json!(maze::Asset::Edge(maze::Node::KeyRoom, maze::Node::Gate)),
        "edge/key-room/gate"
    );
    assert_eq!(
        serde_json::json!(maze::RateId::Move {
            from: maze::Node::Start,
            to: maze::Node::Atrium,
            energy: 3,
            needs_open_door: false,
        }),
        "move/start/atrium/3/false"
    );
    assert_eq!(
        serde_json::json!(work_league::RateId::Expire {
            job: work_league::JobId(3),
            agent: None,
            mode: Some(work_league::WorkMode::Safe),
        }),
        "expire/3/none/some/safe"
    );

    fn untyped<O: Recordable>(
        label: &str,
        world: &Economy<O>,
        trace: &axionomy::Trace<O>,
        end: &Economy<O>,
    ) {
        let read: paths::Economy =
            serde_json::from_value(serde_json::to_value(world).unwrap()).unwrap();
        assert_eq!(
            state::tree::<Paths>(&read).hash(),
            state::tree::<O>(world).hash(),
            "{label}: initial"
        );
        let trace: paths::Trace =
            serde_json::from_value(serde_json::to_value(trace).unwrap()).unwrap();
        let read_end = read.replayed(&trace).unwrap();
        assert_eq!(
            state::tree::<Paths>(&read_end).hash(),
            state::tree::<O>(end).hash(),
            "{label}: end"
        );
    }

    for experiment in ["maze/micro", "maze/showcase", "maze/stress"] {
        let (world, trace) =
            found::<maze::Ontology>(maze::package::write, experiment, "maze/a-star");
        untyped(experiment, &world, &trace, &world.replayed(&trace).unwrap());
    }
    let (world, trace) = found::<work_league::Ontology>(
        work_league::package::write,
        "work-league/micro",
        "work-league/serial/mixed-field",
    );
    untyped(
        "work-league/micro",
        &world,
        &trace,
        &world.replayed(&trace).unwrap(),
    );
}

/// Where the package `write` writes starts the experiment `experiment`, and
/// the exchanges its arm running `strategy` applies from there, both read
/// back as `O`'s: a package's history written down is typed `O` too.
#[cfg(feature = "packages")]
fn found<O: Recordable>(
    write: axionomy_problems::WritePackage,
    experiment: &str,
    strategy: &str,
) -> (Economy<O>, axionomy::Trace<O>)
where
    Economy<O>: serde::de::DeserializeOwned,
    axionomy::Exchange<O>: serde::de::DeserializeOwned,
{
    let engine = axionomy_engine::Engine::new(Store::memory().unwrap()).unwrap();
    write(&engine).unwrap();
    let results = engine
        .run_experiment(&experiment.parse().unwrap(), &mut |_, _| {
            std::ops::ControlFlow::Continue(())
        })
        .unwrap();
    let arm = results
        .arms
        .iter()
        .find(|arm| arm.arm.strategy.as_str() == strategy)
        .unwrap_or_else(|| panic!("{experiment} has no arm {strategy}"));
    let (initial, exchanges) = engine.history(arm.trials[0].end).unwrap();
    let start = usize::try_from(arm.start.step).unwrap();
    fn typed<T: serde::de::DeserializeOwned, W: serde::Serialize + ?Sized>(written: &W) -> T {
        serde_json::from_value(serde_json::to_value(written).unwrap()).unwrap()
    }
    let world: Economy<O> = typed(&initial);
    let before: Vec<axionomy::Exchange<O>> = typed(&exchanges[..start]);
    let after: Vec<axionomy::Exchange<O>> = typed(&exchanges[start..]);
    let world = world.replayed(&before.into_iter().collect()).unwrap();
    (world, after.into_iter().collect())
}
