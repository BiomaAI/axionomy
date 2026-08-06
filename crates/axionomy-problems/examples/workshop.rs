mod support;

use axionomy_problems::workshop;
use tracing::{debug, info};

fn main() {
    support::init(
        "Workshop",
        "Minimize encoded waste while preserving catalysts and material invariants.",
    );
    let initial = workshop::initial();
    info!(
        accounts = initial.accounts().count(),
        rates = initial.rate_ids().count(),
        "encoded economy ready"
    );
    let solution = workshop::minimize_waste(&initial).expect("two chairs can be produced");
    let final_world = initial
        .replayed(solution.trace())
        .expect("the optimized proposal must pass the core");
    assert!(final_world.matches(&workshop::goal()));

    info!(
        strategy = "best-first",
        exchanges = solution.trace().exchanges().len(),
        expanded = solution.expanded(),
        waste = workshop::waste(&final_world),
        goal_verified = true,
        "proposal replayed"
    );
    debug!(
        trace = ?solution.trace().exchanges(),
        "accepted exchange trace"
    );

    let pareto = workshop::pareto_front(&initial).expect("objective schema is valid");
    info!(
        completeness = ?pareto.front().completeness(),
        outcomes = pareto.front().len(),
        "exact search retained fast and low-waste recipes"
    );
    for entry in pareto.front().entries() {
        let outcome = initial
            .replayed(entry.payload())
            .expect("Pareto trace must replay");
        info!(
            waste = workshop::waste(&outcome),
            process_time = workshop::spent_time(&outcome),
            replay_verified = true,
            "non-dominated production plan"
        );
    }
}
