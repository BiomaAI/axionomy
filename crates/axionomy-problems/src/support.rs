//! What the problems' studies and views share: their catalog entries, the
//! words those entries are written in, and exact fractions.

use axionomy::{Identifier, path};
use axionomy_study::{
    InstanceDescriptor, ProblemDescriptor, ProblemPath, StrategyDescriptor, StrategyPath,
    StudyError, StudyKey,
};
use std::ops::Rem;

/// A problem as the catalog lists it, before its family, strategies and
/// capabilities.
pub(crate) struct ProblemCopy<'a> {
    pub key: ProblemPath,
    pub title: &'a str,
    pub summary: &'a str,
    /// What its Micro, Showcase and Stress instances are, in that order.
    pub instances: [&'a str; 3],
}

/// A strategy as the catalog lists it.
pub(crate) struct StrategyCopy<'a> {
    pub key: StrategyPath,
    pub label: &'a str,
    pub description: &'a str,
    /// The algorithm in words.
    pub algorithm: &'a str,
}

/// The sizes every canonical problem comes in, each one of its instances:
/// `micro`, `showcase` and `stress`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Identifier)]
pub(crate) enum Size {
    /// The smallest economy that still shows the problem, small enough for
    /// exact oracles.
    Micro,
    /// The economy Studio opens and saves: the default.
    Showcase,
    /// A larger economy, scenario set or work budget.
    Stress,
}

impl Size {
    const ALL: [Self; 3] = [Self::Micro, Self::Showcase, Self::Stress];

    fn label(self) -> &'static str {
        match self {
            Self::Micro => "Micro",
            Self::Showcase => "Showcase",
            Self::Stress => "Stress",
        }
    }
}

/// The size of the instance a study of a canonical problem asks for.
pub(crate) fn size(study: &StudyKey) -> Result<Size, StudyError> {
    axionomy_study::instance(study)
}

/// The families the canonical problems belong to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Identifier)]
pub(crate) enum Family {
    Pathfinding,
    Constraint,
    Production,
    Scheduling,
    Allocation,
    Market,
    StochasticPlanning,
    AdversarialGame,
    PartialObservation,
    TemporalSimulation,
    MultiAgentCompetition,
}

impl From<Family> for axionomy_study::ProblemFamily {
    fn from(family: Family) -> Self {
        let label = match family {
            Family::Pathfinding => "Pathfinding",
            Family::Constraint => "Constraint satisfaction",
            Family::Production => "Production",
            Family::Scheduling => "Scheduling",
            Family::Allocation => "Allocation",
            Family::Market => "Market clearing",
            Family::StochasticPlanning => "Planning under uncertainty",
            Family::AdversarialGame => "Adversarial game",
            Family::PartialObservation => "Hidden information",
            Family::TemporalSimulation => "Time and decay",
            Family::MultiAgentCompetition => "Multi-agent competition",
        };
        Self::new(path::write(&family), label)
    }
}

/// What the canonical problems exercise.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Identifier)]
pub(crate) enum Capability {
    DeterministicSearch,
    WeightedSearch,
    SpecializedAlgorithm,
    ExactPareto,
    ApproximatePareto,
    FeasibilityAssessment,
    MultiAccountExchange,
    AtomicSettlement,
    BranchOptimization,
    MonteCarlo,
    Mcts,
    InformationSetSearch,
    PartialObservation,
    Chance,
    TemporalEffects,
    FungibleCohorts,
    NonFungibleFacts,
    RlProjection,
    ReplayDerivedLeaderboards,
}

impl From<Capability> for axionomy_study::Capability {
    fn from(capability: Capability) -> Self {
        let label = match capability {
            Capability::DeterministicSearch => "Deterministic search",
            Capability::WeightedSearch => "Cost-guided search",
            Capability::SpecializedAlgorithm => "Specialised algorithm",
            Capability::ExactPareto => "Exact Pareto frontier",
            Capability::ApproximatePareto => "Sampled Pareto frontier",
            Capability::FeasibilityAssessment => "Explains why something is impossible",
            Capability::MultiAccountExchange => "Multi-account atomic changes",
            Capability::AtomicSettlement => "Atomic settlement",
            Capability::BranchOptimization => "Branch and bound",
            Capability::MonteCarlo => "Monte Carlo",
            Capability::Mcts => "MCTS",
            Capability::InformationSetSearch => "Information-set search (ISMCTS)",
            Capability::PartialObservation => "Partial observation",
            Capability::Chance => "Random events",
            Capability::TemporalEffects => "Time-triggered effects",
            Capability::FungibleCohorts => "Interchangeable batches",
            Capability::NonFungibleFacts => "Unique facts",
            Capability::RlProjection => "Reinforcement-learning training data",
            Capability::ReplayDerivedLeaderboards => "Replay-derived leaderboards",
        };
        Self::new(path::write(&capability), label)
    }
}

/// A problem's catalog entry: its Micro, Showcase and Stress instances, the
/// Showcase by default, its family, its strategies and what it exercises.
pub(crate) fn problem(
    copy: ProblemCopy<'_>,
    family: Family,
    default_strategy: StrategyPath,
    strategies: &[StrategyCopy<'_>],
    capabilities: &[Capability],
) -> ProblemDescriptor {
    ProblemDescriptor {
        key: copy.key,
        title: copy.title.into(),
        summary: copy.summary.into(),
        family: family.into(),
        default_instance: path::write(&Size::Showcase),
        instances: Size::ALL
            .into_iter()
            .zip(copy.instances)
            .map(|(size, description)| InstanceDescriptor {
                key: path::write(&size),
                label: size.label().into(),
                description: description.into(),
            })
            .collect(),
        default_strategy,
        strategies: strategies
            .iter()
            .map(|strategy| StrategyDescriptor {
                key: strategy.key.clone(),
                label: strategy.label.into(),
                description: strategy.description.into(),
                algorithm: strategy.algorithm.into(),
            })
            .collect(),
        capabilities: capabilities
            .iter()
            .map(|&capability| capability.into())
            .collect(),
    }
}

/// The greatest common divisor of two counts, at least 1, so a fraction
/// reduced by it keeps a nonzero denominator.
pub(crate) fn gcd<T>(mut left: T, mut right: T) -> T
where
    T: Copy + Ord + Default + Rem<Output = T> + From<u8>,
{
    while right != T::default() {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left.max(T::from(1))
}
