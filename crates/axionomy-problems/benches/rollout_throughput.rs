use axionomy_problems::logistics::{self, Policy};
use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;

fn benchmark(c: &mut Criterion) {
    let model = logistics::initial();
    let mut seed = 0_u64;

    c.bench_function("logistics_reliable_policy_rollout", |bencher| {
        bencher.iter(|| {
            let rollout =
                logistics::run_policy(black_box(&model), Policy::Reliable, black_box(seed));
            seed = seed.wrapping_add(1);
            assert!(rollout.completed());
            black_box(rollout.steps())
        });
    });
}

criterion_group!(benches, benchmark);
criterion_main!(benches);
