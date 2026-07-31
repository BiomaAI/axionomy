use axionomy_problems::mission;
use axionomy_search::{mcts::MctsConfig, rl::replay_transitions};

fn main() {
    let model = mission::initial();
    let actual =
        mission::instantiate(&model, 3).expect("the encoded prior contains the selected scenario");
    let decision = mission::plan_initial(&actual, MctsConfig::new(1_024, 12).with_seed(29))
        .expect("the initial mission information set can be searched");
    assert_eq!(decision.action().rate(), &mission::RateId::BeginScan);
    assert!(actual.is_applicable(decision.action()));

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
        "Mission: ISMCTS chose {:?} from {} information sets; {:?} won {}/{} samples; {} learning transitions; {} encoded time",
        decision.action().rate(),
        decision.information_sets(),
        estimate.chosen(),
        estimate.coordinated_successes(),
        estimate.samples(),
        learning_trajectory.len(),
        rollout.elapsed_time(),
    );
}
