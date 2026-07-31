use axionomy_problems::scheduling;

fn main() {
    let initial = scheduling::initial();
    let generic = scheduling::solve_best_first(&initial).expect("schedule is feasible");
    let specialized = scheduling::independent_optimize(&initial).expect("schedule is feasible");
    let final_world = initial
        .replayed(specialized.trace())
        .expect("the optimizer's proposal must pass the core");
    assert!(final_world.matches(&scheduling::goal()));
    assert_eq!(generic.cost(), u64::from(specialized.makespan()));

    println!(
        "Scheduling: both optimizers found makespan {} in {} exchanges",
        specialized.makespan(),
        specialized.trace().exchanges().len(),
    );
}
