//! What the problems' studies and views share: their catalog entries, and
//! exact fractions.

use axionomy_service::{
    Capability, InstanceDescriptor, InstanceProfile, ProblemDescriptor, ProblemFamily, ProblemPath,
    StrategyDescriptor, StrategyPath,
};
use std::ops::Rem;

/// A problem as the catalog lists it, before its family, strategies and
/// capabilities.
pub(crate) struct ProblemCopy<'a> {
    pub key: ProblemPath,
    pub title: &'a str,
    pub summary: &'a str,
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

/// A problem's catalog entry: its Micro, Showcase and Stress instances, the
/// Showcase by default, and its strategies.
pub(crate) fn problem(
    copy: ProblemCopy<'_>,
    family: ProblemFamily,
    default_strategy: StrategyPath,
    strategies: &[StrategyCopy<'_>],
    capabilities: &[Capability],
) -> ProblemDescriptor {
    ProblemDescriptor {
        key: copy.key,
        title: copy.title.into(),
        summary: copy.summary.into(),
        family,
        default_instance: InstanceProfile::Showcase,
        instances: [
            InstanceProfile::Micro,
            InstanceProfile::Showcase,
            InstanceProfile::Stress,
        ]
        .into_iter()
        .zip(["Micro", "Showcase", "Stress"])
        .zip(copy.instances)
        .map(|((profile, label), description)| InstanceDescriptor {
            profile,
            label: label.into(),
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
        capabilities: capabilities.to_vec(),
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
