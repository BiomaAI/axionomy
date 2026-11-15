#![doc = include_str!("README.md")]

use axionomy::path;
use axionomy::{
    Account, ApplyError, Economy, EconomyBuilder, Exchange, Goal, LinearInvariant, Pattern,
    Quantity, Rate, basket,
};
#[cfg(feature = "packages")]
pub mod package;
mod production;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum AccountId {
    Workshop,
    Customer(u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum Asset {
    Wood,
    Credit,
    Frame,
    Panel,
    Offcut,
    Table,
    Durability,
    Wear,
    ToolPotential,
    RepairKit,
    UsedRepairKit,
    UpgradeKit,
    Upgraded,
    Idle,
    Contract(u8),
    Raw(u8),
    Prepared(u8),
    Assembled(u8),
    Fulfilled(u8),
    Completed,
    Labor,
    SpentLabor,
    Tool,
    Chair,
    Waste,
    Time,
    SpentTime,
    Active,
    Solved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum Role {
    Shop,
    Customer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, axionomy::Identifier)]
pub enum RateId {
    Accept(u8),
    Prepare {
        order: u8,
        precise: bool,
        upgraded: bool,
    },
    Assemble(u8),
    Deliver(u8),
    Repair,
    Upgrade,
    Recycle,
    BasicChair,
    EfficientBatch,
    CounterfeitChair,
    Finish,
}

pub type World = Economy<Ontology>;

/// The identifier types of [`World`], named `workshop` where economies are
/// stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Ontology {}

impl axionomy::Ontology for Ontology {
    const NAME: &'static str = "workshop";
    type Account = AccountId;
    type Asset = Asset;
    type Rate = RateId;
    type Role = Role;
}
pub type Action = Exchange<Ontology>;
pub type Failure = ApplyError<Ontology>;

pub fn initial() -> World {
    build(6, 4, 3, 2)
}

/// Requires a multi-batch production plan with visible time/waste tradeoffs.
pub fn initial_showcase() -> World {
    production::build(false)
}

pub fn initial_stress() -> World {
    production::build(true)
}

/// A law of the workshop, its shop role fixed to the workshop's account.
fn law() -> Rate<Ontology> {
    Rate::new().account(Role::Shop, Pattern::literal(AccountId::Workshop))
}

fn build(wood: u64, labor: u64, time: u64, target: u64) -> World {
    EconomyBuilder::new()
        .account(
            AccountId::Workshop,
            Account::from(basket([
                (Asset::Wood, wood),
                (Asset::Labor, labor),
                (Asset::Tool, 1),
                (Asset::Time, time),
                (Asset::Active, 1),
            ])),
        )
        .rate(
            RateId::BasicChair,
            law()
                .preserve(Role::Shop, basket([(Asset::Active, 1)]))
                .consume(
                    Role::Shop,
                    basket([(Asset::Wood, 2), (Asset::Labor, 1), (Asset::Time, 1)]),
                )
                .preserve(Role::Shop, basket([(Asset::Tool, 1)]))
                .produce(
                    Role::Shop,
                    basket([
                        (Asset::Chair, 1),
                        (Asset::Waste, 1),
                        (Asset::SpentLabor, 1),
                        (Asset::SpentTime, 1),
                    ]),
                ),
        )
        .rate(
            RateId::EfficientBatch,
            law()
                .preserve(Role::Shop, basket([(Asset::Active, 1)]))
                .consume(
                    Role::Shop,
                    basket([(Asset::Wood, 3), (Asset::Labor, 2), (Asset::Time, 3)]),
                )
                .preserve(Role::Shop, basket([(Asset::Tool, 1)]))
                .produce(
                    Role::Shop,
                    basket([
                        (Asset::Chair, 2),
                        (Asset::Waste, 1),
                        (Asset::SpentLabor, 2),
                        (Asset::SpentTime, 3),
                    ]),
                ),
        )
        // This deliberately malformed domain proposal is installed to prove
        // that declared invariants, rather than solver discipline, are final.
        .rate(
            RateId::CounterfeitChair,
            law()
                .preserve(Role::Shop, basket([(Asset::Active, 1)]))
                .consume(Role::Shop, basket([(Asset::Wood, 1), (Asset::Time, 1)]))
                .produce(
                    Role::Shop,
                    basket([(Asset::Chair, 2), (Asset::SpentTime, 1)]),
                ),
        )
        .rate(
            RateId::Finish,
            law()
                .preserve(Role::Shop, basket([(Asset::Chair, target)]))
                .consume(Role::Shop, basket([(Asset::Active, 1)]))
                .produce(Role::Shop, basket([(Asset::Solved, 1)])),
        )
        .invariant(
            LinearInvariant::new(path!("material-mass"))
                .weight(Asset::Wood, 1)
                .weight(Asset::Chair, 1)
                .weight(Asset::Waste, 1),
        )
        .invariant(
            LinearInvariant::new(path!("labor-accounting"))
                .weight(Asset::Labor, 1)
                .weight(Asset::SpentLabor, 1),
        )
        .invariant(
            LinearInvariant::new(path!("time-accounting"))
                .weight(Asset::Time, 1)
                .weight(Asset::SpentTime, 1),
        )
        .invariant(
            LinearInvariant::new(path!("workshop-lifecycle"))
                .weight(Asset::Active, 1)
                .weight(Asset::Solved, 1),
        )
        .build()
        .expect("workshop model is valid")
}

pub fn goal() -> Goal<Ontology> {
    Goal::new().require(AccountId::Workshop, basket([(Asset::Solved, 1)]))
}

/// The exchange applying `rate` once in the workshop, a delivery to the
/// order's own customer.
pub fn action(rate: RateId) -> Action {
    let action = Exchange::new(rate, Quantity::new(1)).bind(Role::Shop, AccountId::Workshop);
    if let RateId::Deliver(order) = rate {
        action.bind(Role::Customer, AccountId::Customer(order))
    } else {
        action
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_efficient_batch_wastes_less_and_two_basic_chairs_take_less_time() {
        for (recipes, waste, time) in [
            (&[RateId::EfficientBatch, RateId::Finish][..], 1, 3),
            (
                &[RateId::BasicChair, RateId::BasicChair, RateId::Finish],
                2,
                2,
            ),
        ] {
            let made = initial()
                .replayed(&recipes.iter().copied().map(action).collect())
                .expect("the recipes replay");
            assert!(made.matches(&goal()));
            let held = |asset| made.balance(&AccountId::Workshop, &asset).to_u64();
            assert_eq!(held(Asset::Waste), waste);
            assert_eq!(held(Asset::SpentTime), time);
            // A finished shift has no recipe left.
            assert!(
                made.rate_ids()
                    .filter_map(|rate| rate.as_literal())
                    .all(|rate| !made.is_applicable(&action(*rate)))
            );
        }
    }

    #[test]
    fn invariant_rejects_a_stoichiometrically_invalid_rate_atomically() {
        let mut world = initial();
        let before = world.state_key();
        let error = world
            .apply(action(RateId::CounterfeitChair))
            .expect_err("mass creation must be rejected");
        assert!(matches!(
            error,
            ApplyError::InvariantViolation {
                invariant,
                before,
                after,
            } if invariant.as_str() == "material-mass" && before.to_string() == "6" && after.to_string() == "7"
        ));
        assert_eq!(world.state_key(), before);
    }
}
