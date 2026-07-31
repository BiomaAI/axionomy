use axionomy::{
    Account, AccountDelta, ApplyError, AssessmentStatus, Economy, EconomyBuilder, Exchange,
    ExchangeAssessment, Goal, LinearInvariant, Quantity, Rate, Trace, basket,
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

fn assert_deltas_eq(
    left: &[AccountDelta<AccountId, Asset>],
    right: &[AccountDelta<AccountId, Asset>],
) {
    assert_eq!(left.len(), right.len());
    for (left, right) in left.iter().zip(right) {
        assert_eq!(left.account(), right.account());
        assert_eq!(left.consumed(), right.consumed());
        assert_eq!(left.produced(), right.produced());
        assert_eq!(left.preserved(), right.preserved());
    }
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

    let ApplyError::Infeasible { shortfalls } = error else {
        panic!("expected complete infeasibility report");
    };
    assert_eq!(shortfalls.len(), 2);
    assert_eq!(shortfalls[0].account(), &AccountId::Source);
    assert_eq!(shortfalls[0].missing(), &basket([(Asset::Input, 2)]));
    assert_eq!(shortfalls[1].account(), &AccountId::Machine);
    assert_eq!(shortfalls[1].missing(), &basket([(Asset::Capacity, 1)]));
    assert_eq!(world.state_key(), before);
}

#[test]
fn assessment_explains_complete_multi_account_distance_without_mutation() {
    let world = world();
    let before = world.state_key();
    let assessment = world.assess(&transform(3));

    assert_eq!(assessment.status(), AssessmentStatus::Infeasible);
    assert!(!assessment.is_applicable());
    assert_eq!(assessment.accounts().len(), 3);
    assert!(assessment.projected_deltas().is_none());
    assert!(assessment.issues().is_empty());

    let source = assessment
        .account(&AccountId::Source)
        .expect("source account is assessed");
    assert_eq!(source.required(), &basket([(Asset::Input, 6)]));
    assert_eq!(source.available(), &basket([(Asset::Input, 4)]));

    let machine = assessment
        .account(&AccountId::Machine)
        .expect("machine account is assessed");
    assert_eq!(
        machine.required(),
        &basket([(Asset::Catalyst, 1), (Asset::Capacity, 3)])
    );
    assert_eq!(
        machine.available(),
        &basket([(Asset::Catalyst, 1), (Asset::Capacity, 2)])
    );

    let shortfalls = assessment.shortfalls();
    assert_eq!(shortfalls.len(), 2);
    assert_eq!(shortfalls[0].account(), &AccountId::Source);
    assert_eq!(shortfalls[0].missing(), &basket([(Asset::Input, 2)]));
    assert_eq!(shortfalls[1].account(), &AccountId::Machine);
    assert_eq!(shortfalls[1].missing(), &basket([(Asset::Capacity, 1)]));
    assert_eq!(
        assessment.shortfall(&AccountId::Source),
        Some(&basket([(Asset::Input, 2)]))
    );
    assert_eq!(assessment.shortfall(&AccountId::Sink), None);
    assert_eq!(world.state_key(), before);
}

#[test]
fn applicable_assessment_projects_exact_receipt_deltas() {
    let mut world = world();
    let action = transform(2);
    let assessment = world.assess(&action);

    assert_eq!(assessment.status(), AssessmentStatus::Applicable);
    assert!(assessment.is_applicable());
    assert_eq!(assessment.accounts().len(), 3);
    assert!(assessment.shortfalls().is_empty());
    assert!(assessment.issues().is_empty());

    let projected = assessment
        .projected_deltas()
        .expect("applicable assessment projects deltas")
        .to_vec();
    let receipt = world.apply(action).expect("assessed exchange applies");

    assert_deltas_eq(&projected, receipt.deltas());
}

#[test]
fn applicability_conveniences_derive_from_assessment() {
    let world = world();
    let feasible = transform(2);
    let infeasible = transform(3);

    assert!(world.is_applicable(&feasible));
    assert!(!world.is_applicable(&infeasible));
    assert_eq!(
        world.applicable([feasible.clone(), infeasible]),
        vec![feasible]
    );
}

#[test]
fn invalid_assessment_collects_structural_issues() {
    let world = world();
    let malformed =
        Exchange::new(RateId::Transform, Quantity::ZERO).bind(Role::Source, AccountId::Source);
    let assessment = world.assess(&malformed);

    assert_eq!(assessment.status(), AssessmentStatus::Invalid);
    assert!(!assessment.is_applicable());
    assert!(assessment.accounts().is_empty());
    assert!(assessment.shortfalls().is_empty());
    assert!(assessment.projected_deltas().is_none());
    assert_eq!(assessment.issues().len(), 3);
    assert!(matches!(assessment.issues()[0], ApplyError::ZeroUnits));
    assert!(matches!(
        assessment.issues()[1],
        ApplyError::MissingBinding {
            role: Role::Machine
        }
    ));
    assert!(matches!(
        assessment.issues()[2],
        ApplyError::MissingBinding { role: Role::Sink }
    ));
    assert!(matches!(assessment, ExchangeAssessment::Invalid { .. }));
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
    let assessment = world.assess(&invalid);
    assert_eq!(assessment.status(), AssessmentStatus::Invalid);
    assert!(matches!(
        assessment.issues(),
        [ApplyError::InvariantViolation { .. }]
    ));
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
    let before_fingerprint = world.state_fingerprint();
    let mut trace = Trace::new();
    trace.push(transform(2));
    trace.push(finish());

    let replayed = world.replayed(&trace).expect("trace must be valid");
    let goal = Goal::new().require(AccountId::Goal, basket([(Asset::Solved, 1)]));
    assert!(replayed.matches(&goal));
    assert_eq!(world.state_key(), before);

    let (simulated, _) = world.simulate(transform(1)).expect("fork is feasible");
    assert_ne!(simulated.state_key(), world.state_key());
    assert_ne!(simulated.state_fingerprint(), before_fingerprint);
    assert_eq!(world.state_key(), before);
    assert_eq!(world.state_fingerprint(), before_fingerprint);
}

#[test]
fn economic_views_have_canonical_observation_identities() {
    let world = world();
    let source_and_sink = world.view([AccountId::Source, AccountId::Sink]);
    let source_only = world.view([AccountId::Source]);

    assert_eq!(
        source_and_sink.observation_key(),
        vec![(AccountId::Source, Asset::Input, Quantity::new(4))]
    );
    assert_eq!(
        source_and_sink.observation_key(),
        source_only.observation_key(),
        "empty visible accounts do not add hidden or synthetic state"
    );
    assert!(
        source_and_sink
            .observation_key()
            .iter()
            .all(|(account, _, _)| account != &AccountId::Machine)
    );
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
