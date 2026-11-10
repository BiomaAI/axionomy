use axionomy::path;
use axionomy::{
    Account, AccountDelta, ApplyError, AssessmentStatus, Basket, Economy, EconomyBuilder, Exchange,
    ExchangeAssessment, Goal, LinearInvariant, Quantity, Rate, ReplayError, Trace, basket,
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
type Action = Exchange<Test>;

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
            LinearInvariant::new(path!("input-output-units"))
                .weight(Asset::Input, 1)
                .weight(Asset::Output, 2),
        )
        .invariant(
            LinearInvariant::new(path!("capacity-units"))
                .weight(Asset::Capacity, 1)
                .weight(Asset::UsedCapacity, 1),
        )
        .build()
        .expect("test model is valid")
}

fn transform(units: u64) -> Action {
    Exchange::new(RateId::Transform, Quantity::new(units))
        .bind(Role::Source, AccountId::Source)
        .bind(Role::Machine, AccountId::Machine)
        .bind(Role::Sink, AccountId::Sink)
}

#[test]
fn model_and_goal_expose_all_referenced_asset_keys() {
    let world = world();
    assert_eq!(
        world.asset_keys().into_iter().copied().collect::<Vec<_>>(),
        vec![
            Asset::Input,
            Asset::Catalyst,
            Asset::Output,
            Asset::Capacity,
            Asset::UsedCapacity,
            Asset::Solved,
        ]
    );

    let goal = Goal::<Test>::new().require(AccountId::Goal, basket([(Asset::Solved, 1)]));
    assert_eq!(goal.asset_keys(), [&Asset::Solved].into_iter().collect());
}

fn finish() -> Action {
    Exchange::new(RateId::Finish, Quantity::new(1))
        .bind(Role::Sink, AccountId::Sink)
        .bind(Role::Goal, AccountId::Goal)
}

fn assert_deltas_eq(left: &[AccountDelta<Test>], right: &[AccountDelta<Test>]) {
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
    let before = world.state_key();
    let smaller = transform(1);
    let feasible = transform(2);
    let infeasible = transform(3);

    assert!(world.is_applicable(&feasible));
    assert!(!world.is_applicable(&infeasible));
    assert_eq!(
        world.applicable([
            smaller.clone(),
            infeasible,
            feasible.clone(),
            smaller.clone(),
        ]),
        vec![smaller.clone(), feasible, smaller]
    );
    assert_eq!(world.state_key(), before);
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

    let mut simulated = world.clone();
    simulated
        .apply(transform(1))
        .expect("the branch is feasible");
    assert_ne!(simulated.state_key(), world.state_key());
    assert_ne!(simulated.state_fingerprint(), before_fingerprint);
    assert_eq!(world.state_key(), before);
    assert_eq!(world.state_fingerprint(), before_fingerprint);
}

#[test]
fn replay_applies_all_of_a_trace_or_none_of_it() {
    let original = world();
    let mut prefix = original.clone();
    let accepted = transform(2);
    let rejected = transform(1);
    prefix.apply(accepted.clone()).unwrap();
    assert!(!prefix.assess(&rejected).is_applicable());
    // The exchange after the failure would apply; it must not run either.
    assert!(prefix.is_applicable(&finish()));
    let mut trace = Trace::new();
    trace.extend([accepted, rejected, finish()]);
    let before = original.state_key();

    let mut actual = original.clone();
    assert!(matches!(
        actual.replay(&trace),
        Err(ReplayError {
            index: 1,
            error: ApplyError::Infeasible { .. }
        })
    ));
    assert_eq!(actual.state_key(), before);
    assert!(matches!(
        original.replayed(&trace),
        Err(ReplayError { index: 1, .. })
    ));
    assert_eq!(original.state_key(), before);
}

#[test]
fn economic_views_have_canonical_observation_identities() {
    let world = world();
    let source_and_sink = world.view([AccountId::Source, AccountId::Sink]);
    let source_only = world.view([AccountId::Source]);

    assert_eq!(
        source_and_sink.observation_key().balances(),
        vec![(AccountId::Source, Asset::Input, Quantity::new(4))]
    );
    assert_ne!(
        source_and_sink.observation_key(),
        source_only.observation_key(),
        "the account visibility boundary is part of the information identity"
    );
    assert_eq!(
        source_and_sink.observation_key().visible_accounts(),
        &[AccountId::Source, AccountId::Sink]
    );
    assert!(
        source_and_sink
            .observation_key()
            .balances()
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
    let mut rate_overflow = EconomyBuilder::<Test>::new()
        .account(
            AccountId::Source,
            Account::from(Basket::from([(Asset::Input, Quantity::MAX)])),
        )
        .rate(
            RateId::Overflow,
            Rate::new().consume(Role::Source, Basket::from([(Asset::Input, Quantity::MAX)])),
        )
        .build()
        .expect("test model is valid");
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

    let mut balance_overflow = EconomyBuilder::<Test>::new()
        .account(
            AccountId::Sink,
            Account::from(Basket::from([(Asset::Output, Quantity::MAX)])),
        )
        .rate(
            RateId::Overflow,
            Rate::new().produce(Role::Sink, basket([(Asset::Output, 1)])),
        )
        .build()
        .expect("test model is valid");
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Bytes {}

impl axionomy::Ontology for Bytes {
    const NAME: &'static str = "bytes";
    type Account = u8;
    type Asset = u8;
    type Rate = u8;
    type Role = u8;
}

#[test]
fn observation_keys_distinguish_missing_from_empty_visible_accounts() {
    let empty: Economy<Bytes> = EconomyBuilder::new()
        .account(0, Account::default())
        .build()
        .unwrap();
    let missing: Economy<Bytes> = EconomyBuilder::new().build().unwrap();
    let present_view = empty.view([1, 0, 0]);
    let absent_view = missing.view([0, 1]);
    assert_eq!(present_view.balance(&0, &2), Some(Quantity::new(0)));
    assert_eq!(absent_view.balance(&0, &2), None);
    assert_eq!(present_view.balance(&1, &2), None);
    assert_eq!(present_view.balance(&2, &2), None);
    let present_key = present_view.observation_key();
    let absent_key = absent_view.observation_key();
    assert_eq!(present_key.visible_accounts(), &[0, 1]);
    assert_eq!(present_key.existing_accounts(), &[0]);
    assert!(present_key.balances().is_empty());
    assert!(absent_key.existing_accounts().is_empty());
    assert_ne!(present_key, absent_key);
    // Hidden account existence must not leak into the observation.
    assert_eq!(
        empty.view([1]).observation_key(),
        missing.view([1]).observation_key()
    );
    // A goal mentioning an empty account still requires that account to exist.
    let goal = Goal::new().require(0, Basket::new());
    assert!(empty.matches(&goal));
    assert!(!missing.matches(&goal));
}
