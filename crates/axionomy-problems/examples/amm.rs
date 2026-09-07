mod support;

use axionomy_problems::amm::{self, Actor, Asset, Scenario};
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

    let source = initial.state_key();
    for scenario in [Scenario::NoWhale, Scenario::ThinLiquidity] {
        let alternative_trace = amm::trace(&initial, scenario);
        let alternative = initial
            .replayed(&alternative_trace)
            .expect("counterfactual must replay");
        assert!(alternative.matches(&amm::goal()));
        let pool = amm::pool_state(&alternative);
        info!(
            ?scenario,
            opening_price_milli = opening.price_milli,
            closing_price_milli = pool.price_milli,
            difference_from_market_day =
                i128::from(pool.price_milli) - i128::from(closing.price_milli),
            remaining_energy = pool.energy,
            exchanges = alternative_trace.exchanges().len(),
            replay_verified = true,
            "change one market condition and replay the consequences"
        );
    }
    assert_eq!(initial.state_key(), source);

    let quote = amm::quote_output(&initial, Asset::Credit, 10_000).expect("opening quote exists");
    let guarded = amm::buy_energy(Actor::Factory, 10_000, quote + 1);
    let assessment = initial.assess(&guarded);
    assert!(!assessment.is_applicable());
    let mut rejected_branch = initial.fork();
    assert!(rejected_branch.apply(guarded).is_err());
    assert_eq!(rejected_branch.state_key(), source);
    info!(
        quoted_energy = quote,
        requested_minimum = quote + 1,
        rejected = true,
        all_accounts_unchanged = true,
        "a funded trader cannot override the curve; failed settlement moves nothing"
    );
    debug!(?assessment, "minimum-output rejection evidence");

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
