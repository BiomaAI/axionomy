mod support;

use axionomy_problems::logistics::{self, Policy};
use tracing::{debug, info};

fn main() {
    support::init(
        "Logistics",
        "Compare route policies across recurrent weather and breakdown outcomes.",
    );
    let model = logistics::initial();
    info!(
        accounts = model.accounts().count(),
        rates = model.rate_ids().count(),
        orders = 4,
        "encoded economy ready"
    );
    let estimate = logistics::monte_carlo(&model, 64).expect("policies can be evaluated");
    for policy in [Policy::Direct, Policy::Reliable] {
        let summary = estimate
            .estimate(policy)
            .expect("both logistics policies are evaluated");
        info!(
            policy = ?policy,
            samples = summary.samples(),
            mean_utility = summary.mean(),
            variance = summary.variance(),
            worst_utility = summary.minimum().unwrap_or_default(),
            "Monte Carlo estimate"
        );
    }
    let chosen = estimate.chosen();
    let rollout = logistics::run_policy(&model, chosen, 7);
    let replayed = model
        .replayed(rollout.trace())
        .expect("selected mission must replay");

    assert!(rollout.completed());
    assert!(replayed.matches(&logistics::goal()));
    assert_eq!(chosen, Policy::Reliable);

    info!(
        policy = ?chosen,
        delivered = rollout.delivered(),
        encoded_time = rollout.elapsed_time(),
        exchanges = rollout.steps(),
        goal_verified = true,
        "selected policy replayed"
    );
    debug!(
        trace = ?rollout.trace().exchanges(),
        "accepted exchange trace"
    );
}
