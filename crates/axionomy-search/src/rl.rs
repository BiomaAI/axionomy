//! Reinforcement-learning projections over closed economic execution.

use axionomy::Ontology;
use axionomy::{ApplyError, Economy, Exchange, ExchangeAssessment, Receipt, Trace};

#[derive(Debug, Clone)]
pub enum RlExecution<O: Ontology> {
    Applied(Receipt<O>),
    Rejected(ApplyError<O>),
}

impl<O: Ontology> RlExecution<O> {
    pub const fn applied(&self) -> bool {
        matches!(self, Self::Applied(_))
    }
}

#[derive(Debug, Clone)]
pub struct RlTransition<Observation, Outcome, O: Ontology> {
    before: Observation,
    action: Exchange<O>,
    assessment: ExchangeAssessment<O>,
    execution: RlExecution<O>,
    after: Observation,
    outcome: Outcome,
    terminal: bool,
}

impl<Observation, Outcome, O: Ontology> RlTransition<Observation, Outcome, O> {
    pub const fn before(&self) -> &Observation {
        &self.before
    }

    pub const fn action(&self) -> &Exchange<O> {
        &self.action
    }

    pub const fn assessment(&self) -> &ExchangeAssessment<O> {
        &self.assessment
    }

    pub const fn execution(&self) -> &RlExecution<O> {
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
fn step<O: Ontology, Observation, Outcome, Observe, ReadOutcome, Terminal>(
    world: &mut Economy<O>,
    action: Exchange<O>,
    observe: Observe,
    read_outcome: ReadOutcome,
    is_terminal: Terminal,
) -> RlTransition<Observation, Outcome, O>
where
    Observe: Fn(&Economy<O>) -> Observation,
    ReadOutcome: Fn(&Economy<O>) -> Outcome,
    Terminal: Fn(&Economy<O>) -> bool,
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

pub type RlTrajectoryResult<Observation, Outcome, O> =
    Result<Vec<RlTransition<Observation, Outcome, O>>, ApplyError<O>>;

/// Replays a trace on a fork and returns one learning transition per exchange.
pub fn replay_transitions<O: Ontology, Observation, Outcome, Observe, ReadOutcome, Terminal>(
    initial: &Economy<O>,
    trace: &Trace<O>,
    observe: Observe,
    read_outcome: ReadOutcome,
    is_terminal: Terminal,
) -> RlTrajectoryResult<Observation, Outcome, O>
where
    Observe: Fn(&Economy<O>) -> Observation,
    ReadOutcome: Fn(&Economy<O>) -> Outcome,
    Terminal: Fn(&Economy<O>) -> bool,
{
    let mut world = initial.clone();
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

#[cfg(test)]
mod tests {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    enum Test {}

    impl axionomy::Ontology for Test {
        const NAME: &'static str = "test";
        type Account = AccountId;
        type Asset = Asset;
        type Rate = RateId;
        type Role = Role;
    }

    use super::*;
    use axionomy::{Account, EconomyBuilder, Quantity, Rate, basket};

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

    type World = Economy<Test>;
    type Action = Exchange<Test>;

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
            world.balance(&AccountId::Actor, &Asset::Energy).to_u64(),
            world.balance(&AccountId::Actor, &Asset::Progress).to_u64(),
            world.balance(&AccountId::Actor, &Asset::Solved).to_u64(),
        ]
    }

    #[test]
    fn applied_step_records_observations_receipt_and_encoded_outcome() {
        let mut world = world();
        let transition = step(
            &mut world,
            action(RateId::Advance),
            observation,
            |world| world.balance(&AccountId::Actor, &Asset::Progress).to_u64(),
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
            |world| world.balance(&AccountId::Actor, &Asset::Solved).to_u64(),
            |world| !world.balance(&AccountId::Actor, &Asset::Solved).is_zero(),
        )
        .expect("valid trace becomes a dataset");

        assert_eq!(transitions.len(), 2);
        assert!(!transitions[0].terminal());
        assert!(transitions[1].terminal());
        assert_eq!(transitions[1].outcome(), &1);
    }
}
