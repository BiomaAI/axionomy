use axionomy::{
    Account, ApplyError, Economy, EconomyBuilder, Exchange, Goal, LinearInvariant, Quantity, Rate,
    Trace, basket,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum Asset {
    Input,
    Catalyst,
    Output,
    Capacity,
    UsedCapacity,
    Solved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum AccountId {
    Source,
    Machine,
    Sink,
    Goal,
    Missing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum Role {
    Source,
    Machine,
    Sink,
    Goal,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum RateId {
    Transform,
    Finish,
    Overflow,
    Invalid,
    Missing,
}

type World = Economy<AccountId, Asset, RateId, Role>;
type Action = Exchange<RateId, Role, AccountId>;

fn world() -> World {
    EconomyBuilder::new()
        .account(
            AccountId::Source,
            Account::from(basket([(Asset::Input, 4)])),
        )
        .account(
            AccountId::Machine,
            Account::from(basket([(Asset::Catalyst, 1), (Asset::Capacity, 2)])),
        )
        .account(AccountId::Sink, Account::default())
        .account(AccountId::Goal, Account::default())
        .rate(
            RateId::Transform,
            Rate::new()
                .consume(Role::Source, basket([(Asset::Input, 2)]))
                .preserve(Role::Machine, basket([(Asset::Catalyst, 1)]))
                .consume(Role::Machine, basket([(Asset::Capacity, 1)]))
                .produce(Role::Machine, basket([(Asset::UsedCapacity, 1)]))
                .produce(Role::Sink, basket([(Asset::Output, 1)]))
                .distinct(Role::Source, Role::Machine)
                .distinct(Role::Machine, Role::Sink),
        )
        .rate(
            RateId::Finish,
            Rate::new()
                .preserve(Role::Sink, basket([(Asset::Output, 2)]))
                .produce(Role::Goal, basket([(Asset::Solved, 1)]))
                .distinct(Role::Sink, Role::Goal),
        )
        .rate(
            RateId::Invalid,
            Rate::new()
                .consume(Role::Source, basket([(Asset::Input, 1)]))
                .produce(Role::Sink, basket([(Asset::Output, 2)])),
        )
        .invariant(
            LinearInvariant::new("input-output units")
                .weight(Asset::Input, 1)
                .weight(Asset::Output, 2),
        )
        .invariant(
            LinearInvariant::new("capacity units")
                .weight(Asset::Capacity, 1)
                .weight(Asset::UsedCapacity, 1),
        )
        .build()
}

fn transform(units: u64) -> Action {
    Exchange::new(RateId::Transform, Quantity::new(units))
        .bind(Role::Source, AccountId::Source)
        .bind(Role::Machine, AccountId::Machine)
        .bind(Role::Sink, AccountId::Sink)
}

fn finish() -> Action {
    Exchange::new(RateId::Finish, Quantity::new(1))
        .bind(Role::Sink, AccountId::Sink)
        .bind(Role::Goal, AccountId::Goal)
}

#[test]
fn one_exchange_atomically_rewrites_three_accounts() {
    let mut world = world();
    let receipt = world.apply(transform(2)).expect("exchange is feasible");

    assert_eq!(
        world.balance(&AccountId::Source, &Asset::Input),
        Quantity::ZERO
    );
    assert_eq!(
        world.balance(&AccountId::Machine, &Asset::Catalyst),
        Quantity::new(1)
    );
    assert_eq!(
        world.balance(&AccountId::Machine, &Asset::Capacity),
        Quantity::ZERO
    );
    assert_eq!(
        world.balance(&AccountId::Machine, &Asset::UsedCapacity),
        Quantity::new(2)
    );
    assert_eq!(
        world.balance(&AccountId::Sink, &Asset::Output),
        Quantity::new(2)
    );
    assert_eq!(receipt.deltas().len(), 3);
}

#[test]
fn failed_multi_account_exchange_changes_nothing() {
    let mut world = world();
    let before = world.state_key();
    let error = world
        .apply(transform(3))
        .expect_err("capacity and input are insufficient");

    assert!(matches!(
        error,
        ApplyError::InsufficientBalance {
            account: AccountId::Machine,
            ..
        } | ApplyError::InsufficientBalance {
            account: AccountId::Source,
            ..
        }
    ));
    assert_eq!(world.state_key(), before);
}

#[test]
fn bindings_and_invariants_are_core_validated() {
    let mut world = world();
    let before = world.state_key();

    let missing = Exchange::new(RateId::Transform, Quantity::new(1))
        .bind(Role::Source, AccountId::Source)
        .bind(Role::Machine, AccountId::Machine);
    assert!(matches!(
        world.apply(missing),
        Err(ApplyError::MissingBinding { role: Role::Sink })
    ));

    let unknown = transform(1).bind(Role::Unknown, AccountId::Sink);
    assert!(matches!(
        world.apply(unknown),
        Err(ApplyError::UnknownBinding {
            role: Role::Unknown
        })
    ));

    let invalid = Exchange::new(RateId::Invalid, Quantity::new(1))
        .bind(Role::Source, AccountId::Source)
        .bind(Role::Sink, AccountId::Sink);
    assert!(matches!(
        world.apply(invalid),
        Err(ApplyError::InvariantViolation { .. })
    ));
    assert_eq!(world.state_key(), before);
}

#[test]
fn forks_and_replay_validate_a_trace_without_touching_the_source() {
    let world = world();
    let before = world.state_key();
    let mut trace = Trace::new();
    trace.push(transform(2));
    trace.push(finish());

    let replayed = world.replayed(&trace).expect("trace must be valid");
    let goal = Goal::new().require(AccountId::Goal, basket([(Asset::Solved, 1)]));
    assert!(replayed.matches(&goal));
    assert_eq!(world.state_key(), before);

    let (simulated, _) = world.simulate(transform(1)).expect("fork is feasible");
    assert_ne!(simulated.state_key(), world.state_key());
    assert_eq!(world.state_key(), before);
}

#[test]
fn invalid_identifiers_units_and_role_aliasing_are_structured_errors() {
    let mut world = world();

    assert!(matches!(
        world.apply(Exchange::new(RateId::Missing, Quantity::new(1))),
        Err(ApplyError::MissingRate {
            rate: RateId::Missing
        })
    ));
    assert!(matches!(
        world.apply(transform(0)),
        Err(ApplyError::ZeroUnits)
    ));
    let aliased = Exchange::new(RateId::Transform, Quantity::new(1))
        .bind(Role::Source, AccountId::Source)
        .bind(Role::Machine, AccountId::Source)
        .bind(Role::Sink, AccountId::Sink);
    assert!(matches!(
        world.apply(aliased),
        Err(ApplyError::RolesMustDiffer { .. })
    ));
    let missing_account = Exchange::new(RateId::Finish, Quantity::new(1))
        .bind(Role::Sink, AccountId::Missing)
        .bind(Role::Goal, AccountId::Goal);
    assert!(matches!(
        world.apply(missing_account),
        Err(ApplyError::MissingAccount {
            account: AccountId::Missing
        })
    ));
}

#[test]
fn checked_arithmetic_failures_are_atomic() {
    let mut rate_overflow = EconomyBuilder::new()
        .account(
            AccountId::Source,
            Account::from(basket([(Asset::Input, u64::MAX)])),
        )
        .rate(
            RateId::Overflow,
            Rate::new().consume(Role::Source, basket([(Asset::Input, u64::MAX)])),
        )
        .build();
    let before = rate_overflow.state_key();
    let action =
        Exchange::new(RateId::Overflow, Quantity::new(2)).bind(Role::Source, AccountId::Source);
    assert!(matches!(
        rate_overflow.apply(action),
        Err(ApplyError::RateOverflow {
            asset: Asset::Input,
            ..
        })
    ));
    assert_eq!(rate_overflow.state_key(), before);

    let mut balance_overflow = EconomyBuilder::new()
        .account(
            AccountId::Sink,
            Account::from(basket([(Asset::Output, u64::MAX)])),
        )
        .rate(
            RateId::Overflow,
            Rate::new().produce(Role::Sink, basket([(Asset::Output, 1)])),
        )
        .build();
    let before = balance_overflow.state_key();
    let action =
        Exchange::new(RateId::Overflow, Quantity::new(1)).bind(Role::Sink, AccountId::Sink);
    assert!(matches!(
        balance_overflow.apply(action),
        Err(ApplyError::BalanceOverflow {
            account: AccountId::Sink,
            asset: Asset::Output,
        })
    ));
    assert_eq!(balance_overflow.state_key(), before);
}
