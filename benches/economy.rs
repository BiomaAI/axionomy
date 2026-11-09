use axionomy::{Account, Economy, EconomyBuilder, Exchange, Quantity, Rate, basket};
use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Test {}

impl axionomy::Ontology for Test {
    const NAME: &'static str = "test";
    type Account = AccountId;
    type Asset = Asset;
    type Rate = RateId;
    type Role = Role;
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
            let mut branch = black_box(&world).clone();
            black_box(branch.apply(black_box(action.clone())).unwrap());
        });
    });

    let mut large_world = world_with_unrelated_accounts(10_000);
    c.bench_function("apply_one_of_10_001_accounts", |bencher| {
        bencher.iter(|| {
            black_box(
                large_world
                    .apply(black_box(action.clone()))
                    .expect("the worker retains ample input"),
            );
        });
    });

    // A fork shares every account with its source, so its cost must not grow
    // with the number of accounts.
    let mut forks = c.benchmark_group("fork_and_apply_by_unrelated_accounts");
    for unrelated in [10, 1_000, 100_000] {
        let world = world_with_unrelated_accounts(unrelated);
        forks.bench_with_input(
            BenchmarkId::from_parameter(unrelated),
            &world,
            |bencher, world| {
                bencher.iter(|| {
                    let mut branch = world.clone();
                    black_box(branch.apply(black_box(action.clone())).unwrap());
                });
            },
        );
    }
    forks.finish();
}

fn world_with_unrelated_accounts(unrelated: u32) -> Economy<Test> {
    let mut builder = EconomyBuilder::new().account(
        AccountId::Worker,
        Account::from(basket([(Asset::Input, u64::MAX)])),
    );
    for id in 0..unrelated {
        builder = builder.account(AccountId::Unrelated(id), Account::default());
    }
    builder
        .rate(
            RateId::Convert,
            Rate::new()
                .consume(Role::Worker, basket([(Asset::Input, 1)]))
                .produce(Role::Worker, basket([(Asset::Output, 1)])),
        )
        .build()
        .unwrap()
}

criterion_group!(benches, benchmark);
criterion_main!(benches);
