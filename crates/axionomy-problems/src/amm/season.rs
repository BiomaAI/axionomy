//! Repeated production, consumption and investment over the same exact AMM.
//! A period closes only after every actor ends trading. Future needs, defaults,
//! construction and production limits are balances, not runner-owned state.

use super::*;
use axionomy::path;

/// How many days the season lasts, `{season-length}`: a constant.
#[cfg(test)]
fn season_length() -> Param<u64> {
    Param::new(path!("season-length"))
}
fn weather(seed: u64, day: u8) -> u64 {
    if axionomy_search::sampling::splitmix(seed, u64::from(day)).is_multiple_of(3) {
        40
    } else {
        100
    }
}

pub(super) fn build(size: MarketSize, days: u8, seed: u64) -> World {
    let reference = match size {
        MarketSize::Showcase => axionomy::path!("amm/showcase"),
        MarketSize::Stress => axionomy::path!("amm/stress"),
    };
    crate::fixtures::variant(reference, |instance| {
        let day = Param::<u8>::new(path!("day"));
        let calendar = Relation::each_giving(
            &day,
            [path!("sunlight"), path!("shortage")],
            (0..days - 1).map(|d| {
                (
                    d,
                    [
                        weather(seed, d + 1),
                        u64::from(d + 2 < days && weather(seed, d + 2) < 100),
                    ],
                )
            }),
        );
        let advance = RateIdPattern::AdvanceDay(day.var()).into();
        instance
            .tables
            .get_mut(&advance)
            .expect("declared market calendar")
            .relations = vec![calendar];
        crate::fixtures::set(
            instance,
            AccountId::World,
            [(Asset::CurrentSunlight, weather(seed, 0))],
        );
        crate::fixtures::set(
            instance,
            AccountId::Information,
            [(Asset::ShortageSignal, u64::from(weather(seed, 1) < 100))],
        );
        crate::fixtures::set(
            instance,
            AccountId::Actor(Actor::Generator),
            [(
                Asset::ProductionAllowance,
                800 * size.scale() * weather(seed, 0) / 100,
            )],
        );
    })
}

