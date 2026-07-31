use axionomy_problems::connect_four;

fn main() {
    let initial = connect_four::initial();
    let trace = connect_four::play_game(128, 5);
    let final_world = initial
        .replayed(&trace)
        .expect("MCTS game must replay through the core");
    let values = connect_four::terminal_values(&final_world).expect("game must terminate");

    println!(
        "Connect Four: {} exchanges, terminal values {values:?}",
        trace.exchanges().len(),
    );
}
