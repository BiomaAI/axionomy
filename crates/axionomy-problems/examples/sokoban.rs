mod support;

use axionomy_problems::sokoban;
use tracing::{debug, info};

fn main() {
    support::init(
        "Sokoban",
        "Solve a spatial push puzzle through atomic, multi-account exchanges.",
    );
    let initial = sokoban::initial();
    info!(
        accounts = initial.accounts().count(),
        rates = initial.rate_ids().count(),
        "encoded economy ready"
    );
    let solution = sokoban::solve(&initial).expect("Sokoban instance has a solution");
    let final_world = initial
        .replayed(solution.trace())
        .expect("the solver's proposal must pass the core");
    assert!(final_world.matches(&sokoban::goal()));

    info!(
        strategy = "BFS",
        exchanges = solution.trace().exchanges().len(),
        expanded = solution.expanded(),
        goal_verified = true,
        "proposal replayed"
    );
    debug!(
        trace = ?solution.trace().exchanges(),
        "accepted exchange trace"
    );
}
