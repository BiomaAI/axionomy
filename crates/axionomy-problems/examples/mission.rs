use axionomy_problems::mission;
use axionomy_search::rl::replay_transitions;

fn main() {
    let model = mission::initial();
    let estimate = mission::monte_carlo(&model, 16).expect("mission policies can be evaluated");
    let rollout = mission::run_policy(&model, estimate.chosen(), 3);
    let replayed = model
        .replayed(rollout.trace())
        .expect("coordinated mission must replay");

    assert!(rollout.succeeded());
    assert!(replayed.matches(&mission::goal()));
    let learning_trajectory = replay_transitions(
        &model,
        rollout.trace(),
        |world| {
            [
                world
                    .balance(
                        &mission::AccountId::Agent(mission::AgentId::Medic),
                        &mission::Asset::Intel(mission::Location::North),
                    )
                    .get(),
                world
                    .balance(
                        &mission::AccountId::Agent(mission::AgentId::Medic),
                        &mission::Asset::Intel(mission::Location::South),
                    )
                    .get(),
            ]
        },
        |world| {
            [
                world
                    .balance(&mission::AccountId::Success, &mission::Asset::Solved)
                    .get(),
                world
                    .balance(&mission::AccountId::Mission, &mission::Asset::ElapsedTime)
                    .get(),
            ]
        },
        |world| world.matches(&mission::goal()),
    )
    .expect("mission trace becomes learning transitions");

    println!(
        "Mission: {:?} won {}/{} samples; {} learning transitions; {} encoded time",
        estimate.chosen(),
        estimate.coordinated_successes(),
        estimate.samples(),
        learning_trajectory.len(),
        rollout.elapsed_time(),
    );
}
