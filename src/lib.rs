#![doc = include_str!("../README.md")]

pub mod account;
pub mod basket;
pub mod economy;
pub mod exchange;
pub mod problems;
pub mod quantity;
pub mod rate;
pub mod search;

pub use account::{Account, AccountError};
pub use basket::Basket;
pub use economy::{
    ApplyError, ApplyResult, EconomicView, Economy, EconomyBuilder, Goal, ReplayResult,
    SimulationResult,
};
pub use exchange::{AccountDelta, Exchange, Receipt, Trace};
pub use quantity::Quantity;
pub use rate::{LinearInvariant, Rate, basket};
pub use search::{SearchSolution, best_first, bfs};
