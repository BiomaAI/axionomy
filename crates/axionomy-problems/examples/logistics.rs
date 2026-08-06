mod support;

use axionomy_problems::logistics::{self, OrderId, Policy, RateId, RiskCriterion};
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
            lower_decile_utility = summary.lower_tail_mean(0.1).unwrap_or_default(),
            "Monte Carlo estimate"
        );
    }
    let tail_choice = logistics::monte_carlo_with_risk(&model, 64, RiskCriterion::LowerDecile)
        .expect("tail risk can be evaluated");
    info!(
        mean_choice = ?estimate.chosen(),
        lower_decile_choice = ?tail_choice.chosen(),
        "caller-selected risk criteria compared"
    );
    let front = logistics::policy_front(&model, 64).expect("policies can be sampled");
    info!(
        completeness = ?front.completeness(),
        retained_policies = front.len(),
        "sampled multi-objective policy front estimated"
    );
    for entry in front.entries() {
        let dimensions = entry.payload().summary().dimensions();
        info!(
            policy = ?entry.payload().policy(),
            samples = dimensions[0].samples(),
            completion = %format_args!("{:.1}%", dimensions[0].mean() * 100.0),
            mean_delivered = %format_args!("{:.2}", dimensions[1].mean()),
            mean_elapsed_time = %format_args!("{:.2}", dimensions[2].mean()),
            exact = false,
            "non-dominated sampled policy"
        );
    }

    let mut loaded = model.fork();
    let load = logistics::candidates(&loaded)
        .into_iter()
        .find(|exchange| exchange.rate() == &RateId::Load(OrderId::A))
        .expect("first package can be loaded");
    loaded.apply(load).expect("load applies");
    let planned = logistics::plan_action(&loaded, 64, 31).expect("route can be planned");
    info!(
        action = ?planned.action().rate(),
        iterations = planned.iterations(),
        root_actions = planned.children().len(),
        live_applicable = loaded.is_applicable(planned.action()),
        "MCTS planned a route from encoded chance"
    );
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
