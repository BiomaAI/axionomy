mod support;

use axionomy_problems::amm::{self, Scenario};
use tracing::{debug, info};

fn main() {
    support::init(
        "The Living Market",
        "Discover the only public price in a closed economy through exact AMM exchanges.",
    );
    let initial = amm::initial_showcase();
    let opening = amm::pool_state(&initial);
    info!(
        energy = opening.energy,
        credit = opening.credit,
        price_milli = opening.price_milli,
        actors = amm::ACTORS.len(),
        "founding liquidity establishes a price hypothesis"
    );

    let trace = amm::trace(&initial, Scenario::MarketDay);
    let final_world = initial
        .replayed(&trace)
        .expect("the canonical market day must replay");
    let closing = amm::pool_state(&final_world);
    assert!(final_world.matches(&amm::goal()));
    info!(
        exchanges = trace.exchanges().len(),
        closing_price_milli = closing.price_milli,
        product = %closing.product,
        goal_verified = true,
        "closed market discovered a new exchange value"
    );

    for (actor, contribution) in amm::direct_price_contributions(&initial, &trace) {
        info!(
            ?actor,
            price_milli_delta = contribution,
            "direct price contribution"
        );
    }
    for contribution in amm::shapley_price_contributions(&initial) {
        info!(
            actor = ?contribution.actor,
            numerator = %contribution.numerator,
            denominator = contribution.denominator,
            "exact counterfactual Shapley contribution"
        );
    }
    debug!(exchanges = ?trace.exchanges(), "verified market replay");
}
