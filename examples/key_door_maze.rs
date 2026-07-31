use axionomy::problems::maze;

fn main() {
    let initial = maze::initial();
    let shallow = maze::solve_bfs(&initial).expect("maze has a path");
    let cheapest = maze::solve_astar(&initial).expect("maze has a path");

    println!(
        "BFS: {} exchanges; A*: {} energy across {} exchanges",
        shallow.trace().exchanges().len(),
        cheapest.cost(),
        cheapest.trace().exchanges().len(),
    );

    let replayed = initial
        .replayed(cheapest.trace())
        .expect("the solver's proposal must pass the core");
    assert!(replayed.matches(&maze::goal()));
}
