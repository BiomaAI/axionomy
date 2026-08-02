use axionomy::{Account, EconomyBuilder, Exchange, Quantity, Rate, basket};
use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum Asset {
    Input,
    Output,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum AccountId {
    Worker,
    Unrelated(u32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum RateId {
    Convert,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum Role {
    Worker,
}

fn benchmark(c: &mut Criterion) {
    let world = EconomyBuilder::new()
        .account(
            AccountId::Worker,
            Account::from(basket([(Asset::Input, 1_000_000)])),
        )
        .rate(
            RateId::Convert,
            Rate::new()
                .consume(Role::Worker, basket([(Asset::Input, 1)]))
                .produce(Role::Worker, basket([(Asset::Output, 1)])),
        )
        .build()
        .unwrap();
    let action =
        Exchange::new(RateId::Convert, Quantity::new(1)).bind(Role::Worker, AccountId::Worker);

    c.bench_function("assess_single_account_exchange", |bencher| {
        bencher.iter(|| black_box(&world).assess(black_box(&action)));
    });
    c.bench_function("fork_and_apply_single_account_exchange", |bencher| {
        bencher.iter(|| {
            let mut branch = black_box(&world).fork();
            black_box(branch.apply(black_box(action.clone())).unwrap());
        });
    });

    let mut builder = EconomyBuilder::new().account(
        AccountId::Worker,
        Account::from(basket([(Asset::Input, u64::MAX)])),
    );
    for id in 0..10_000 {
        builder = builder.account(AccountId::Unrelated(id), Account::default());
    }
    let mut large_world = builder
        .rate(
            RateId::Convert,
            Rate::new()
                .consume(Role::Worker, basket([(Asset::Input, 1)]))
                .produce(Role::Worker, basket([(Asset::Output, 1)])),
        )
        .build()
        .unwrap();
    c.bench_function("apply_one_of_10_001_accounts", |bencher| {
        bencher.iter(|| {
            black_box(
                large_world
                    .apply(black_box(action.clone()))
                    .expect("the worker retains ample input"),
            );
        });
    });
}

criterion_group!(benches, benchmark);
criterion_main!(benches);
