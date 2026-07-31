//! Deterministic selection among weighted, core-encoded exchange proposals.

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SamplingError {
    EmptySupport,
    ZeroTotalWeight,
    TotalWeightOverflow,
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

/// Small deterministic generator for reproducible solver exploration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SeededSampler {
    state: u64,
}

impl SeededSampler {
    pub const fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut value = self.state;
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        value ^ (value >> 31)
    }
}

impl TicketSource for SeededSampler {
    fn ticket(&mut self, total_weight: u64) -> u64 {
        let rejection_limit = u64::MAX - (u64::MAX % total_weight);
        loop {
            let value = self.next_u64();
            if value < rejection_limit {
                return value % total_weight;
            }
        }
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
