//! Rounding against the caller, exercised through the kernel.
//!
//! `Axionomy.RoundingDiscipline` proves these laws over exact naturals. Here
//! the same share-vault laws run as real exchanges whose amounts are computed
//! by `MulDiv` expressions, with declared conservation invariants checked on
//! every commit.

use axionomy::{
    Account, Economy, EconomyBuilder, Exchange, LinearInvariant, Quantity, QuantityComparison,
    QuantityExpression as Q, Rate, Rounding, basket,
};
use proptest::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum Asset {
    Cash,
    Share,
    ShareSupply,
    VaultIdentity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum AccountId {
    Vault,
    User,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum Role {
    Vault,
    User,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum RateId {
    Deposit,
    Mint,
    Redeem,
    Withdraw,
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

type World = Economy<Test>;

fn assets() -> Q<Test> {
    Q::balance(Role::Vault, Asset::Cash)
}

fn supply() -> Q<Test> {
    Q::balance(Role::Vault, Asset::ShareSupply)
}

fn vault_rate() -> Rate<Test> {
    Rate::new()
        .preserve(Role::Vault, basket([(Asset::VaultIdentity, 1)]))
        .distinct(Role::Vault, Role::User)
}

fn world(total_assets: u64, total_shares: u64, user_cash: u64) -> World {
    // Deposit: pay `units` cash, receive ⌊units · shares / assets⌋ shares.
    let minted = Q::mul_div(Q::units(), supply(), assets(), Rounding::Floor);
    // Mint: receive `units` shares, pay ⌈units · assets / shares⌉ cash.
    let cost = Q::mul_div(Q::units(), assets(), supply(), Rounding::Ceil);
    // Redeem: return `units` shares, receive ⌊units · assets / shares⌋ cash.
    let paid = Q::mul_div(Q::units(), assets(), supply(), Rounding::Floor);
    // Withdraw: receive `units` cash, burn ⌈units · shares / assets⌉ shares.
    let burned = Q::mul_div(Q::units(), supply(), assets(), Rounding::Ceil);
    EconomyBuilder::new()
        .account(
            AccountId::Vault,
            Account::from(basket([
                (Asset::Cash, total_assets),
                (Asset::ShareSupply, total_shares),
                (Asset::VaultIdentity, 1),
            ])),
        )
        .account(
            AccountId::User,
            Account::from(basket([(Asset::Cash, user_cash)])),
        )
        .rate(
            RateId::Deposit,
            vault_rate()
                .consume(Role::User, basket([(Asset::Cash, 1)]))
                .produce(Role::Vault, basket([(Asset::Cash, 1)]))
                .produce_computed(Role::User, Asset::Share, minted.clone())
                .produce_computed(Role::Vault, Asset::ShareSupply, minted),
        )
        .rate(
            RateId::Mint,
            vault_rate()
                .produce(Role::User, basket([(Asset::Share, 1)]))
                .produce(Role::Vault, basket([(Asset::ShareSupply, 1)]))
                .consume_computed(Role::User, Asset::Cash, cost.clone())
                .produce_computed(Role::Vault, Asset::Cash, cost),
        )
        .rate(
            RateId::Redeem,
            vault_rate()
                .consume(Role::User, basket([(Asset::Share, 1)]))
                .consume(Role::Vault, basket([(Asset::ShareSupply, 1)]))
                .consume_computed(Role::Vault, Asset::Cash, paid.clone())
                .produce_computed(Role::User, Asset::Cash, paid.clone())
                .condition(
                    "pays out",
                    paid,
                    QuantityComparison::GreaterThanOrEqual,
                    Q::constant(0),
                ),
        )
        .rate(
            RateId::Withdraw,
            vault_rate()
                .consume(Role::Vault, basket([(Asset::Cash, 1)]))
                .produce(Role::User, basket([(Asset::Cash, 1)]))
                .consume_computed(Role::User, Asset::Share, burned.clone())
                .consume_computed(Role::Vault, Asset::ShareSupply, burned),
        )
        .invariant(LinearInvariant::new("cash").weight(Asset::Cash, 1))
        .invariant(
            LinearInvariant::new("shares")
                .weight(Asset::Share, 1)
                .weight(Asset::ShareSupply, -1),
        )
        .build()
        .unwrap()
}

fn exchange(rate: RateId, units: u64) -> Exchange<Test> {
    Exchange::new(rate, Quantity::new(units))
        .bind(Role::Vault, AccountId::Vault)
        .bind(Role::User, AccountId::User)
}

fn balance(world: &World, account: AccountId, asset: Asset) -> u64 {
    world.balance(&account, &asset).to_u64()
}

proptest! {
    #[test]
    fn deposit_then_redeem_never_profits(
        total_assets in 1_u64..1_000_000_000,
        total_shares in 1_u64..1_000_000_000,
        deposit in 1_u64..1_000_000,
    ) {
        let mut world = world(total_assets, total_shares, deposit);
        world.apply(exchange(RateId::Deposit, deposit)).unwrap();
        let shares = balance(&world, AccountId::User, Asset::Share);
        if shares > 0 {
            world.apply(exchange(RateId::Redeem, shares)).unwrap();
        }
        let returned = balance(&world, AccountId::User, Asset::Cash);
        prop_assert!(returned <= deposit, "returned {returned} > deposit {deposit}");
        // The vault never ends below its starting assets.
        prop_assert!(balance(&world, AccountId::Vault, Asset::Cash) >= total_assets);
    }

    #[test]
    fn mint_then_redeem_never_profits(
        total_assets in 1_u64..1_000_000_000,
        total_shares in 1_u64..1_000_000_000,
        shares in 1_u64..1_000_000,
    ) {
        let cost = (u128::from(shares) * u128::from(total_assets)).div_ceil(u128::from(total_shares));
        let cost = u64::try_from(cost).unwrap();
        let mut world = world(total_assets, total_shares, cost);
        world.apply(exchange(RateId::Mint, shares)).unwrap();
        prop_assert_eq!(balance(&world, AccountId::User, Asset::Cash), 0);
        world.apply(exchange(RateId::Redeem, shares)).unwrap();
        let returned = balance(&world, AccountId::User, Asset::Cash);
        prop_assert!(returned <= cost, "returned {returned} > paid {cost}");
    }

    #[test]
    fn withdraw_burns_at_least_the_exact_share_value(
        total_assets in 1_u64..1_000_000_000,
        total_shares in 1_u64..1_000_000_000,
        withdrawn in 1_u64..1_000_000,
    ) {
        let total_assets = total_assets.max(withdrawn);
        let ceil = |numerator: u64, factor: u64, denominator: u64| {
            u64::try_from((u128::from(numerator) * u128::from(factor)).div_ceil(u128::from(denominator)))
                .unwrap()
        };
        // Mint enough shares at the rounded-up price; this can only raise the
        // share price, so the later withdrawal is always covered.
        let needed = ceil(withdrawn, total_shares, total_assets);
        let mut world = world(total_assets, total_shares, ceil(needed, total_assets, total_shares));
        world.apply(exchange(RateId::Mint, needed)).unwrap();
        let assets = balance(&world, AccountId::Vault, Asset::Cash);
        let supply = balance(&world, AccountId::Vault, Asset::ShareSupply);
        let shares = balance(&world, AccountId::User, Asset::Share);
        world.apply(exchange(RateId::Withdraw, withdrawn)).unwrap();
        let burned = shares - balance(&world, AccountId::User, Asset::Share);
        prop_assert!(u128::from(withdrawn) * u128::from(supply) <= u128::from(burned) * u128::from(assets));
    }
}

#[test]
fn split_with_explicit_remainder_conserves_the_total() {
    // ⌊total · weight / total_weight⌋ to one party, the remainder to the other.
    for (total, weight, total_weight) in [
        (100_u64, 1_u64, 3_u64),
        (7, 2, 3),
        (0, 1, 2),
        (u64::MAX, 1, 3),
    ] {
        let first = Quantity::new(total)
            .checked_mul_div(
                Quantity::new(weight),
                Quantity::new(total_weight),
                Rounding::Floor,
            )
            .unwrap();
        let second = Quantity::new(total).checked_sub(first).unwrap();
        assert_eq!(first.checked_add(second), Some(Quantity::new(total)));
    }
}
