use axionomy_problems::workshop;

fn main() {
    let initial = workshop::initial();
    let solution = workshop::minimize_waste(&initial).expect("two chairs can be produced");
    let final_world = initial
        .replayed(solution.trace())
        .expect("the optimized proposal must pass the core");
    assert!(final_world.matches(&workshop::goal()));

    println!(
        "Workshop: produced the goal with {} waste in {} exchanges",
        workshop::waste(&final_world),
        solution.trace().exchanges().len(),
    );
}
