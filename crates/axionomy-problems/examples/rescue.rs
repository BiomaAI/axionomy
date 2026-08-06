mod support;

use axionomy_problems::rescue::{self, Location, Policy};
use tracing::{debug, info};

fn main() {
    support::init(
        "Rescue",
        "Compare partially observed policies against an encoded stochastic prior.",
    );
    let model = rescue::uniform_uncertain();
    info!(
        accounts = model.accounts().count(),
        rates = model.rate_ids().count(),
        scenarios = 8,
        "uncertain economy ready"
    );
    let estimate = rescue::evaluate_scenarios(&model).expect("prior has positive weight");
    info!(
        samples = estimate.samples(),
        observe_then_follow_wins = estimate.observe_successes(),
        direct_north_wins = estimate.direct_successes(),
        chosen = ?estimate.chosen(),
        "exact encoded-scenario comparison complete"
    );
    let front = rescue::policy_front(&model, 64, 19).expect("policies can be sampled");
    info!(
        completeness = ?front.completeness(),
        retained_policies = front.len(),
        "sampled success/resource front estimated"
    );
    for entry in front.entries() {
        let dimensions = entry.payload().summary().dimensions();
        info!(
            policy = ?entry.payload().policy(),
            samples = dimensions[0].samples(),
            success = %format_args!("{:.1}%", dimensions[0].mean() * 100.0),
            sensor_use = %format_args!("{:.1}%", dimensions[1].mean() * 100.0),
            mean_spent_energy = %format_args!("{:.2}", dimensions[2].mean()),
            exact = false,
            "non-dominated sampled policy"
        );
    }
    let sample = rescue::instantiate(&model, Location::South, 1).expect("scenario is encoded");
    let rollout = rescue::run_sampled_policy(&model, &sample, estimate.chosen())
        .expect("scenario can be instantiated");
    let final_world = model
        .replayed(rollout.trace())
        .expect("the sampled policy must pass the core");
    assert!(final_world.matches(&rescue::goal()));
    assert_eq!(estimate.chosen(), Policy::ObserveThenFollow);

    info!(
        policy = ?estimate.chosen(),
        exchanges = rollout.trace().exchanges().len(),
        spent_energy = rollout.spent_energy(),
        used_sensor = rollout.used_sensor(),
        succeeded = rollout.succeeded(),
        public_intent = rollout
            .trace()
            .exchanges()
            .iter()
            .any(|exchange| exchange.rate() == &rescue::RateId::BeginObserve),
        nature_resolution = rollout.trace().exchanges().iter().any(|exchange| matches!(
            exchange.rate(),
            rescue::RateId::ResolveObservation { .. }
        )),
        goal_verified = true,
        "sampled scenario replayed"
    );
    debug!(scenario = ?sample, "encoded Nature instantiation");
    debug!(
        trace = ?rollout.trace().exchanges(),
        "accepted exchange trace"
    );
}
