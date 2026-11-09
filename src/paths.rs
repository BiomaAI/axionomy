//! The kernel's types with a [`Path`](crate::path::Path) for every identifier:
//! an economy written down entirely as data, by remote clients, by storage, or
//! by a problem defined without Rust types of its own.

use crate::{AccountPath, AssetPath, Ontology, RatePath, RolePath};

/// The ontology of economies whose identifiers are all paths.
pub enum Paths {}

impl Ontology for Paths {
    const NAME: &'static str = "paths";
    type Account = AccountPath;
    type Asset = AssetPath;
    type Rate = RatePath;
    type Role = RolePath;
}

pub type Economy = crate::Economy<AccountPath, AssetPath, RatePath, RolePath>;
pub type EconomyBuilder = crate::EconomyBuilder<AccountPath, AssetPath, RatePath, RolePath>;
pub type Account = crate::Account<AssetPath>;
pub type Basket = crate::Basket<AssetPath>;
pub type Rate = crate::Rate<RolePath, AssetPath>;
pub type RateCondition = crate::RateCondition<RolePath, AssetPath>;
pub type QuantityExpression = crate::QuantityExpression<RolePath, AssetPath>;
pub type LinearInvariant = crate::LinearInvariant<AssetPath>;
pub type Exchange = crate::Exchange<RatePath, RolePath, AccountPath>;
pub type Trace = crate::Trace<RatePath, RolePath, AccountPath>;
pub type Receipt = crate::Receipt<RatePath, RolePath, AccountPath, AssetPath>;
pub type AccountDelta = crate::AccountDelta<AccountPath, AssetPath>;
pub type Assessment = crate::ExchangeAssessment<AccountPath, AssetPath, RatePath, RolePath>;
pub type ApplyError = crate::ApplyError<RatePath, RolePath, AccountPath, AssetPath>;
pub type Goal = crate::Goal<AccountPath, AssetPath>;
