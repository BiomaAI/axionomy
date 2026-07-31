use axionomy::{Account, Economy, EconomyBuilder, Exchange, Goal, Quantity, Rate, basket};
use axionomy_search::rollout::{
    RolloutConfig, RolloutDecision, RolloutStop, RolloutTermination, TraceRetention, run_to_goal,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum AccountId {
    Actor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum Asset {
    Step(u8),
    Solved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum RateId {
    Advance(u8),
    Finish,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum Role {
    Actor,
}

type World = Economy<AccountId, Asset, RateId, Role>;
type Action = Exchange<RateId, Role, AccountId>;

fn world() -> World {
    EconomyBuilder::new()
        .account(
            AccountId::Actor,
            Account::from(basket([(Asset::Step(0), 1)])),
        )
        .rate(
            RateId::Advance(0),
            Rate::new()
                .consume(Role::Actor, basket([(Asset::Step(0), 1)]))
                .produce(Role::Actor, basket([(Asset::Step(1), 1)])),
        )
        .rate(
            RateId::Advance(1),
            Rate::new()
                .consume(Role::Actor, basket([(Asset::Step(1), 1)]))
                .produce(Role::Actor, basket([(Asset::Step(2), 1)])),
        )
        .rate(
            RateId::Finish,
            Rate::new()
                .consume(Role::Actor, basket([(Asset::Step(2), 1)]))
                .produce(Role::Actor, basket([(Asset::Solved, 1)])),
        )
        .build()
        .expect("test model is valid")
}

fn goal() -> Goal<AccountId, Asset> {
    Goal::new().require(AccountId::Actor, basket([(Asset::Solved, 1)]))
}

fn action(rate: RateId) -> Action {
    Exchange::new(rate, Quantity::new(1)).bind(Role::Actor, AccountId::Actor)
}

fn sequential_controller(world: &World, _: usize) -> RolloutDecision<Action> {
    let rate = if world.balance(&AccountId::Actor, &Asset::Step(0)).get() == 1 {
        RateId::Advance(0)
    } else if world.balance(&AccountId::Actor, &Asset::Step(1)).get() == 1 {
        RateId::Advance(1)
    } else if world.balance(&AccountId::Actor, &Asset::Step(2)).get() == 1 {
        RateId::Finish
    } else {
        return RolloutDecision::Stop(RolloutStop::NoProposal);
    };
    RolloutDecision::Propose(action(rate))
}

#[test]
fn rollout_reaches_encoded_goal_and_replays() {
    let initial = world();
    let before = initial.state_key();
    let rollout = run_to_goal(
        &initial,
        &goal(),
        RolloutConfig::new(3).with_retention(TraceRetention::Full),
        sequential_controller,
    );

    assert!(rollout.termination().is_terminal());
    assert_eq!(rollout.steps(), 3);
    assert_eq!(rollout.receipts().len(), 3);
    let trace = rollout.trace().expect("full retention keeps a trace");
    let replayed = initial.replayed(trace).expect("rollout must replay");
    assert_eq!(replayed.state_key(), rollout.world().state_key());
    assert_eq!(initial.state_key(), before);
}

#[test]
fn horizon_is_not_terminal_success() {
    let rollout = run_to_goal(
        &world(),
        &goal(),
        RolloutConfig::new(2),
        sequential_controller,
    );

    assert!(matches!(
        rollout.termination(),
        RolloutTermination::HorizonReached
    ));
    assert!(!rollout.world().matches(&goal()));
}

#[test]
fn rejected_proposal_preserves_the_last_valid_branch_state() {
    let initial = world();
    let rollout = run_to_goal(&initial, &goal(), RolloutConfig::new(3), |_, _| {
        RolloutDecision::Propose(action(RateId::Finish))
    });

    assert!(matches!(
        rollout.termination(),
        RolloutTermination::Rejected(_)
    ));
    assert_eq!(rollout.steps(), 0);
    assert_eq!(rollout.world().state_key(), initial.state_key());
}

#[test]
fn trace_retention_can_be_disabled() {
    let rollout = run_to_goal(
        &world(),
        &goal(),
        RolloutConfig::new(3).with_retention(TraceRetention::None),
        sequential_controller,
    );

    assert!(rollout.trace().is_none());
    assert!(rollout.receipts().is_empty());
    assert!(rollout.termination().is_terminal());
}
