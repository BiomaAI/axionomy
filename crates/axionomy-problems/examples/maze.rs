use axionomy_problems::maze;

fn main() {
    let initial = maze::initial();
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

    println!(
        "Maze: BFS {} exchanges; A* {} energy across {} exchanges",
        shallow.trace().exchanges().len(),
        cheapest.cost(),
        cheapest.trace().exchanges().len(),
    );
}
