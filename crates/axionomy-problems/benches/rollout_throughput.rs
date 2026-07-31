use axionomy_problems::logistics::{self, Policy};
use std::hint::black_box;
use std::time::Instant;

const ROLLOUTS: u64 = 256;

fn main() {
    let model = logistics::initial();
    let started = Instant::now();
    let exchanges = (0..ROLLOUTS)
        .map(|seed| {
            let rollout =
                logistics::run_policy(black_box(&model), Policy::Reliable, black_box(seed));
            assert!(rollout.completed());
            rollout.steps()
        })
        .sum::<usize>();
    let elapsed = started.elapsed();

    println!(
        "Rollout throughput: {ROLLOUTS} logistics rollouts, {exchanges} exchanges, \
         {:.0} rollouts/s",
        ROLLOUTS as f64 / elapsed.as_secs_f64(),
    );
}