pub fn market_goal(world: &World) -> Goal<Ontology> {
    if world.account(&AccountId::World).is_some() {
        Goal::new().require(AccountId::World, basket([(Asset::MarketEnded, 1)]))
    } else {
        goal()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn period(world: &World) -> u64 {
        world.balance(&AccountId::World, &Asset::Day).to_u64()
    }
    fn balance(world: &World, actor: Actor, asset: Asset) -> u64 {
        world.balance(&AccountId::Actor(actor), &asset).to_u64()
    }
    fn basic(rate: RateId, role: Role, actor: Actor) -> Action {
        Exchange::new(rate, Quantity::new(1)).bind(role, AccountId::Actor(actor))
    }
    fn event(day: u8, last: bool) -> Action {
        let mut action = Exchange::new(
            if last {
                RateId::CloseMarket
            } else {
                RateId::AdvanceDay(day)
            },
            Quantity::new(1),
        )
        .bind(Role::World, AccountId::World)
        .bind(Role::Pool, AccountId::Pool);
        for actor in ACTORS {
            action = action.bind(Role::Participant(actor), AccountId::Actor(actor));
        }
        if !last {
            action = action.bind(Role::Information, AccountId::Information);
        }
        action
    }
    fn commission() -> Action {
        basic(
            RateId::CommissionProduction,
            Role::Producer,
            Actor::Generator,
        )
        .bind(Role::World, AccountId::World)
    }
    fn invest() -> Action {
        basic(RateId::InvestProduction, Role::Producer, Actor::Generator)
            .bind(Role::World, AccountId::World)
            .bind(Role::Treasury, AccountId::Treasury)
    }
    fn fund(actor: Actor, shares: u64) -> Action {
        Exchange::new(RateId::FundLiquidity, Quantity::new(shares))
            .bind(Role::Pool, AccountId::Pool)
            .bind(Role::LiquidityProvider, AccountId::Actor(actor))
    }
    fn end_day(world: &mut World) {
        for actor in ACTORS {
            if balance(world, actor, Asset::Trading) > 0 {
                world
                    .apply(basic(RateId::EndTrading(actor), Role::Consumer, actor))
                    .unwrap();
            }
        }
        let day = period(world) as u8;
        let days = world
            .constants()
            .find(|(name, _)| *name == season_length().name())
            .and_then(|(_, days)| days.number())
            .expect("a season has a length");
        let last = u64::from(day + 1) == days;
        world.apply(event(day, last)).unwrap();
    }
    fn rejects_without_change(world: &mut World, action: Action) {
        let before = world.state_key();
        assert!(world.apply(action).is_err());
        assert_eq!(world.state_key(), before);
    }

    #[test]
    fn daily_entitlements_expire_and_refresh_only_at_an_authorized_boundary() {
        let mut world = initial_showcase();
        rejects_without_change(&mut world, event(0, false));
        let allowance = balance(&world, Actor::Factory, Asset::Decisions);
        for _ in 0..allowance {
            world.apply(buy_energy(Actor::Factory, 100, 1)).unwrap();
        }
        rejects_without_change(&mut world, buy_energy(Actor::Factory, 100, 1));
        end_day(&mut world);
        assert_eq!(period(&world), 1);
        assert_eq!(
            balance(&world, Actor::Factory, Asset::MissedNeed(Actor::Factory)),
            1
        );
        assert_eq!(
            balance(
                &world,
                Actor::Generator,
                Asset::DefaultedObligation(Actor::Generator)
            ),
            1
        );
        assert_eq!(
            balance(&world, Actor::Factory, Asset::Need(Actor::Factory)),
            1
        );
        assert_eq!(
            balance(&world, Actor::Factory, Asset::FutureNeed(Actor::Factory)),
            6
        );
        world.apply(buy_energy(Actor::Factory, 100, 1)).unwrap();
        rejects_without_change(&mut world, event(0, false));
        while !world.matches(&market_goal(&world)) {
            end_day(&mut world);
        }
        assert_eq!(
            balance(&world, Actor::Factory, Asset::MissedNeed(Actor::Factory)),
            8
        );
        rejects_without_change(&mut world, buy_energy(Actor::Factory, 100, 1));
        rejects_without_change(&mut world, event(7, true));
    }

    #[test]
    fn storage_and_daily_production_limits_are_enforced_by_the_economy() {
        let mut world = initial_showcase();
        rejects_without_change(&mut world, buy_energy(Actor::Household, 20_000, 1));
        world
            .apply(sell_energy(Actor::Generator, 1_000, 1))
            .unwrap();
        let allowance = balance(&world, Actor::Generator, Asset::ProductionAllowance);
        rejects_without_change(&mut world, produce_energy(Actor::Generator, allowance + 1));
        world
            .apply(produce_energy(Actor::Generator, allowance))
            .unwrap();
        rejects_without_change(&mut world, produce_energy(Actor::Generator, 1));
        rejects_without_change(
            &mut world,
            basic(RateId::DeliverGoods, Role::Producer, Actor::Generator)
                .bind(Role::Treasury, AccountId::Treasury),
        );
    }

    #[test]
    fn investment_spends_credit_now_and_adds_capacity_after_two_days() {
        let mut world = initial_showcase();
        rejects_without_change(&mut world, invest());
        world
            .apply(sell_energy(Actor::Generator, 1_000, 1))
            .unwrap();
        let before = balance(&world, Actor::Generator, Asset::Credit);
        world.apply(invest()).unwrap();
        assert_eq!(
            balance(&world, Actor::Generator, Asset::Credit),
            before - 6_000
        );
        assert_eq!(
            balance(&world, Actor::Generator, Asset::ProductionCapacity),
            800
        );
        rejects_without_change(&mut world, commission());
        end_day(&mut world);
        rejects_without_change(&mut world, commission());
        end_day(&mut world);
        world.apply(commission()).unwrap();
        assert_eq!(
            balance(&world, Actor::Generator, Asset::ProductionCapacity),
            1_200
        );
        assert_eq!(
            balance(&world, Actor::Generator, Asset::ProductionAllowance),
            1_200
                * world
                    .balance(&AccountId::World, &Asset::CurrentSunlight)
                    .to_u64()
                / 100
        );
        rejects_without_change(&mut world, commission());
    }

    #[test]
    fn liquidity_can_follow_changed_prices_with_at_most_one_atom_rounding_per_reserve() {
        let mut world = initial_showcase();
        world.apply(buy_energy(Actor::Factory, 10_000, 1)).unwrap();
        let before = pool_state(&world);
        world.apply(fund(Actor::AdaptiveLp, 50)).unwrap();
        let after = pool_state(&world);
        assert_eq!(after.issued_lp_shares - before.issued_lp_shares, 50);
        assert_eq!(
            after.energy - before.energy,
            (before.energy * 50).div_ceil(before.issued_lp_shares)
        );
        assert_eq!(
            after.credit - before.credit,
            (before.credit * 50).div_ceil(before.issued_lp_shares)
        );
        rejects_without_change(&mut world, fund(Actor::AdaptiveLp, 50));
        rejects_without_change(&mut world, remove_liquidity(Actor::AdaptiveLp, 50));
    }
}
