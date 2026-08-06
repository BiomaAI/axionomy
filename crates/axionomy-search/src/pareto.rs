//! Multi-objective comparison and exhaustive Pareto search.
//!
//! This module does not define utility for an economy. Callers project objective
//! values from assets in a resulting [`Economy`], while Axionomy continues to
//! decide which transitions and outcomes are valid.

use crate::session::{AdvanceReport, SearchObserver, SearchStatus, WorkBudget};
use axionomy::{Economy, Exchange, Goal, Quantity, QuantityScalar, Trace};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::{HashSet, VecDeque};
use std::hash::Hash;
use thiserror::Error;

/// Whether greater or smaller values improve one objective.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ObjectiveDirection {
    Minimize,
    Maximize,
}

/// One named, directed objective value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Objective<K, V> {
    key: K,
    direction: ObjectiveDirection,
    value: V,
}

impl<K, V> Objective<K, V> {
    pub fn minimize(key: K, value: V) -> Self {
        Self {
            key,
            direction: ObjectiveDirection::Minimize,
            value,
        }
    }

    pub fn maximize(key: K, value: V) -> Self {
        Self {
            key,
            direction: ObjectiveDirection::Maximize,
            value,
        }
    }

    pub fn key(&self) -> &K {
        &self.key
    }

    pub const fn direction(&self) -> ObjectiveDirection {
        self.direction
    }

    pub fn value(&self) -> &V {
        &self.value
    }
}

/// An ordered objective schema and its values for one outcome.
///
/// Vectors are comparable only when every dimension has the same key and
/// direction in the same order. The order is explicit so serialization and
/// diagnostics cannot silently change the meaning of a dimension.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ObjectiveVector<K, V> {
    objectives: Vec<Objective<K, V>>,
}

impl<K, V> ObjectiveVector<K, V>
where
    K: Eq,
{
    pub fn try_new(
        objectives: impl IntoIterator<Item = Objective<K, V>>,
    ) -> Result<Self, ParetoError> {
        let objectives: Vec<_> = objectives.into_iter().collect();
        for right in 0..objectives.len() {
            if let Some(left) = objectives[..right]
                .iter()
                .position(|candidate| candidate.key == objectives[right].key)
            {
                return Err(ParetoError::DuplicateObjective { left, right });
            }
        }
        Ok(Self { objectives })
    }

    pub fn dominance(&self, other: &Self) -> Result<Dominance, ParetoError>
    where
        V: PartialOrd,
    {
        if self.objectives.len() != other.objectives.len() {
            return Err(ParetoError::SchemaLength {
                left: self.objectives.len(),
                right: other.objectives.len(),
            });
        }

        let mut improves = false;
        let mut worsens = false;
        for (dimension, (left, right)) in self.objectives.iter().zip(&other.objectives).enumerate()
        {
            if left.key != right.key || left.direction != right.direction {
                return Err(ParetoError::SchemaMismatch { dimension });
            }
            let ordering = left
                .value
                .partial_cmp(&right.value)
                .ok_or(ParetoError::UnorderedValue { dimension })?;
            let improvement = match left.direction {
                ObjectiveDirection::Minimize => ordering.reverse(),
                ObjectiveDirection::Maximize => ordering,
            };
            improves |= improvement == Ordering::Greater;
            worsens |= improvement == Ordering::Less;
        }

        Ok(match (improves, worsens) {
            (true, false) => Dominance::Dominates,
            (false, true) => Dominance::Dominated,
            (false, false) => Dominance::Equal,
            (true, true) => Dominance::Incomparable,
        })
    }

    fn validate_values(&self) -> Result<(), ParetoError>
    where
        V: PartialOrd,
    {
        for (dimension, objective) in self.objectives.iter().enumerate() {
            objective
                .value
                .partial_cmp(&objective.value)
                .ok_or(ParetoError::UnorderedValue { dimension })?;
        }
        Ok(())
    }

    pub fn objectives(&self) -> &[Objective<K, V>] {
        &self.objectives
    }

    pub fn into_objectives(self) -> Vec<Objective<K, V>> {
        self.objectives
    }
}

/// The multi-objective relation between a left and right outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Dominance {
    Dominates,
    Dominated,
    Equal,
    Incomparable,
}

/// Whether the containing front is proven complete or is the best known so far.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FrontierCompleteness {
    Exact,
    Approximate,
}

