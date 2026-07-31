#![doc = include_str!("../README.md")]

pub mod account;
pub mod basket;
pub mod economy;
pub mod exchange;
pub mod quantity;
pub mod rate;

pub use account::{Account, AccountError};
pub use basket::Basket;
pub use economy::{
    AccountAssessment, AccountShortfall, ApplyError, ApplyResult, AssessmentStatus, EconomicView,
    Economy, EconomyBuilder, ExchangeAssessment, Goal, ReplayResult, SimulationResult,
    StateFingerprint,
};
pub use exchange::{AccountDelta, Exchange, Receipt, Trace};
pub use quantity::Quantity;
pub use rate::{LinearInvariant, Rate, basket};
