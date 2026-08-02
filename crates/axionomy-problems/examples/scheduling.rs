mod support;

use axionomy_problems::scheduling;
use tracing::{debug, info};

fn main() {
    support::init(
        "Job-shop scheduling",
        "Compare best-first search with a separate bounded branch optimizer.",
    );
    let initial = scheduling::initial();
    info!(
        accounts = initial.accounts().count(),
        rates = initial.rate_ids().count(),
        "encoded economy ready"
    );
    let generic = scheduling::solve_best_first(&initial).expect("schedule is feasible");
    let specialized = scheduling::branch_optimize(&initial).expect("schedule is feasible");
    let generic_world = initial
        .replayed(generic.trace())
        .expect("best-first search must pass the core");
    let specialized_world = initial
        .replayed(specialized.trace())
        .expect("the optimizer's proposal must pass the core");

    assert!(generic_world.matches(&scheduling::goal()));
    assert!(specialized_world.matches(&scheduling::goal()));
    assert_eq!(generic.cost(), u64::from(specialized.makespan()));

    info!(
        strategy = "best-first",
        exchanges = generic.trace().exchanges().len(),
        expanded = generic.expanded(),
        makespan = generic.cost(),
        goal_verified = true,
        "proposal replayed"
    );
    info!(
        strategy = "bounded optimizer",
        exchanges = specialized.trace().exchanges().len(),
        makespan = specialized.makespan(),
        goal_verified = true,
        "proposal replayed"
    );
    debug!(
        strategy = "best-first",
        trace = ?generic.trace().exchanges(),
        "accepted exchange trace"
    );
    debug!(
        strategy = "bounded optimizer",
        trace = ?specialized.trace().exchanges(),
        "accepted exchange trace"
    );
}
