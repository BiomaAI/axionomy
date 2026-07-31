//! Reinforcement-learning projections over closed economic execution.

use axionomy::{
    ApplyError, AssessmentStatus, Economy, Exchange, ExchangeAssessment, Quantity, Receipt, Trace,
};
use std::hash::Hash;

#[derive(Debug, Clone)]
pub struct AssessedAction<Action, Assessment> {
    action: Action,
    assessment: Assessment,
}

impl<Action, Assessment> AssessedAction<Action, Assessment> {
    pub const fn action(&self) -> &Action {
        &self.action
    }

    pub const fn assessment(&self) -> &Assessment {
        &self.assessment
    }
}

pub type EconomicAssessedAction<AccountId, A, RateId, Role> = AssessedAction<
    Exchange<RateId, Role, AccountId>,
    ExchangeAssessment<AccountId, A, RateId, Role>,
>;

pub fn assessed_actions<AccountId, A, RateId, Role>(
    world: &Economy<AccountId, A, RateId, Role>,
    candidates: impl IntoIterator<Item = Exchange<RateId, Role, AccountId>>,
) -> Vec<EconomicAssessedAction<AccountId, A, RateId, Role>>
where
    AccountId: Clone + Eq + Hash + Ord,
    A: Clone + Eq + Hash + Ord,
    RateId: Clone + Eq + Hash + Ord,
    Role: Clone + Ord,
{
    candidates
        .into_iter()
        .map(|action| AssessedAction {
            assessment: world.assess(&action),
            action,
        })
        .collect()
}

