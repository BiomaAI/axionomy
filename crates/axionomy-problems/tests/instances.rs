//! Each sample's larger instances are economies of their own: Stress is not
//! Showcase under another name.

#[test]
fn formerly_aliased_stress_models_are_structurally_distinct() {
    use axionomy_problems::{bridge, exact_cover, marketplace, maze, rescue, sokoban};

    let maze_showcase = maze::initial_showcase();
    let maze_stress = maze::initial_stress();
    assert_ne!(maze_showcase.state_key(), maze_stress.state_key());
    assert_ne!(
        maze_showcase.rate_ids().count(),
        maze_stress.rate_ids().count()
    );

    let cover_showcase = exact_cover::initial_showcase();
    let cover_stress = exact_cover::initial_stress();
    assert_ne!(cover_showcase.state_key(), cover_stress.state_key());
    assert_ne!(
        cover_showcase.rate_ids().count(),
        cover_stress.rate_ids().count()
    );

    let warehouse_showcase = sokoban::initial_showcase();
    let warehouse_stress = sokoban::initial_stress();
    // The latches a warehouse has: the instances of its latch rate.
    let latches = |world: &sokoban::World| -> usize {
        world
            .rate_ids()
            .filter(|key| {
                matches!(
                    world.instances_of(key).next(),
                    Some(sokoban::RateId::Latch { .. })
                )
            })
            .map(|key| world.instances_of(key).count())
            .sum()
    };
    assert_eq!(latches(&warehouse_showcase), 1);
    assert_eq!(latches(&warehouse_stress), 2);
    assert!(
        warehouse_showcase
            .accounts()
            .any(|(_, account)| account.balance(&sokoban::Asset::ExitCell).to_u64() == 1)
    );

    let bridge_showcase = bridge::initial_showcase();
    let bridge_stress = bridge::initial_stress();
    assert_ne!(bridge_showcase.state_key(), bridge_stress.state_key());

    let market_showcase = marketplace::initial_showcase();
    let market_stress = marketplace::initial_stress();
    assert_ne!(market_showcase.state_key(), market_stress.state_key());
    // Their laws are written alike; Stress's answer to more orders.
    let market_instances = |world: &marketplace::World| -> usize {
        world
            .rate_ids()
            .map(|key| world.instances_of(key).count())
            .sum()
    };
    assert_ne!(
        market_instances(&market_showcase),
        market_instances(&market_stress)
    );

    let rescue_showcase = rescue::uniform_uncertain_showcase();
    let rescue_stress = rescue::uniform_uncertain_stress();
    assert_ne!(rescue_showcase.state_key(), rescue_stress.state_key());
    // Their rates are written alike; Stress's answer to more scenarios.
    let instances = |world: &rescue::World| -> usize {
        world
            .rate_ids()
            .map(|key| world.instances_of(key).count())
            .sum()
    };
    assert_ne!(instances(&rescue_showcase), instances(&rescue_stress));
}
