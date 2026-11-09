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
pub use path::{AccountPath, AssetPath, OntologyPath, PathError, PathKind, RatePath, RolePath};

pub use quantity::{Quantity, QuantityError, Rounding};
pub use rate::{
    ComputedAmount, LinearInvariant, Measure, QuantityComparison, QuantityExpression, Rate,
    RateCondition, basket,
};
use std::{fmt::Debug, hash::Hash};

/// What the kernel needs of an identifier type: values it can clone, compare,
/// order, hash and show. Every account, asset, rate and role type is one.
pub trait Identifier: Clone + Eq + Ord + Hash + Debug {}

impl<T: Clone + Eq + Ord + Hash + Debug> Identifier for T {}

/// The vocabulary of one kind of economy, under one name: who holds
/// (accounts), what is held (assets), the laws by which holdings change
/// (rates), and the parts of a law an exchange binds to accounts (roles).
///
/// Every economy, exchange, receipt, trace, goal and rate is of an ontology:
/// an [`Economy<O>`](Economy) holds `O::Account`s with balances of
/// `O::Asset`s and applies the rates it names by `O::Rate`, binding their
/// `O::Role`s to accounts. The four stay distinct types, so an account can
/// never stand where an asset or a role is expected.
///
/// An ontology is a marker type, usually an empty enum, and is never a value.
/// It derives the standard traits so that the kernel's types can derive
/// theirs:
///
/// ```
/// use axionomy::{Account, EconomyBuilder, Exchange, Ontology, Quantity, Rate, basket};
///
/// #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
/// enum Bakery {}
///
/// #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
/// enum Holder { Baker }
/// #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
/// enum Good { Flour, Bread }
/// #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
/// enum Law { Bake }
/// #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
/// enum Part { Oven }
///
/// impl Ontology for Bakery {
///     const NAME: &'static str = "bakery";
///     type Account = Holder;
///     type Asset = Good;
///     type Rate = Law;
///     type Role = Part;
/// }
///
/// let mut economy = EconomyBuilder::<Bakery>::new()
///     .account(Holder::Baker, Account::from(basket([(Good::Flour, 2)])))
///     .rate(
///         Law::Bake,
///         Rate::new()
///             .consume(Part::Oven, basket([(Good::Flour, 1)]))
///             .produce(Part::Oven, basket([(Good::Bread, 1)])),
///     )
///     .build()
///     .unwrap();
/// let bake = Exchange::new(Law::Bake, Quantity::new(2)).bind(Part::Oven, Holder::Baker);
/// economy.apply(bake).unwrap();
/// assert_eq!(economy.balance(&Holder::Baker, &Good::Bread), Quantity::new(2));
/// ```
///
/// The name is a path, checked when the ontology is used: storage records it
/// with every run and reads a run back only as the ontology that recorded it.
/// [`OntologyPath::of`] gives it as a typed path.
pub trait Ontology: Copy + Debug + Ord + Hash + Send + Sync + 'static {
    /// The ontology's name, a path such as `maze` or `logistics/fleet`.
    const NAME: &'static str;
    type Account: Identifier;
    type Asset: Identifier;
    type Rate: Identifier;
    type Role: Identifier;
}