pub fn action_mask<AccountId, A, RateId, Role>(
    actions: &[EconomicAssessedAction<AccountId, A, RateId, Role>],
) -> Vec<bool> {
    actions
        .iter()
        .map(|action| action.assessment().is_applicable())
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShortfallFeature<AccountId, A> {
    account: AccountId,
    asset: A,
    missing: Quantity,
}

impl<AccountId, A> ShortfallFeature<AccountId, A> {
    pub const fn account(&self) -> &AccountId {
        &self.account
    }

    pub const fn asset(&self) -> &A {
        &self.asset
    }

    pub const fn missing(&self) -> Quantity {
        self.missing
    }
}

pub fn shortfall_features<AccountId, A, RateId, Role>(
    assessment: &ExchangeAssessment<AccountId, A, RateId, Role>,
) -> Vec<ShortfallFeature<AccountId, A>>
where
    AccountId: Clone,
    A: Clone + Ord,
{
    assessment
        .shortfalls()
        .iter()
        .flat_map(|shortfall| {
            shortfall
                .missing()
                .iter_sorted()
                .map(|(asset, missing)| ShortfallFeature {
                    account: shortfall.account().clone(),
                    asset: asset.clone(),
                    missing: missing.clone(),
                })
        })
        .collect()
}

#[derive(Debug, Clone)]
pub enum RlExecution<RateId, Role, AccountId, A> {
    Applied(Receipt<RateId, Role, AccountId, A>),
    Rejected(ApplyError<RateId, Role, AccountId, A>),
}

impl<RateId, Role, AccountId, A> RlExecution<RateId, Role, AccountId, A> {
    pub const fn applied(&self) -> bool {
        matches!(self, Self::Applied(_))
    }
}

#[derive(Debug, Clone)]
pub struct RlTransition<Observation, Outcome, AccountId, A, RateId, Role> {
    before: Observation,
    action: Exchange<RateId, Role, AccountId>,
    assessment: ExchangeAssessment<AccountId, A, RateId, Role>,
    execution: RlExecution<RateId, Role, AccountId, A>,
    after: Observation,
    outcome: Outcome,
    terminal: bool,
}

impl<Observation, Outcome, AccountId, A, RateId, Role>
    RlTransition<Observation, Outcome, AccountId, A, RateId, Role>
{
    pub const fn before(&self) -> &Observation {
        &self.before
    }

    pub const fn action(&self) -> &Exchange<RateId, Role, AccountId> {
        &self.action
    }

    pub const fn assessment(&self) -> &ExchangeAssessment<AccountId, A, RateId, Role> {
        &self.assessment
    }

    pub const fn execution(&self) -> &RlExecution<RateId, Role, AccountId, A> {
        &self.execution
    }

    pub const fn after(&self) -> &Observation {
        &self.after
    }

    pub const fn outcome(&self) -> &Outcome {
        &self.outcome
    }

    pub const fn terminal(&self) -> bool {
        self.terminal
    }
}

/// Applies one action and emits learning projections derived from economic
/// state before and after the sole core commit attempt.
pub fn step<AccountId, A, RateId, Role, Observation, Outcome, Observe, ReadOutcome, Terminal>(
    world: &mut Economy<AccountId, A, RateId, Role>,
    action: Exchange<RateId, Role, AccountId>,
    observe: Observe,
    read_outcome: ReadOutcome,
    is_terminal: Terminal,
) -> RlTransition<Observation, Outcome, AccountId, A, RateId, Role>
where
    AccountId: Clone + Eq + Hash + Ord,
    A: Clone + Eq + Hash + Ord,
    RateId: Clone + Eq + Hash + Ord,
    Role: Clone + Ord,
    Observe: Fn(&Economy<AccountId, A, RateId, Role>) -> Observation,
    ReadOutcome: Fn(&Economy<AccountId, A, RateId, Role>) -> Outcome,
    Terminal: Fn(&Economy<AccountId, A, RateId, Role>) -> bool,
{
    let before = observe(world);
    let assessment = world.assess(&action);
    let execution = match world.apply(action.clone()) {
        Ok(receipt) => RlExecution::Applied(receipt),
        Err(error) => RlExecution::Rejected(error),
    };
    let after = observe(world);
    let outcome = read_outcome(world);
    let terminal = is_terminal(world);
    RlTransition {
        before,
        action,
        assessment,
        execution,
        after,
        outcome,
        terminal,
    }
}

pub type RlTrajectoryResult<Observation, Outcome, AccountId, A, RateId, Role> = Result<
    Vec<RlTransition<Observation, Outcome, AccountId, A, RateId, Role>>,
    ApplyError<RateId, Role, AccountId, A>,
>;

/// Replays a trace on a fork and returns one learning transition per exchange.
pub fn replay_transitions<
    AccountId,
    A,
    RateId,
    Role,
    Observation,
    Outcome,
    Observe,
    ReadOutcome,
    Terminal,
>(
    initial: &Economy<AccountId, A, RateId, Role>,
    trace: &Trace<RateId, Role, AccountId>,
    observe: Observe,
    read_outcome: ReadOutcome,
    is_terminal: Terminal,
) -> RlTrajectoryResult<Observation, Outcome, AccountId, A, RateId, Role>
where
    AccountId: Clone + Eq + Hash + Ord,
    A: Clone + Eq + Hash + Ord,
    RateId: Clone + Eq + Hash + Ord,
    Role: Clone + Ord,
    Observe: Fn(&Economy<AccountId, A, RateId, Role>) -> Observation,
    ReadOutcome: Fn(&Economy<AccountId, A, RateId, Role>) -> Outcome,
    Terminal: Fn(&Economy<AccountId, A, RateId, Role>) -> bool,
{
    let mut world = initial.fork();
    let mut transitions = Vec::with_capacity(trace.exchanges().len());
    for action in trace.exchanges() {
        let transition = step(
            &mut world,
            action.clone(),
            &observe,
            &read_outcome,
            &is_terminal,
        );
        if let RlExecution::Rejected(error) = transition.execution() {
            return Err(error.clone());
        }
        transitions.push(transition);
    }
    Ok(transitions)
}

pub fn status_code(status: AssessmentStatus) -> [f32; 3] {
    match status {
        AssessmentStatus::Applicable => [1.0, 0.0, 0.0],
        AssessmentStatus::Infeasible => [0.0, 1.0, 0.0],
        AssessmentStatus::Invalid => [0.0, 0.0, 1.0],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axionomy::{Account, EconomyBuilder, Rate, basket};

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
    enum AccountId {
        Actor,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
    enum Asset {
        Energy,
        Progress,
        Solved,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
    enum RateId {
        Advance,
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
                Account::from(basket([(Asset::Energy, 1)])),
            )
            .rate(
                RateId::Advance,
                Rate::new()
                    .consume(Role::Actor, basket([(Asset::Energy, 1)]))
                    .produce(Role::Actor, basket([(Asset::Progress, 1)])),
            )
            .rate(
                RateId::Finish,
                Rate::new()
                    .consume(Role::Actor, basket([(Asset::Progress, 1)]))
                    .produce(Role::Actor, basket([(Asset::Solved, 1)])),
            )
            .build()
            .expect("test model is valid")
    }

    fn action(rate: RateId) -> Action {
        Exchange::new(rate, Quantity::new(1)).bind(Role::Actor, AccountId::Actor)
    }

    fn observation(world: &World) -> [u64; 3] {
        [
            world.balance(&AccountId::Actor, &Asset::Energy).get(),
            world.balance(&AccountId::Actor, &Asset::Progress).get(),
            world.balance(&AccountId::Actor, &Asset::Solved).get(),
        ]
    }

    #[test]
    fn masks_and_shortfalls_come_from_assessment() {
        let world = world();
        let actions = assessed_actions(&world, [action(RateId::Advance), action(RateId::Finish)]);

        assert_eq!(action_mask(&actions), vec![true, false]);
        let features = shortfall_features(actions[1].assessment());
        assert_eq!(features.len(), 1);
        assert_eq!(features[0].asset(), &Asset::Progress);
        assert_eq!(features[0].missing(), Quantity::new(1));
    }

    #[test]
    fn applied_step_records_observations_receipt_and_encoded_outcome() {
        let mut world = world();
        let transition = step(
            &mut world,
            action(RateId::Advance),
            observation,
            |world| world.balance(&AccountId::Actor, &Asset::Progress).get(),
            |_| false,
        );

        assert_eq!(transition.before(), &[1, 0, 0]);
        assert_eq!(transition.after(), &[0, 1, 0]);
        assert_eq!(transition.outcome(), &1);
        assert!(transition.execution().applied());
        assert!(!transition.terminal());
    }

    #[test]
    fn replayed_trace_becomes_a_learning_trajectory() {
        let mut trace = Trace::new();
        trace.push(action(RateId::Advance));
        trace.push(action(RateId::Finish));
        let transitions = replay_transitions(
            &world(),
            &trace,
            observation,
            |world| world.balance(&AccountId::Actor, &Asset::Solved).get(),
            |world| !world.balance(&AccountId::Actor, &Asset::Solved).is_zero(),
        )
        .expect("valid trace becomes a dataset");

        assert_eq!(transitions.len(), 2);
        assert!(!transitions[0].terminal());
        assert!(transitions[1].terminal());
        assert_eq!(transitions[1].outcome(), &1);
    }
}
