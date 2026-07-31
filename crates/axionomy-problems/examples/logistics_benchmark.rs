use axionomy_problems::logistics::{self, Policy};
use std::time::Instant;

const ROLLOUTS: u64 = 256;

fn main() {
    let model = logistics::initial();
    let started = Instant::now();
    let exchanges = (0..ROLLOUTS)
        .map(|seed| logistics::run_policy(&model, Policy::Reliable, seed).steps())
        .sum::<usize>();
    let elapsed = started.elapsed();

    println!(
        "Logistics benchmark: {ROLLOUTS} rollouts, {exchanges} exchanges, {:.0} rollouts/s",
        ROLLOUTS as f64 / elapsed.as_secs_f64(),
    );
}
