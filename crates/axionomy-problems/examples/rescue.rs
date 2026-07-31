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
    let estimate = rescue::monte_carlo(&model, 8).expect("prior has positive weight");
    info!(
        samples = estimate.samples(),
        observe_then_follow_wins = estimate.observe_successes(),
        direct_north_wins = estimate.direct_successes(),
        chosen = ?estimate.chosen(),
        "Monte Carlo policy comparison complete"
    );
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
        succeeded = rollout.succeeded(),
        goal_verified = true,
        "sampled scenario replayed"
    );
    debug!(scenario = ?sample, "encoded Nature instantiation");
    debug!(
        trace = ?rollout.trace().exchanges(),
        "accepted exchange trace"
    );
}
