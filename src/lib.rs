#![doc = include_str!("../README.md")]

pub mod account;
pub mod amount;
pub mod basket;
pub mod economy;
pub mod exchange;
pub mod quantity;
pub mod rate;

pub use account::{Account, AccountError};
pub use amount::AssetAmount;
pub use basket::{Basket, BasketError};
pub use economy::{
    AccountAssessment, AccountShortfall, ApplyError, ApplyResult, AssessmentStatus, EconomicView,
    Economy, EconomyBuilder, ExchangeAssessment, Goal, ModelBuildError, ModelBuildResult,
    ModelIssue, ObservationKey, ReplayResult, SimulationResult, StateFingerprint,
};
pub use exchange::{AccountDelta, Exchange, Receipt, Trace};
pub use quantity::{Quantity, QuantityError, QuantityScalar};
pub use rate::{LinearInvariant, Rate, RateError, basket};
