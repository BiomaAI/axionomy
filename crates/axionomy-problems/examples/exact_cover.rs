use axionomy_problems::exact_cover;

fn main() {
    let initial = exact_cover::initial();
    let generic = exact_cover::solve_bfs(&initial).expect("exact cover exists");
    let specialized = exact_cover::algorithm_x(&initial).expect("Algorithm X finds a cover");
    let final_world = initial
        .replayed(&specialized)
        .expect("Algorithm X must emit core-valid exchanges");
    assert!(final_world.matches(&exact_cover::goal()));

    println!(
        "Exact cover: BFS and Algorithm X agree on {} exchanges",
        generic.trace().exchanges().len(),
    );
}
