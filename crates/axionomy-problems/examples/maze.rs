mod support;

use axionomy_problems::maze;
use tracing::{debug, info};

fn main() {
    support::init(
        "Maze",
        "Compare shortest-depth BFS with energy-aware A* over one encoded graph.",
    );
    let initial = maze::initial();
    info!(
        accounts = initial.accounts().count(),
        rates = initial.rate_ids().count(),
        "encoded economy ready"
    );
    let shallow = maze::solve_bfs(&initial).expect("maze has a path");
    let cheapest = maze::solve_astar(&initial).expect("maze has a path");
    let shallow_world = initial
        .replayed(shallow.trace())
        .expect("the BFS proposal must pass the core");
    let cheapest_world = initial
        .replayed(cheapest.trace())
        .expect("the A* proposal must pass the core");

    assert!(shallow_world.matches(&maze::goal()));
    assert!(cheapest_world.matches(&maze::goal()));

    info!(
        strategy = "BFS",
        exchanges = shallow.trace().exchanges().len(),
        expanded = shallow.expanded(),
        goal_verified = true,
        "proposal replayed"
    );
    info!(
        strategy = "A*",
        exchanges = cheapest.trace().exchanges().len(),
        expanded = cheapest.expanded(),
        energy = cheapest.cost(),
        goal_verified = true,
        "proposal replayed"
    );
    debug!(
        strategy = "BFS",
        trace = ?shallow.trace().exchanges(),
        "accepted exchange trace"
    );
    debug!(
        strategy = "A*",
        trace = ?cheapest.trace().exchanges(),
        "accepted exchange trace"
    );

    let pareto = maze::pareto_front(&initial).expect("objective schema is valid");
    info!(
        completeness = ?pareto.front().completeness(),
        outcomes = pareto.front().len(),
        terminal_outcomes = pareto.progress().terminal_outcomes(),
        expanded = pareto.progress().expanded(),
        "exact search retained every non-dominated route"
    );
    for entry in pareto.front().entries() {
        let outcome = initial
            .replayed(entry.payload())
            .expect("Pareto trace must replay");
        info!(
            energy = maze::spent_energy(&outcome),
            time = maze::spent_time(&outcome),
            exchanges = entry.payload().exchanges().len(),
            replay_verified = true,
            "non-dominated route"
        );
    }
}
