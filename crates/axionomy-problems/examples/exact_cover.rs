mod support;

use axionomy_problems::exact_cover;
use tracing::{debug, info};

fn main() {
    support::init(
        "Exact cover",
        "Compare generic graph search with Algorithm X over identical encoded constraints.",
    );
    let initial = exact_cover::initial();
    info!(
        accounts = initial.accounts().count(),
        rates = initial.rate_ids().count(),
        "encoded economy ready"
    );
    let generic = exact_cover::solve_bfs(&initial).expect("exact cover exists");
    let specialized = exact_cover::algorithm_x(&initial).expect("Algorithm X finds a cover");
    let generic_world = initial
        .replayed(generic.trace())
        .expect("BFS must emit core-valid exchanges");
    let specialized_world = initial
        .replayed(&specialized)
        .expect("Algorithm X must emit core-valid exchanges");

    assert!(generic_world.matches(&exact_cover::goal()));
    assert!(specialized_world.matches(&exact_cover::goal()));

    info!(
        strategy = "BFS",
        exchanges = generic.trace().exchanges().len(),
        expanded = generic.expanded(),
        goal_verified = true,
        "proposal replayed"
    );
    info!(
        strategy = "Algorithm X",
        exchanges = specialized.exchanges().len(),
        goal_verified = true,
        "proposal replayed"
    );
    debug!(
        strategy = "BFS",
        trace = ?generic.trace().exchanges(),
        "accepted exchange trace"
    );
    debug!(
        strategy = "Algorithm X",
        trace = ?specialized.exchanges(),
        "accepted exchange trace"
    );
}
