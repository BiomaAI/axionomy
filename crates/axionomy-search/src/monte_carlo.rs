//! Generic repeated-experiment evaluation for core-validated rollouts.

use crate::session::{AdvanceReport, Continue, SearchObserver, WorkBudget};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use statrs::distribution::{Beta, ContinuousCDF};
use statrs::statistics::{Data, OrderStatistics, Statistics as StatrsStatistics};
use std::convert::Infallible;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct MonteCarloConfig {
    samples: usize,
}

impl MonteCarloConfig {
    pub const fn new(samples: usize) -> Self {
        Self { samples }
    }

    pub const fn samples(self) -> usize {
        self.samples
    }
}

pub trait Statistics<Observation> {
    type Summary;
    type Error;

    fn observe(&mut self, observation: Observation) -> Result<(), Self::Error>;
    /// Produces an owned snapshot without consuming the accumulator.
    fn summarize(&self) -> Self::Summary;
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PolicyEstimate<Policy, Summary> {
    policy: Policy,
    summary: Summary,
}

impl<Policy, Summary> PolicyEstimate<Policy, Summary> {
    pub const fn policy(&self) -> &Policy {
        &self.policy
    }

    pub const fn summary(&self) -> &Summary {
        &self.summary
    }

    pub fn into_parts(self) -> (Policy, Summary) {
        (self.policy, self.summary)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum MonteCarloError<ExperimentError, StatisticsError> {
    #[error("experiment failed for policy {policy_index}, sample {sample_index}")]
    Experiment {
        policy_index: usize,
        sample_index: usize,
        error: ExperimentError,
    },
    #[error("statistics failed for policy {policy_index}, sample {sample_index}")]
    Statistics {
        policy_index: usize,
        sample_index: usize,
        error: StatisticsError,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MonteCarloStatus {
    Running,
    Completed,
    Interrupted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct MonteCarloProgress {
    policies: usize,
    samples_per_policy: usize,
    policy_index: usize,
    sample_index: usize,
    completed_samples: usize,
    total_samples: usize,
}

impl MonteCarloProgress {
    pub const fn policies(self) -> usize {
        self.policies
    }

    pub const fn samples_per_policy(self) -> usize {
        self.samples_per_policy
    }

    pub const fn policy_index(self) -> usize {
        self.policy_index
    }

    pub const fn sample_index(self) -> usize {
        self.sample_index
    }

    pub const fn completed_samples(self) -> usize {
        self.completed_samples
    }

    pub const fn total_samples(self) -> usize {
        self.total_samples
    }
}

/// Resumable policy evaluation with deterministic policy-major sample order.
pub struct MonteCarloSession<Policy, Statistic, Experiment> {
    policies: Vec<(Policy, Statistic)>,
    config: MonteCarloConfig,
    experiment: Experiment,
    policy_index: usize,
    sample_index: usize,
    completed_samples: usize,
}

impl<Policy, Statistic, Experiment> MonteCarloSession<Policy, Statistic, Experiment> {
    pub fn new(
        policies: impl IntoIterator<Item = Policy>,
        config: MonteCarloConfig,
        experiment: Experiment,
        mut make_statistics: impl FnMut() -> Statistic,
    ) -> Self {
        Self {
            policies: policies
                .into_iter()
                .map(|policy| (policy, make_statistics()))
                .collect(),
            config,
            experiment,
            policy_index: 0,
            sample_index: 0,
            completed_samples: 0,
        }
    }

    pub fn progress(&self) -> MonteCarloProgress {
        MonteCarloProgress {
            policies: self.policies.len(),
            samples_per_policy: self.config.samples(),
            policy_index: self.policy_index.min(self.policies.len()),
            sample_index: self.sample_index,
            completed_samples: self.completed_samples,
            total_samples: self.policies.len().saturating_mul(self.config.samples()),
        }
    }

    pub fn status(&self) -> MonteCarloStatus {
        if self.policy_index >= self.policies.len() || self.config.samples() == 0 {
            MonteCarloStatus::Completed
        } else {
            MonteCarloStatus::Running
        }
    }

    pub fn advance<Observation, ExperimentError, StatisticsError>(
        &mut self,
        budget: WorkBudget,
        observer: &mut impl SearchObserver<MonteCarloProgress>,
    ) -> Result<
        AdvanceReport<MonteCarloProgress, MonteCarloStatus>,
        MonteCarloError<ExperimentError, StatisticsError>,
    >
    where
        Experiment: FnMut(&Policy, usize) -> Result<Observation, ExperimentError>,
        Statistic: Statistics<Observation, Error = StatisticsError>,
    {
        if self.status() == MonteCarloStatus::Completed {
            return Ok(AdvanceReport::new(
                MonteCarloStatus::Completed,
                0,
                self.progress(),
            ));
        }

        let mut completed = 0;
        while completed < budget.units() && self.policy_index < self.policies.len() {
            if observer.observe(&self.progress()).is_break() {
                return Ok(AdvanceReport::new(
                    MonteCarloStatus::Interrupted,
                    completed,
                    self.progress(),
                ));
            }

            let (policy, statistics) = &mut self.policies[self.policy_index];
            let observation = (self.experiment)(policy, self.sample_index).map_err(|error| {
                MonteCarloError::Experiment {
                    policy_index: self.policy_index,
                    sample_index: self.sample_index,
                    error,
                }
            })?;
            statistics
                .observe(observation)
                .map_err(|error| MonteCarloError::Statistics {
                    policy_index: self.policy_index,
                    sample_index: self.sample_index,
                    error,
                })?;

            self.sample_index += 1;
            self.completed_samples += 1;
            completed += 1;
            if self.sample_index == self.config.samples() {
                self.policy_index += 1;
                self.sample_index = 0;
            }
        }

        Ok(AdvanceReport::new(
            self.status(),
            completed,
            self.progress(),
        ))
    }

    pub fn into_estimates<Observation, Summary, StatisticsError>(
        self,
    ) -> Option<Vec<PolicyEstimate<Policy, Summary>>>
    where
        Statistic: Statistics<Observation, Summary = Summary, Error = StatisticsError>,
    {
        (self.status() == MonteCarloStatus::Completed).then(|| {
            self.policies
                .into_iter()
                .map(|(policy, statistics)| PolicyEstimate {
                    policy,
                    summary: statistics.summarize(),
                })
                .collect()
        })
    }

    pub fn estimates<Observation, Summary, StatisticsError>(
        &self,
    ) -> Vec<PolicyEstimate<&Policy, Summary>>
    where
        Statistic: Statistics<Observation, Summary = Summary, Error = StatisticsError>,
    {
        self.policies
            .iter()
            .map(|(policy, statistics)| PolicyEstimate {
                policy,
                summary: statistics.summarize(),
            })
            .collect()
    }
}

/// Evaluates each policy with common sample indices.
///
/// The experiment is expected to run a core-validated rollout. Statistics are
/// disposable projections of encoded rollout outcomes.
pub fn evaluate<
    Policy,
    Policies,
    Observation,
    Summary,
    Experiment,
    ExperimentError,
    MakeStatistics,
    Statistic,
    StatisticsError,
>(
    policies: Policies,
    config: MonteCarloConfig,
    experiment: Experiment,
    make_statistics: MakeStatistics,
) -> Result<Vec<PolicyEstimate<Policy, Summary>>, MonteCarloError<ExperimentError, StatisticsError>>
where
    Policies: IntoIterator<Item = Policy>,
    Experiment: FnMut(&Policy, usize) -> Result<Observation, ExperimentError>,
    MakeStatistics: FnMut() -> Statistic,
    Statistic: Statistics<Observation, Summary = Summary, Error = StatisticsError>,
{
    let mut session = MonteCarloSession::new(policies, config, experiment, make_statistics);
    let mut observer = Continue;
    session.advance(WorkBudget::new(usize::MAX), &mut observer)?;
    Ok(session
        .into_estimates()
        .expect("an unbounded advance completes finite Monte Carlo work"))
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct BernoulliStatistics {
    samples: usize,
    successes: usize,
}

impl BernoulliStatistics {
    pub const fn new() -> Self {
        Self {
            samples: 0,
            successes: 0,
        }
    }
}

impl Statistics<bool> for BernoulliStatistics {
    type Summary = BernoulliSummary;
    type Error = Infallible;

    fn observe(&mut self, succeeded: bool) -> Result<(), Self::Error> {
        self.samples += 1;
        self.successes += usize::from(succeeded);
        Ok(())
    }

    fn summarize(&self) -> Self::Summary {
        BernoulliSummary {
            samples: self.samples,
            successes: self.successes,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct BernoulliSummary {
    samples: usize,
    successes: usize,
}

impl BernoulliSummary {
    pub const fn samples(self) -> usize {
        self.samples
    }

    pub const fn successes(self) -> usize {
        self.successes
    }

    pub fn probability(self) -> f64 {
        if self.samples == 0 {
            0.0
        } else {
            self.successes as f64 / self.samples as f64
        }
    }

    /// Equal-tailed posterior interval using a uniform Beta(1, 1) prior.
    pub fn credible_interval(self, confidence: f64) -> Option<(f64, f64)> {
        if self.samples == 0 || !confidence.is_finite() || !(0.0..1.0).contains(&confidence) {
            return None;
        }
        let failures = self.samples.checked_sub(self.successes)?;
        let posterior = Beta::new(self.successes as f64 + 1.0, failures as f64 + 1.0).ok()?;
        let tail = (1.0 - confidence) / 2.0;
        Some((
            posterior.inverse_cdf(tail),
            posterior.inverse_cdf(1.0 - tail),
        ))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum ScalarStatisticsError {
    #[error("statistics observation must be finite")]
    NonFiniteValue,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct ScalarStatistics {
    values: Vec<f64>,
}

impl ScalarStatistics {
    pub const fn new() -> Self {
        Self { values: Vec::new() }
    }
}

impl Statistics<f64> for ScalarStatistics {
    type Summary = ScalarSummary;
    type Error = ScalarStatisticsError;

    fn observe(&mut self, value: f64) -> Result<(), Self::Error> {
        if !value.is_finite() {
            return Err(ScalarStatisticsError::NonFiniteValue);
        }
        self.values.push(value);
        Ok(())
    }

    fn summarize(&self) -> Self::Summary {
        let mut values = self.values.clone();
        values.sort_by(f64::total_cmp);
        let count = values.len();
        let mean = if count == 0 {
            0.0
        } else {
            StatrsStatistics::mean(values.as_slice())
        };
        let variance = if count == 0 {
            0.0
        } else {
            StatrsStatistics::population_variance(values.as_slice())
        };
        ScalarSummary {
            values,
            mean,
            variance,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ScalarSummary {
    values: Vec<f64>,
    mean: f64,
    variance: f64,
}

impl ScalarSummary {
    pub fn samples(&self) -> usize {
        self.values.len()
    }

    pub const fn mean(&self) -> f64 {
        self.mean
    }

    pub const fn variance(&self) -> f64 {
        self.variance
    }

    pub fn minimum(&self) -> Option<f64> {
        self.values.first().copied()
    }

    pub fn maximum(&self) -> Option<f64> {
        self.values.last().copied()
    }

    pub fn quantile(&self, probability: f64) -> Option<f64> {
        if self.values.is_empty() || !probability.is_finite() {
            return None;
        }
        let mut data = Data::new(self.values.clone());
        Some(data.quantile(probability.clamp(0.0, 1.0)))
    }

    /// Mean of the lowest `fraction` of observations.
    pub fn lower_tail_mean(&self, fraction: f64) -> Option<f64> {
        if self.values.is_empty() || !fraction.is_finite() || fraction <= 0.0 {
            return None;
        }
        let count = ((self.values.len() as f64 * fraction.clamp(0.0, 1.0)).ceil() as usize).max(1);
        Some(StatrsStatistics::mean(&self.values[..count]))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum VectorStatisticsError {
    #[error("statistics vector has {actual} dimensions, expected {expected}")]
    WrongDimensions { expected: usize, actual: usize },
    #[error("statistics vector dimension {dimension} must be finite")]
    NonFiniteValue { dimension: usize },
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct VectorStatistics {
    dimensions: Vec<ScalarStatistics>,
}

impl VectorStatistics {
    pub fn new(dimensions: usize) -> Self {
        Self {
            dimensions: vec![ScalarStatistics::new(); dimensions],
        }
    }
}

impl Statistics<Vec<f64>> for VectorStatistics {
    type Summary = VectorSummary;
    type Error = VectorStatisticsError;

    fn observe(&mut self, values: Vec<f64>) -> Result<(), Self::Error> {
        if values.len() != self.dimensions.len() {
            return Err(VectorStatisticsError::WrongDimensions {
                expected: self.dimensions.len(),
                actual: values.len(),
            });
        }
        for (dimension, (statistics, value)) in self.dimensions.iter_mut().zip(values).enumerate() {
            statistics
                .observe(value)
                .map_err(|_| VectorStatisticsError::NonFiniteValue { dimension })?;
        }
        Ok(())
    }

    fn summarize(&self) -> Self::Summary {
        VectorSummary {
            dimensions: self
                .dimensions
                .iter()
                .map(ScalarStatistics::summarize)
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct VectorSummary {
    dimensions: Vec<ScalarSummary>,
}

impl VectorSummary {
    pub fn dimensions(&self) -> &[ScalarSummary] {
        &self.dimensions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evaluates_policies_with_shared_sample_indices() {
        let estimates = evaluate(
            ["always", "alternating"],
            MonteCarloConfig::new(4),
            |policy, sample| Ok::<_, Infallible>(*policy == "always" || sample % 2 == 0),
            BernoulliStatistics::new,
        )
        .expect("infallible experiment");

        assert_eq!(estimates[0].summary().successes(), 4);
        assert_eq!(estimates[1].summary().successes(), 2);
        let interval = estimates[1]
            .summary()
            .credible_interval(0.95)
            .expect("non-empty Bernoulli samples");
        assert!(interval.0 < 0.5 && interval.1 > 0.5);
    }

    #[test]
    fn scalar_summary_exposes_distribution_and_tail_risk() {
        let estimates = evaluate(
            [()],
            MonteCarloConfig::new(4),
            |_, sample| Ok::<_, Infallible>(sample as f64 + 1.0),
            ScalarStatistics::new,
        )
        .expect("finite samples");
        let summary = estimates[0].summary();

        assert_eq!(summary.samples(), 4);
        assert_eq!(summary.mean(), 2.5);
        assert_eq!(summary.variance(), 1.25);
        assert_eq!(summary.quantile(0.5), Some(2.5));
        assert_eq!(summary.lower_tail_mean(0.5), Some(1.5));
    }

    #[test]
    fn vector_statistics_reject_dimension_drift() {
        let error = evaluate(
            [()],
            MonteCarloConfig::new(1),
            |_, _| Ok::<_, Infallible>(vec![1.0]),
            || VectorStatistics::new(2),
        )
        .expect_err("the vector has the wrong dimensionality");
        assert!(matches!(
            error,
            MonteCarloError::Statistics {
                error: VectorStatisticsError::WrongDimensions {
                    expected: 2,
                    actual: 1,
                },
                ..
            }
        ));
    }

    #[test]
    fn monte_carlo_sessions_are_chunk_invariant_and_expose_snapshots() {
        let mut session = MonteCarloSession::new(
            ["always", "alternating"],
            MonteCarloConfig::new(4),
            |policy: &&str, sample| Ok::<_, Infallible>(*policy == "always" || sample % 2 == 0),
            BernoulliStatistics::new,
        );
        let mut observer = Continue;

        let first = session.advance(WorkBudget::new(3), &mut observer).unwrap();
        assert_eq!(first.status(), MonteCarloStatus::Running);
        assert_eq!(first.progress().completed_samples(), 3);
        let snapshots = session.estimates::<bool, BernoulliSummary, Infallible>();
        assert_eq!(snapshots[0].summary().samples(), 3);
        assert_eq!(snapshots[1].summary().samples(), 0);

        let second = session.advance(WorkBudget::new(5), &mut observer).unwrap();
        assert_eq!(second.status(), MonteCarloStatus::Completed);
        let estimates = session
            .into_estimates::<bool, BernoulliSummary, Infallible>()
            .unwrap();
        assert_eq!(estimates[0].summary().successes(), 4);
        assert_eq!(estimates[1].summary().successes(), 2);
    }

    #[test]
    fn monte_carlo_interruption_can_resume() {
        let mut session = MonteCarloSession::new(
            ["policy"],
            MonteCarloConfig::new(3),
            |_: &&str, _: usize| Ok::<_, Infallible>(true),
            BernoulliStatistics::new,
        );
        let mut observations = 0;
        let report = session
            .advance(WorkBudget::new(3), &mut |_: &MonteCarloProgress| {
                observations += 1;
                if observations == 2 {
                    std::ops::ControlFlow::Break(())
                } else {
                    std::ops::ControlFlow::Continue(())
                }
            })
            .unwrap();
        assert_eq!(report.status(), MonteCarloStatus::Interrupted);
        assert_eq!(report.work_completed(), 1);

        let mut observer = Continue;
        let report = session.advance(WorkBudget::new(2), &mut observer).unwrap();
        assert_eq!(report.status(), MonteCarloStatus::Completed);
    }
}
