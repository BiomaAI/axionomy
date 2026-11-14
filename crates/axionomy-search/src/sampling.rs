//! Deterministic selection among weighted, core-encoded exchange proposals.
//!
//! Tickets come from one of two kinds of source. A [`SeededSampler`] draws a
//! sequence: each ticket is the next, so what it draws depends on everything
//! drawn before. That suits a decision-maker's own randomness, such as a tree
//! search exploring. [`EventTickets`] draw the world's chance by named event:
//! the tickets of an event depend only on the seed and the event's name, so
//! runs that differ elsewhere meet the same luck wherever they meet the same
//! event.

use axionomy::path::Path;
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

/// The golden-ratio increment SplitMix64 steps its state by.
const GOLDEN_GAMMA: u64 = 0x9e37_79b9_7f4a_7c15;

/// Value `index` of the SplitMix64 sequence that starts at `seed`:
/// deterministic and the same on every platform.
pub const fn splitmix(seed: u64, index: u64) -> u64 {
    let mut value = seed.wrapping_add(GOLDEN_GAMMA.wrapping_mul(index));
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

/// The seed of sample `index` of a repeated trial seeded `seed`, well
/// apart from its neighbours' seeds.
pub fn sample_seed(seed: u64, index: usize) -> u64 {
    seed.wrapping_add(
        u64::try_from(index)
            .unwrap_or(u64::MAX)
            .wrapping_mul(GOLDEN_GAMMA),
    )
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

/// The kind of path a chance event is named by.
pub mod kind {
    use axionomy::PathKind;

    /// One occurrence of the world's chance, such as the weather a road
    /// meets at a given time.
    pub enum Event {}

    impl PathKind for Event {
        const SCHEMA: &'static str = "EventPath";
        const EXAMPLE: &'static str = "weather/direct-out/12";
    }
}

/// A chance event's name, such as `weather/direct-out/12`: the weather of
/// the direct road out, met at time 12. The name says which occurrence a
/// draw is for, so each occurrence the world meets has a name of its own.
pub type EventPath = Path<kind::Event>;

/// The tickets of the chance event `event` in a world seeded `seed`. They
/// depend on nothing but the seed and the event's name: the same event of
/// the same seed draws the same tickets whatever else was drawn before it,
/// and different events draw independently. Runs that differ elsewhere, such
/// as a branch that makes another choice, meet the same luck wherever they
/// meet the same event (common random numbers), so comparing them compares
/// the choices, not the luck.
///
/// The event's key folds the name's bytes into the seed eight at a time,
/// little-endian with the last word padded with zeros, each word mixed in by
/// SplitMix64, and then the name's length. Its tickets are the SplitMix64
/// sequence that starts at that key, each brought below the total weight by
/// Lemire's multiply-and-reject, so every ticket below the total is equally
/// likely. Only integer arithmetic is used, so every platform, wasm32 among
/// them, draws the same.
#[derive(Debug, Clone)]
pub struct EventTickets {
    key: u64,
    drawn: u64,
}

impl EventTickets {
    pub fn new(seed: u64, event: &EventPath) -> Self {
        let name = event.as_str().as_bytes();
        let folded = name.chunks(8).fold(seed, |key, chunk| {
            let mut word = [0; 8];
            word[..chunk.len()].copy_from_slice(chunk);
            splitmix(key ^ u64::from_le_bytes(word), 1)
        });
        Self {
            key: splitmix(folded ^ name.len() as u64, 1),
            drawn: 0,
        }
    }
}

impl TicketSource for EventTickets {
    /// The event's next ticket below `total_weight`, which must be positive,
    /// as [`sample`] asks only for a positive total.
    fn ticket(&mut self, total_weight: u64) -> u64 {
        // Values whose low half of the product falls below this would make
        // some tickets likelier than others; they are drawn again.
        let biased = total_weight.wrapping_neg() % total_weight;
        loop {
            self.drawn += 1;
            let product = u128::from(splitmix(self.key, self.drawn)) * u128::from(total_weight);
            if product as u64 >= biased {
                return (product >> 64) as u64;
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
    fn an_events_tickets_are_its_seed_and_name_alone() {
        let tickets = |seed, event: &str| {
            let mut tickets = EventTickets::new(seed, &event.parse().unwrap());
            [1000, 1000, 6, 1 << 63].map(|total| tickets.ticket(total))
        };
        // Fixed by the seed and the name's bytes on every platform: a store
        // replays a run's chance wherever it is opened.
        assert_eq!(
            tickets(17, "scenario"),
            [836, 662, 0, 7_672_138_121_928_009_263]
        );
        assert_eq!(
            tickets(17, "weather/direct-out/12"),
            [596, 461, 3, 4_840_340_453_360_151_081]
        );
        assert_eq!(
            tickets(0, "attempt/3/2"),
            [602, 362, 3, 4_285_863_854_964_752_419]
        );
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
