#![doc = include_str!("../README.md")]

pub mod account;
mod accounts;
pub mod amount;
pub mod basket;
pub mod economy;
pub mod exchange;
pub mod path;
pub mod paths;
pub mod quantity;
pub mod rate;

pub use account::{Account, AccountError};
pub use amount::AssetAmount;
pub use basket::{Basket, BasketError};
pub use economy::{
    AccountAssessment, AccountShortfall, ApplyError, ApplyResult, AssessmentStatus, EconomicView,
    Economy, EconomyBuilder, ExchangeAssessment, Goal, ModelBuildError, ModelBuildResult,
    ModelIssue, ObservationKey, ReplayResult, StateFingerprint, StateKey,
};
pub use exchange::{AccountDelta, Exchange, Receipt, Trace};
pub use path::{AccountPath, AssetPath, PathError, PathKind, RatePath, RolePath};
pub use quantity::{Quantity, QuantityError, Rounding};
pub use rate::{
    ComputedAmount, LinearInvariant, Measure, QuantityComparison, QuantityExpression, Rate,
    RateCondition, basket,
};
