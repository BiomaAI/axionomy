mod support;

use axionomy_problems::connect_four;
use tracing::{debug, info};

fn main() {
    support::init(
        "Connect Four",
        "Play a complete encoded game using adversarial vector-valued MCTS.",
    );
    let initial = connect_four::initial();
    let iterations_per_move = 128;
    let seed = 5;
    info!(
        accounts = initial.accounts().count(),
        rates = initial.rate_ids().count(),
        iterations_per_move,
        seed,
        "encoded game ready"
    );
    let trace = connect_four::play_game(iterations_per_move, seed);
    let final_world = initial
        .replayed(&trace)
        .expect("MCTS game must replay through the core");
    let values = connect_four::terminal_values(&final_world).expect("game must terminate");

    info!(
        exchanges = trace.exchanges().len(),
        terminal_values = ?values,
        replay_verified = true,
        "complete game replayed"
    );
    debug!(trace = ?trace.exchanges(), "accepted exchange trace");
}