/// A payload and its objective values retained on a Pareto front.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ParetoEntry<T, K, V> {
    payload: T,
    objectives: ObjectiveVector<K, V>,
}

impl<T, K, V> ParetoEntry<T, K, V> {
    pub fn payload(&self) -> &T {
        &self.payload
    }

    pub fn objectives(&self) -> &ObjectiveVector<K, V> {
        &self.objectives
    }

    pub fn into_parts(self) -> (T, ObjectiveVector<K, V>) {
        (self.payload, self.objectives)
    }
}

/// The result of considering one candidate for a front.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InsertResult {
    Inserted { removed: usize },
    Dominated,
    Equivalent,
}

/// An incrementally maintained set of mutually non-dominated outcomes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ParetoFront<T, K, V> {
    completeness: FrontierCompleteness,
    entries: Vec<ParetoEntry<T, K, V>>,
}

impl<T, K, V> ParetoFront<T, K, V>
where
    K: Eq,
    V: PartialOrd,
{
    pub const fn new(completeness: FrontierCompleteness) -> Self {
        Self {
            completeness,
            entries: Vec::new(),
        }
    }

    pub const fn exact() -> Self {
        Self::new(FrontierCompleteness::Exact)
    }

    pub const fn approximate() -> Self {
        Self::new(FrontierCompleteness::Approximate)
    }

    pub fn insert(
        &mut self,
        payload: T,
        objectives: ObjectiveVector<K, V>,
    ) -> Result<InsertResult, ParetoError> {
        objectives.validate_values()?;
        let mut dominated = Vec::new();
        for (index, entry) in self.entries.iter().enumerate() {
            match objectives.dominance(&entry.objectives)? {
                Dominance::Dominated => return Ok(InsertResult::Dominated),
                Dominance::Equal => return Ok(InsertResult::Equivalent),
                Dominance::Dominates => dominated.push(index),
                Dominance::Incomparable => {}
            }
        }

        let removed = dominated.len();
        let mut index = 0;
        self.entries.retain(|_| {
            let keep = dominated.binary_search(&index).is_err();
            index += 1;
            keep
        });
        self.entries.push(ParetoEntry {
            payload,
            objectives,
        });
        Ok(InsertResult::Inserted { removed })
    }

    pub const fn completeness(&self) -> FrontierCompleteness {
        self.completeness
    }

    pub fn entries(&self) -> &[ParetoEntry<T, K, V>] {
        &self.entries
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn into_entries(self) -> Vec<ParetoEntry<T, K, V>> {
        self.entries
    }

    fn mark_exact(&mut self) {
        self.completeness = FrontierCompleteness::Exact;
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ParetoError {
    #[error("objective dimensions {left} and {right} use the same key")]
    DuplicateObjective { left: usize, right: usize },
    #[error("objective vectors have different lengths ({left} and {right})")]
    SchemaLength { left: usize, right: usize },
    #[error("objective dimension {dimension} has a different key or direction")]
    SchemaMismatch { dimension: usize },
    #[error("objective dimension {dimension} contains unordered values")]
    UnorderedValue { dimension: usize },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ParetoSearchProgress {
    expanded: usize,
    generated: usize,
    frontier: usize,
    visited: usize,
    terminal_outcomes: usize,
    pareto_outcomes: usize,
}

impl ParetoSearchProgress {
    pub const fn expanded(self) -> usize {
        self.expanded
    }

    pub const fn generated(self) -> usize {
        self.generated
    }

    pub const fn frontier(self) -> usize {
        self.frontier
    }

    pub const fn visited(self) -> usize {
        self.visited
    }

    pub const fn terminal_outcomes(self) -> usize {
        self.terminal_outcomes
    }

    pub const fn pareto_outcomes(self) -> usize {
        self.pareto_outcomes
    }
}

struct ParetoSearchState<AccountId, A, RateId, Role, N>
where
    N: QuantityScalar,
{
    world: Economy<AccountId, A, RateId, Role, N>,
    trace: Trace<RateId, Role, AccountId, N>,
}

/// Resumable exhaustive search whose retained outcomes are replayable traces.
///
/// The front remains [`FrontierCompleteness::Approximate`] while work remains.
/// It becomes exact only when every reachable state has been exhausted.
pub struct ParetoSearchSession<AccountId, A, RateId, Role, N, K, V, Candidates, Project>
where
    N: QuantityScalar,
{
    goal: Goal<AccountId, A, N>,
    candidates: Candidates,
    project: Project,
    states: Vec<ParetoSearchState<AccountId, A, RateId, Role, N>>,
    frontier: VecDeque<usize>,
    visited: HashSet<Vec<(AccountId, A, Quantity<N>)>>,
    results: ParetoFront<Trace<RateId, Role, AccountId, N>, K, V>,
    expanded: usize,
    generated: usize,
    terminal_outcomes: usize,
    exhausted: bool,
}

impl<AccountId, A, RateId, Role, N, K, V, Candidates, Project>
    ParetoSearchSession<AccountId, A, RateId, Role, N, K, V, Candidates, Project>
where
    AccountId: Clone + Eq + Hash + Ord,
    A: Clone + Eq + Hash + Ord,
    RateId: Clone + Eq + Hash + Ord,
    Role: Clone + Ord,
    N: QuantityScalar,
    K: Eq,
    V: PartialOrd,
    Candidates:
        FnMut(&Economy<AccountId, A, RateId, Role, N>) -> Vec<Exchange<RateId, Role, AccountId, N>>,
    Project: FnMut(&Economy<AccountId, A, RateId, Role, N>) -> ObjectiveVector<K, V>,
{
    pub fn new(
        initial: &Economy<AccountId, A, RateId, Role, N>,
        goal: Goal<AccountId, A, N>,
        candidates: Candidates,
        project: Project,
    ) -> Self {
        let initial = initial.fork();
        let key = initial.state_key();
        Self {
            goal,
            candidates,
            project,
            states: vec![ParetoSearchState {
                world: initial,
                trace: Trace::new(),
            }],
            frontier: VecDeque::from([0]),
            visited: HashSet::from([key]),
            results: ParetoFront::approximate(),
            expanded: 0,
            generated: 0,
            terminal_outcomes: 0,
            exhausted: false,
        }
    }

    pub fn progress(&self) -> ParetoSearchProgress {
        ParetoSearchProgress {
            expanded: self.expanded,
            generated: self.generated,
            frontier: self.frontier.len(),
            visited: self.visited.len(),
            terminal_outcomes: self.terminal_outcomes,
            pareto_outcomes: self.results.len(),
        }
    }

    pub fn status(&self) -> SearchStatus {
        if self.exhausted {
            SearchStatus::Exhausted
        } else {
            SearchStatus::Running
        }
    }

    pub fn front(&self) -> &ParetoFront<Trace<RateId, Role, AccountId, N>, K, V> {
        &self.results
    }

    pub fn into_front(self) -> ParetoFront<Trace<RateId, Role, AccountId, N>, K, V> {
        self.results
    }

    pub fn advance(
        &mut self,
        budget: WorkBudget,
        observer: &mut impl SearchObserver<ParetoSearchProgress>,
    ) -> Result<AdvanceReport<ParetoSearchProgress>, ParetoError> {
        if self.exhausted {
            return Ok(AdvanceReport::new(
                SearchStatus::Exhausted,
                0,
                self.progress(),
            ));
        }

        let mut completed = 0;
        while completed < budget.units() {
            if observer.observe(&self.progress()).is_break() {
                return Ok(AdvanceReport::new(
                    SearchStatus::Interrupted,
                    completed,
                    self.progress(),
                ));
            }

            let Some(index) = self.frontier.pop_front() else {
                self.finish();
                return Ok(AdvanceReport::new(
                    SearchStatus::Exhausted,
                    completed,
                    self.progress(),
                ));
            };
            self.expanded += 1;
            completed += 1;

            if self.states[index].world.matches(&self.goal) {
                self.terminal_outcomes += 1;
                let objectives = (self.project)(&self.states[index].world);
                self.results
                    .insert(self.states[index].trace.clone(), objectives)?;
                continue;
            }

            let exchanges = (self.candidates)(&self.states[index].world);
            for exchange in exchanges {
                let mut next = self.states[index].world.fork();
                if next.apply(exchange.clone()).is_err() {
                    continue;
                }
                let key = next.state_key();
                if !self.visited.insert(key) {
                    continue;
                }
                self.generated += 1;
                let mut trace = self.states[index].trace.clone();
                trace.push(exchange);
                let next_index = self.states.len();
                self.states.push(ParetoSearchState { world: next, trace });
                self.frontier.push_back(next_index);
            }
        }

        if self.frontier.is_empty() {
            self.finish();
        }
        Ok(AdvanceReport::new(
            self.status(),
            completed,
            self.progress(),
        ))
    }

    fn finish(&mut self) {
        self.exhausted = true;
        self.results.mark_exact();
    }
}

/// The exact result of exhausting a finite reachable state space.
pub struct ParetoSearchResult<RateId, Role, AccountId, N, K, V> {
    front: ParetoFront<Trace<RateId, Role, AccountId, N>, K, V>,
    progress: ParetoSearchProgress,
}

impl<RateId, Role, AccountId, N, K, V> ParetoSearchResult<RateId, Role, AccountId, N, K, V> {
    pub fn front(&self) -> &ParetoFront<Trace<RateId, Role, AccountId, N>, K, V> {
        &self.front
    }

    pub fn into_front(self) -> ParetoFront<Trace<RateId, Role, AccountId, N>, K, V> {
        self.front
    }

    pub const fn progress(&self) -> ParetoSearchProgress {
        self.progress
    }
}

/// Exhausts a finite reachable state space and returns its replayable Pareto front.
pub fn search<AccountId, A, RateId, Role, N, K, V, Candidates, Project>(
    initial: &Economy<AccountId, A, RateId, Role, N>,
    goal: &Goal<AccountId, A, N>,
    candidates: Candidates,
    project: Project,
) -> Result<ParetoSearchResult<RateId, Role, AccountId, N, K, V>, ParetoError>
where
    AccountId: Clone + Eq + Hash + Ord,
    A: Clone + Eq + Hash + Ord,
    RateId: Clone + Eq + Hash + Ord,
    Role: Clone + Ord,
    N: QuantityScalar,
    K: Eq,
    V: PartialOrd,
    Candidates:
        FnMut(&Economy<AccountId, A, RateId, Role, N>) -> Vec<Exchange<RateId, Role, AccountId, N>>,
    Project: FnMut(&Economy<AccountId, A, RateId, Role, N>) -> ObjectiveVector<K, V>,
{
    let mut session = ParetoSearchSession::new(initial, goal.clone(), candidates, project);
    let mut observer = crate::session::Continue;
    while !session.status().is_terminal() {
        session.advance(WorkBudget::new(usize::MAX), &mut observer)?;
    }
    let progress = session.progress();
    Ok(ParetoSearchResult {
        front: session.into_front(),
        progress,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use axionomy::{Account, EconomyBuilder, Rate, basket};
    use std::ops::ControlFlow;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
    enum AccountId {
        Agent,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
    enum Asset {
        Start,
        Done,
        Energy,
        Time,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
    enum RateId {
        Fast,
        Efficient,
        Wasteful,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
    enum Role {
        Actor,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum ObjectiveKey {
        Energy,
        Time,
    }

    type World = Economy<AccountId, Asset, RateId, Role>;

    fn vector(energy: u64, time: u64) -> ObjectiveVector<ObjectiveKey, u64> {
        ObjectiveVector::try_new([
            Objective::minimize(ObjectiveKey::Energy, energy),
            Objective::minimize(ObjectiveKey::Time, time),
        ])
        .unwrap()
    }

    #[test]
    fn dominance_observes_direction_and_tradeoffs() {
        let best = ObjectiveVector::try_new([
            Objective::minimize("cost", 2),
            Objective::maximize("quality", 8),
        ])
        .unwrap();
        let worse = ObjectiveVector::try_new([
            Objective::minimize("cost", 3),
            Objective::maximize("quality", 7),
        ])
        .unwrap();
        let tradeoff = ObjectiveVector::try_new([
            Objective::minimize("cost", 1),
            Objective::maximize("quality", 6),
        ])
        .unwrap();

        assert_eq!(best.dominance(&worse), Ok(Dominance::Dominates));
        assert_eq!(worse.dominance(&best), Ok(Dominance::Dominated));
        assert_eq!(best.dominance(&best), Ok(Dominance::Equal));
        assert_eq!(best.dominance(&tradeoff), Ok(Dominance::Incomparable));
    }

    #[test]
    fn schemas_and_unordered_values_fail_loudly() {
        let duplicate = ObjectiveVector::try_new([
            Objective::minimize("cost", 1),
            Objective::maximize("cost", 2),
        ]);
        assert_eq!(
            duplicate,
            Err(ParetoError::DuplicateObjective { left: 0, right: 1 })
        );

        let mut front = ParetoFront::approximate();
        assert_eq!(
            front.insert(
                "bad",
                ObjectiveVector::try_new([Objective::minimize("x", f64::NAN)]).unwrap()
            ),
            Err(ParetoError::UnorderedValue { dimension: 0 })
        );
    }

    #[test]
    fn incremental_front_removes_only_dominated_outcomes() {
        let mut front = ParetoFront::exact();
        assert_eq!(
            front.insert("balanced", vector(3, 3)),
            Ok(InsertResult::Inserted { removed: 0 })
        );
        assert_eq!(
            front.insert("efficient", vector(1, 5)),
            Ok(InsertResult::Inserted { removed: 0 })
        );
        assert_eq!(
            front.insert("fast", vector(5, 1)),
            Ok(InsertResult::Inserted { removed: 0 })
        );
        assert_eq!(
            front.insert("worse", vector(6, 6)),
            Ok(InsertResult::Dominated)
        );
        assert_eq!(
            front.insert("better balanced", vector(2, 2)),
            Ok(InsertResult::Inserted { removed: 1 })
        );

        assert_eq!(front.len(), 3);
        assert!(front.entries().iter().all(|left| {
            front.entries().iter().all(|right| {
                std::ptr::eq(left, right)
                    || left.objectives().dominance(right.objectives()).unwrap()
                        != Dominance::Dominated
            })
        }));
    }

    fn world() -> World {
        let outcome = |energy, time| {
            Rate::new()
                .consume(Role::Actor, basket([(Asset::Start, 1)]))
                .produce(
                    Role::Actor,
                    basket([
                        (Asset::Done, 1),
                        (Asset::Energy, energy),
                        (Asset::Time, time),
                    ]),
                )
        };
        EconomyBuilder::new()
            .account(AccountId::Agent, Account::from(basket([(Asset::Start, 1)])))
            .rate(RateId::Fast, outcome(5, 1))
            .rate(RateId::Efficient, outcome(1, 5))
            .rate(RateId::Wasteful, outcome(6, 6))
            .build()
            .unwrap()
    }

    fn candidates(world: &World) -> Vec<Exchange<RateId, Role, AccountId>> {
        world.applicable(
            world.rate_ids().copied().map(|rate| {
                Exchange::new(rate, Quantity::new(1)).bind(Role::Actor, AccountId::Agent)
            }),
        )
    }

    fn project(world: &World) -> ObjectiveVector<ObjectiveKey, u64> {
        vector(
            world.balance(&AccountId::Agent, &Asset::Energy).get(),
            world.balance(&AccountId::Agent, &Asset::Time).get(),
        )
    }

    #[test]
    fn exhaustive_front_is_exact_and_every_outcome_replays() {
        let initial = world();
        let goal = Goal::new().require(AccountId::Agent, basket([(Asset::Done, 1)]));
        let result = search(&initial, &goal, candidates, project).unwrap();

        assert_eq!(result.front().completeness(), FrontierCompleteness::Exact);
        assert_eq!(result.progress().terminal_outcomes(), 3);
        assert_eq!(result.front().len(), 2);

        let mut outcomes = Vec::new();
        for entry in result.front().entries() {
            let replayed = initial.replayed(entry.payload()).unwrap();
            assert!(replayed.matches(&goal));
            assert_eq!(&project(&replayed), entry.objectives());
            outcomes.push((
                *entry.objectives().objectives()[0].value(),
                *entry.objectives().objectives()[1].value(),
            ));
        }
        outcomes.sort_unstable();
        assert_eq!(outcomes, [(1, 5), (5, 1)]);
    }

    #[test]
    fn interrupted_front_is_approximate_and_session_resumes() {
        let initial = world();
        let goal = Goal::new().require(AccountId::Agent, basket([(Asset::Done, 1)]));
        let mut session = ParetoSearchSession::new(&initial, goal, candidates, project);
        let mut interrupt = |_: &ParetoSearchProgress| ControlFlow::Break(());
        let report = session
            .advance(WorkBudget::new(10), &mut interrupt)
            .unwrap();
        assert_eq!(report.status(), SearchStatus::Interrupted);
        assert_eq!(
            session.front().completeness(),
            FrontierCompleteness::Approximate
        );

        let mut observer = crate::session::Continue;
        while !session.status().is_terminal() {
            session.advance(WorkBudget::new(1), &mut observer).unwrap();
        }
        assert_eq!(session.front().completeness(), FrontierCompleteness::Exact);
        assert_eq!(session.front().len(), 2);
    }
}
