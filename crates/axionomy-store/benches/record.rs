use axionomy::{
    Quantity, Rate, basket,
    path::Path,
    paths::{self, Paths},
};
use axionomy_store::{Checkpoints, Store};
use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use std::{hint::black_box, num::NonZeroU64};

fn path<K>(text: &str) -> Path<K> {
    text.parse().unwrap()
}

fn economy(accounts: u64) -> paths::Economy {
    let mut builder = paths::EconomyBuilder::new()
        .account(path("bank"), basket([(path("cash"), u64::MAX)]).into())
        .rate(
            path("pay"),
            Rate::new()
                .consume(path("from"), basket([(path("cash"), 1)]))
                .produce(path("to"), basket([(path("cash"), 1)])),
        );
    for account in 0..accounts {
        builder = builder.account(path(&format!("agents/{account}")), basket([]).into());
    }
    builder.build().unwrap()
}

fn pay(to: u64) -> paths::Exchange {
    paths::Exchange::new(path("pay"), Quantity::new(1))
        .bind(path("from"), path("bank"))
        .bind(path("to"), path(&format!("agents/{to}")))
}

/// One environment step: the kernel's apply alone, then recorded under each
/// checkpoint rule.
fn benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("step");
    for accounts in [1_000u64, 100_000] {
        let start = economy(accounts);
        let exchanges: Vec<_> = (0..1_024).map(|n| pay(n * 7_919 % accounts)).collect();
        group.bench_with_input(BenchmarkId::new("apply", accounts), &start, |b, start| {
            let mut economy = start.clone();
            let mut next = exchanges.iter().cycle();
            b.iter(|| black_box(economy.apply(next.next().unwrap().clone()).unwrap()));
        });
        for (rule, checkpoints) in [
            ("record_ends", Checkpoints::Ends),
            (
                "record_every_64",
                Checkpoints::Every(NonZeroU64::new(64).unwrap()),
            ),
            ("record_every_step", Checkpoints::EveryStep),
        ] {
            group.bench_with_input(BenchmarkId::new(rule, accounts), &start, |b, start| {
                let store = Store::memory().unwrap();
                let mut recorder = store.record::<Paths>(start.clone(), checkpoints);
                let mut next = exchanges.iter().cycle();
                b.iter(|| black_box(recorder.apply(next.next().unwrap().clone()).unwrap()));
                recorder.finish().unwrap();
            });
        }
    }
    group.finish();
}

criterion_group!(benches, benchmark);
criterion_main!(benches);
