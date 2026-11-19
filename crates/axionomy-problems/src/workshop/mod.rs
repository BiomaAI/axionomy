#![doc = include_str!("README.md")]

use axionomy::{ApplyError, Economy, Exchange, Goal, Quantity, basket};
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
    crate::fixtures::initial(axionomy::path!("workshop/micro"))
}

/// Requires a multi-batch production plan with visible time/waste tradeoffs.
pub fn initial_showcase() -> World {
    crate::fixtures::initial(axionomy::path!("workshop/showcase"))
}

pub fn initial_stress() -> World {
    crate::fixtures::initial(axionomy::path!("workshop/stress"))
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
