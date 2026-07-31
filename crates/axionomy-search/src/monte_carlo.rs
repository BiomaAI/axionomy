//! Generic repeated-experiment evaluation for core-validated rollouts.

use std::convert::Infallible;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    fn summarize(self) -> Self::Summary;
}

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MonteCarloError<ExperimentError, StatisticsError> {
    Experiment {
        policy_index: usize,
        sample_index: usize,
        error: ExperimentError,
    },
    Statistics {
        policy_index: usize,
        sample_index: usize,
        error: StatisticsError,
    },
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
    mut experiment: Experiment,
    mut make_statistics: MakeStatistics,
) -> Result<Vec<PolicyEstimate<Policy, Summary>>, MonteCarloError<ExperimentError, StatisticsError>>
where
    Policies: IntoIterator<Item = Policy>,
    Experiment: FnMut(&Policy, usize) -> Result<Observation, ExperimentError>,
    MakeStatistics: FnMut() -> Statistic,
    Statistic: Statistics<Observation, Summary = Summary, Error = StatisticsError>,
{
    policies
        .into_iter()
        .enumerate()
        .map(|(policy_index, policy)| {
            let mut statistics = make_statistics();
            for sample_index in 0..config.samples() {
                let observation = experiment(&policy, sample_index).map_err(|error| {
                    MonteCarloError::Experiment {
                        policy_index,
                        sample_index,
                        error,
                    }
                })?;
                statistics
                    .observe(observation)
                    .map_err(|error| MonteCarloError::Statistics {
                        policy_index,
                        sample_index,
                        error,
                    })?;
            }
            Ok(PolicyEstimate {
                policy,
                summary: statistics.summarize(),
            })
        })
        .collect()
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
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

    fn summarize(self) -> Self::Summary {
        BernoulliSummary {
            samples: self.samples,
            successes: self.successes,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScalarStatisticsError {
    NonFiniteValue,
}

#[derive(Debug, Clone, Default)]
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

    fn summarize(mut self) -> Self::Summary {
        self.values.sort_by(f64::total_cmp);
        let count = self.values.len();
        let mean = if count == 0 {
            0.0
        } else {
            self.values.iter().sum::<f64>() / count as f64
        };
        let variance = if count == 0 {
            0.0
        } else {
            self.values
                .iter()
                .map(|value| {
                    let difference = value - mean;
                    difference * difference
                })
                .sum::<f64>()
                / count as f64
        };
        ScalarSummary {
            values: self.values,
            mean,
            variance,
        }
    }
}

#[derive(Debug, Clone)]
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
        let probability = probability.clamp(0.0, 1.0);
        let index = (probability * (self.values.len() - 1) as f64).round() as usize;
        self.values.get(index).copied()
    }

    /// Mean of the lowest `fraction` of observations.
    pub fn lower_tail_mean(&self, fraction: f64) -> Option<f64> {
        if self.values.is_empty() || !fraction.is_finite() || fraction <= 0.0 {
            return None;
        }
        let count = ((self.values.len() as f64 * fraction.clamp(0.0, 1.0)).ceil() as usize).max(1);
        Some(self.values[..count].iter().sum::<f64>() / count as f64)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VectorStatisticsError {
    WrongDimensions { expected: usize, actual: usize },
    NonFiniteValue { dimension: usize },
}

#[derive(Debug, Clone)]
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

    fn summarize(self) -> Self::Summary {
        VectorSummary {
            dimensions: self
                .dimensions
                .into_iter()
                .map(ScalarStatistics::summarize)
                .collect(),
        }
    }
}

#[derive(Debug, Clone)]
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
        assert_eq!(summary.quantile(0.5), Some(3.0));
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
}
