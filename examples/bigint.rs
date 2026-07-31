use axionomy::{Account, Basket, EconomyBuilder, Exchange, LinearInvariant, Quantity, Rate};
use num_bigint::BigUint;
use tracing::info;
use tracing_subscriber::EnvFilter;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum Asset {
    Raw,
    Finished,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum AccountId {
    Workshop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum RateId {
    Build,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum Role {
    Workshop,
}

fn quantity(value: impl Into<BigUint>) -> Quantity<BigUint> {
    Quantity::try_from_scalar(value.into()).expect("BigUint is non-negative")
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_target(false)
        .without_time()
        .compact()
        .init();

    let initial = BigUint::from(u64::MAX) + BigUint::from(2_u8);
    let mut economy = EconomyBuilder::new()
        .account(
            AccountId::Workshop,
            Account::from(Basket::from([(Asset::Raw, quantity(initial.clone()))])),
        )
        .rate(
            RateId::Build,
            Rate::new()
                .consume(Role::Workshop, Basket::from([(Asset::Raw, quantity(1_u8))]))
                .produce(
                    Role::Workshop,
                    Basket::from([(Asset::Finished, quantity(1_u8))]),
                ),
        )
        .invariant(
            LinearInvariant::new("material")
                .weight(Asset::Raw, 1)
                .weight(Asset::Finished, 1),
        )
        .build()?;

    info!(
        backend = "BigUint",
        initial_raw = %initial,
        "constructed an economy beyond the u64 range"
    );

    let exchange =
        Exchange::new(RateId::Build, quantity(2_u8)).bind(Role::Workshop, AccountId::Workshop);
    let assessment = economy.assess(&exchange);
    info!(
        applicable = assessment.is_applicable(),
        projected_accounts = assessment.projected_deltas().map_or(0, <[_]>::len),
        "assessed the exact exchange"
    );

    let receipt = economy.apply(exchange)?;
    let raw = economy.balance(&AccountId::Workshop, &Asset::Raw);
    let finished = economy.balance(&AccountId::Workshop, &Asset::Finished);
    info!(
        affected_accounts = receipt.deltas().len(),
        remaining_raw = %raw,
        finished = %finished,
        "committed the exchange atomically"
    );

    assert_eq!(raw, quantity(initial - BigUint::from(2_u8)));
    assert_eq!(finished, quantity(2_u8));
    Ok(())
}
