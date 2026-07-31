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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SystematicSampler {
    next: u64,
}

impl SystematicSampler {
    pub const fn new() -> Self {
        Self { next: 0 }
    }

    pub const fn from_ticket(next: u64) -> Self {
        Self { next }
    }
}

impl TicketSource for SystematicSampler {
    fn ticket(&mut self, total_weight: u64) -> u64 {
        let ticket = self.next % total_weight;
        self.next = self.next.wrapping_add(1);
        ticket
    }
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

/// Adapts any caller-owned `rand` generator to weighted ticket sampling.
pub struct RngTickets<'a, R: ?Sized> {
    random: &'a mut R,
}

impl<'a, R: ?Sized> RngTickets<'a, R> {
    pub const fn new(random: &'a mut R) -> Self {
        Self { random }
    }
}

impl<R> TicketSource for RngTickets<'_, R>
where
    R: rand::Rng + ?Sized,
{
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
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    #[test]
    fn systematic_sampling_respects_integer_weights() {
        let outcomes = [
            WeightedExchange::new("a", 1),
            WeightedExchange::new("b", 2),
            WeightedExchange::new("ignored", 0),
        ];
        let mut sampler = SystematicSampler::new();
        let samples = (0..6)
            .map(|_| *sample(&outcomes, &mut sampler).expect("valid support"))
            .collect::<Vec<_>>();
        assert_eq!(samples, ["a", "b", "b", "a", "b", "b"]);
    }

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
    fn caller_owned_rand_generators_can_drive_sampling() {
        let outcomes = [
            WeightedExchange::new("left", 1),
            WeightedExchange::new("right", 1),
        ];
        let mut random = ChaCha8Rng::seed_from_u64(7);
        let mut tickets = RngTickets::new(&mut random);

        assert!(sample(&outcomes, &mut tickets).is_ok());
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
