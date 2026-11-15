use axionomy::{
    Account, Economy, EconomyBuilder, Exchange, Pattern, Quantity, Rate, Variable, basket, paths,
};
use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use std::hint::black_box;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, axionomy::Segments)]
enum Asset {
    Input,
    Output,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, axionomy::Segments)]
enum AccountId {
    Worker,
    Unrelated(u32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, axionomy::Segments)]
enum RateId {
    Convert,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, axionomy::Segments)]
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

/// `count` rates `visit/{i}`, each turning a token into the asset
/// `visited/{i}`: written one by one, or as one rate with a variable.
fn visits(count: u64, variable: bool) -> paths::Economy {
    let walker = || "walker".parse().unwrap();
    let builder = paths::EconomyBuilder::new().account(
        "walker".parse().unwrap(),
        paths::Account::from(basket([("token".parse().unwrap(), u64::MAX)])),
    );
    let builder = if variable {
        builder.rate_at(
            "visit/{i}".parse().unwrap(),
            paths::Rate::new()
                .variable(Variable::range("i".parse().unwrap(), 0, count - 1))
                .consume(walker(), basket([("token".parse().unwrap(), 1)]))
                .produce_at(
                    walker(),
                    [(
                        "visited/{i}".parse::<Pattern<_>>().unwrap(),
                        Quantity::new(1),
                    )],
                ),
        )
    } else {
        (0..count).fold(builder, |builder, i| {
            builder.rate(
                format!("visit/{i}").parse().unwrap(),
                paths::Rate::new()
                    .consume(walker(), basket([("token".parse().unwrap(), 1)]))
                    .produce(
                        walker(),
                        basket([(format!("visited/{i}").parse().unwrap(), 1)]),
                    ),
            )
        })
    };
    builder.build().unwrap()
}

/// What a rate with a variable costs beside the rates it stands for: building
/// the economy, and applying one instance.
fn variables(c: &mut Criterion) {
    for count in [10, 1_000] {
        let exchange = paths::Exchange::new(
            format!("visit/{}", count / 2).parse().unwrap(),
            Quantity::new(1),
        )
        .bind("walker".parse().unwrap(), "walker".parse().unwrap());
        for variable in [false, true] {
            let form = if variable { "variable" } else { "concrete" };
            c.bench_function(&format!("build_{count}_visits_{form}"), |bencher| {
                bencher.iter(|| black_box(visits(black_box(count), variable)));
            });
            let world = visits(count, variable);
            c.bench_function(
                &format!("fork_and_apply_one_of_{count}_visits_{form}"),
                |bencher| {
                    bencher.iter(|| {
                        let mut branch = black_box(&world).clone();
                        black_box(branch.apply(black_box(exchange.clone())).unwrap());
                    });
                },
            );
        }
    }
}

criterion_group!(benches, benchmark, variables);
criterion_main!(benches);
