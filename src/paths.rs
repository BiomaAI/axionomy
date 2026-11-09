//! The kernel's types with a [`Path`](crate::path::Path) for every identifier:
//! an economy written down entirely as data, by remote clients, by storage, or
//! by a problem defined without Rust types of its own.

use crate::{AccountPath, AssetPath, Ontology, RatePath, RolePath};

/// The ontology of economies whose identifiers are all paths.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Paths {}

impl Ontology for Paths {
    const NAME: &'static str = "paths";
    type Account = AccountPath;
    type Asset = AssetPath;
    type Rate = RatePath;
    type Role = RolePath;
}

pub type Economy = crate::Economy<Paths>;
pub type EconomyBuilder = crate::EconomyBuilder<Paths>;
pub type Account = crate::Account<AssetPath>;
pub type Basket = crate::Basket<AssetPath>;
pub type Rate = crate::Rate<Paths>;
pub type RateCondition = crate::RateCondition<Paths>;
pub type QuantityExpression = crate::QuantityExpression<Paths>;
pub type LinearInvariant = crate::LinearInvariant<AssetPath>;
pub type Exchange = crate::Exchange<Paths>;
pub type Trace = crate::Trace<Paths>;
pub type Receipt = crate::Receipt<Paths>;
pub type AccountDelta = crate::AccountDelta<Paths>;
pub type Assessment = crate::ExchangeAssessment<Paths>;
pub type ApplyError = crate::ApplyError<Paths>;
pub type Goal = crate::Goal<Paths>;
