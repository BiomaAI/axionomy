mod support;

use axionomy_problems::mission;
use axionomy_search::{mcts::MctsConfig, rl::replay_transitions};
use tracing::{debug, info};

fn main() {
    support::init(
        "Mission",
        "Plan under hidden truth, compare policies, and project replay into RL transitions.",
    );
    let model = mission::initial();
    info!(
        accounts = model.accounts().count(),
        rates = model.rate_ids().count(),
        encoded_scenarios = 16,
        "uncertain economy ready"
    );
    let actual =
        mission::instantiate(&model, 3).expect("the encoded prior contains the selected scenario");
    let beliefs = mission::initial_beliefs(&model);
    let observation = mission::scout_information(&actual);
    info!(
        actor = observation.actor(),
        visible_accounts = observation.key().visible_accounts().len(),
        visible_balances = observation.key().balances().len(),
        "Scout information state derived; hidden Nature account excluded"
    );
    let decision = mission::plan(&actual, &beliefs, MctsConfig::new(1_024, 12).with_seed(29))
        .expect("the initial mission information set can be searched");
    assert_eq!(decision.action().rate(), &mission::RateId::BeginScan);
    assert!(actual.is_applicable(decision.action()));
    info!(
        action = ?decision.action().rate(),
        iterations = decision.iterations(),
        information_sets = decision.information_sets(),
        root_actions = decision.children().len(),
        live_applicable = true,
        "ISMCTS decision selected and revalidated"
    );
    debug!(children = ?decision.children(), "root action statistics");

    let mut observed_actual = actual.fork();
    observed_actual
        .apply(decision.action().clone())
        .expect("public scan intent applies");
    let response = mission::required_nature_response(&observed_actual)
        .expect("Nature has an encoded scan response");
    observed_actual
        .apply(response)
        .expect("encoded Nature response applies");
    let updated_information = mission::scout_information(&observed_actual);
    let posterior = mission::update_beliefs(&beliefs, decision.action(), &updated_information);
    let follow_up = mission::plan(
        &observed_actual,
        &posterior,
        MctsConfig::new(1_024, 12).with_seed(37),
    )
    .expect("the observed mission can be replanned");
    info!(
        prior_worlds = beliefs.len(),
        posterior_worlds = posterior.len(),
        follow_up = ?follow_up.action().rate(),
        live_applicable = observed_actual.is_applicable(follow_up.action()),
        "caller conditioned beliefs and replanned after observation"
    );

    let estimate = mission::evaluate_scenarios(&model).expect("mission policies can be evaluated");
    info!(
        samples = estimate.samples(),
        coordinated_wins = estimate.coordinated_successes(),
        direct_north_wins = estimate.direct_successes(),
        chosen = ?estimate.chosen(),
        "exact encoded-scenario comparison complete"
    );
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

    info!(
        policy = ?estimate.chosen(),
        exchanges = rollout.trace().exchanges().len(),
        encoded_time = rollout.elapsed_time(),
        learning_transitions = learning_trajectory.len(),
        goal_verified = true,
        "coordinated mission replayed and projected"
    );
    debug!(
        trace = ?rollout.trace().exchanges(),
        "accepted exchange trace"
    );
}
