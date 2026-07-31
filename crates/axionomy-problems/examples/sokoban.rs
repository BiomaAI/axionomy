use axionomy_problems::sokoban;

fn main() {
    let initial = sokoban::initial();
    let solution = sokoban::solve(&initial).expect("Sokoban instance has a solution");
    let final_world = initial
        .replayed(solution.trace())
        .expect("the solver's proposal must pass the core");
    assert!(final_world.matches(&sokoban::goal()));

    println!(
        "Sokoban: solved in {} exchanges after expanding {} states",
        solution.trace().exchanges().len(),
        solution.expanded(),
    );
}
