use axionomy_problems::exact_cover;

fn main() {
    let initial = exact_cover::initial();
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

    println!(
        "Exact cover: BFS {} exchanges; Algorithm X {} exchanges; both core-valid",
        generic.trace().exchanges().len(),
        specialized.exchanges().len(),
    );
}
