//! Deterministic selection among weighted, core-encoded exchange proposals.

use rand::{RngExt, SeedableRng};
use rand_chacha::ChaCha8Rng;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WeightedExchange<Action> {
    exchange: Action,
    weight: u64,
}

impl<Action> WeightedExchange<Action> {
    pub const fn new(exchange: Action, weight: u64) -> Self {
        Self { exchange, weight }
    }

    pub const fn exchange(&self) -> &Action {
        &self.exchange
    }

    pub fn into_exchange(self) -> Action {
        self.exchange
    }

    pub const fn weight(&self) -> u64 {
        self.weight
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum SamplingError {
    #[error("weighted support is empty")]
    EmptySupport,
    #[error("weighted support has zero total weight")]
    ZeroTotalWeight,
    #[error("weighted support total overflowed u64")]
    TotalWeightOverflow,
    #[error("ticket {ticket} is outside total weight {total_weight}")]
    TicketOutOfRange { ticket: u64, total_weight: u64 },
}

pub trait TicketSource {
    fn ticket(&mut self, total_weight: u64) -> u64;
}

/// ChaCha8-backed deterministic generator for reproducible solver exploration.
#[derive(Debug, Clone)]
pub struct SeededSampler {
    random: ChaCha8Rng,
}

impl SeededSampler {
    pub fn new(seed: u64) -> Self {
        Self {
            random: ChaCha8Rng::seed_from_u64(seed),
        }
    }
}

impl TicketSource for SeededSampler {
    fn ticket(&mut self, total_weight: u64) -> u64 {
        self.random.random_range(0..total_weight)
    }
}

pub fn total_weight<Action>(outcomes: &[WeightedExchange<Action>]) -> Result<u64, SamplingError> {
    if outcomes.is_empty() {
        return Err(SamplingError::EmptySupport);
    }
    let total = outcomes
        .iter()
        .try_fold(0_u64, |total, outcome| total.checked_add(outcome.weight()));
    match total {
        Some(0) => Err(SamplingError::ZeroTotalWeight),
        Some(total) => Ok(total),
        None => Err(SamplingError::TotalWeightOverflow),
    }
}

pub fn choose_by_ticket<Action>(
    outcomes: &[WeightedExchange<Action>],
    ticket: u64,
) -> Result<&Action, SamplingError> {
    let total = total_weight(outcomes)?;
    if ticket >= total {
        return Err(SamplingError::TicketOutOfRange {
            ticket,
            total_weight: total,
        });
    }

    let mut remaining = ticket;
    for outcome in outcomes {
        if remaining < outcome.weight() {
            return Ok(outcome.exchange());
        }
        remaining -= outcome.weight();
    }
    unreachable!("a ticket below total weight selects one positive-weight outcome")
}

pub fn sample<'a, Action>(
    outcomes: &'a [WeightedExchange<Action>],
    tickets: &mut impl TicketSource,
) -> Result<&'a Action, SamplingError> {
    let total = total_weight(outcomes)?;
    choose_by_ticket(outcomes, tickets.ticket(total))
}

pub fn systematic_ticket(sample_index: usize, total_weight: u64) -> u64 {
    u64::try_from(sample_index).unwrap_or(u64::MAX) % total_weight
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seeded_sampling_is_reproducible() {
        let outcomes = [
            WeightedExchange::new(1, 1),
            WeightedExchange::new(2, 1),
            WeightedExchange::new(3, 1),
        ];
        let mut left = SeededSampler::new(42);
        let mut right = SeededSampler::new(42);
        for _ in 0..100 {
            assert_eq!(sample(&outcomes, &mut left), sample(&outcomes, &mut right));
        }
    }

    #[test]
    fn invalid_support_is_explained() {
        assert_eq!(total_weight::<()>(&[]), Err(SamplingError::EmptySupport));
        assert_eq!(
            total_weight(&[WeightedExchange::new((), 0)]),
            Err(SamplingError::ZeroTotalWeight)
        );
        assert_eq!(
            total_weight(&[
                WeightedExchange::new((), u64::MAX),
                WeightedExchange::new((), 1),
            ]),
            Err(SamplingError::TotalWeightOverflow)
        );
    }
}
