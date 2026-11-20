//! Economic values a Rhai function can construct and return to Rust. These
//! bindings are shared by package construction and the live engine.
use axionomy::{
    AccountPath, AssetAmount, AssetPath, InvariantPath, PathKind, Quantity, RatePath, RolePath,
    path::Path, paths,
};
use indexmap::{IndexMap, map::Entry};
use rhai::{Array, Engine, EvalAltResult, ImmutableString};

type Failure = Box<EvalAltResult>;

fn identifier<K: PathKind + 'static>(engine: &mut Engine, constructor: &'static str) {
    engine
        .register_type_with_name::<Path<K>>(K::SCHEMA)
        .register_fn(constructor, |name: &str| -> Result<Path<K>, Failure> {
            name.parse()
                .map_err(|error| format!("{} `{name}`: {error}", K::SCHEMA).into())
        })
        .register_fn("to_string", |id: &mut Path<K>| {
            ImmutableString::from(id.as_str())
        })
        .register_fn("==", |a: Path<K>, b: Path<K>| a == b)
        .register_fn("!=", |a: Path<K>, b: Path<K>| a != b);
}

pub fn quantity(value: i64) -> Result<Quantity, Failure> {
    u64::try_from(value)
        .map(Quantity::new)
        .map_err(|_| format!("{value} is negative, and quantities are not").into())
}

/// Records even zero weights until validation, so a repeated zero entry is
/// diagnosed just like a duplicate nonzero weight in a written invariant.
#[derive(Clone)]
pub struct Invariant {
    name: InvariantPath,
    weights: IndexMap<AssetPath, i64>,
}

impl Invariant {
    pub fn finish(self) -> paths::LinearInvariant {
        self.weights.into_iter().fold(
            paths::LinearInvariant::new(self.name),
            |rule, (asset, weight)| rule.weight(asset, weight),
        )
    }

    fn weight(&mut self, asset: AssetPath, weight: i64) -> Result<(), Failure> {
        match self.weights.entry(asset) {
            Entry::Occupied(entry) => {
                Err(format!("invariant {} already weights {}", self.name, entry.key()).into())
            }
            Entry::Vacant(entry) => {
                entry.insert(weight);
                Ok(())
            }
        }
    }
}

pub fn register(engine: &mut Engine) {
    use axionomy::path::kind;
    identifier::<kind::Account>(engine, "account");
    identifier::<kind::Asset>(engine, "asset");
    identifier::<kind::Rate>(engine, "rate");
    identifier::<kind::Role>(engine, "role");
    engine
        .register_type_with_name::<Quantity>("Quantity")
        .register_fn("quantity", quantity)
        .register_fn("quantity", |text: &str| -> Result<Quantity, Failure> {
            text.parse()
                .map_err(|_| format!("`{text}` is not a quantity").into())
        })
        .register_type_with_name::<AssetAmount<AssetPath>>("AssetAmount")
        .register_fn("amount", AssetAmount::<AssetPath>::new)
        .register_fn(
            "amount",
            |asset: AssetPath, value: i64| -> Result<_, Failure> {
                Ok(AssetAmount::new(asset, quantity(value)?))
            },
        )
        .register_type_with_name::<paths::Basket>("Basket")
        .register_fn(
            "basket",
            |entries: Array| -> Result<paths::Basket, Failure> {
                let amounts = entries
                    .into_iter()
                    .map(|entry| {
                        let found = entry.type_name();
                        entry.try_cast::<AssetAmount<AssetPath>>().ok_or_else(|| {
                            Failure::from(format!("basket expects AssetAmount, found {found}"))
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                paths::Basket::try_from_amounts(amounts).map_err(|e| e.to_string().into())
            },
        )
        .register_type_with_name::<Invariant>("InvariantBuilder")
        .register_fn("invariant", |name: &str| -> Result<Invariant, Failure> {
            Ok(Invariant {
                name: name
                    .parse()
                    .map_err(|e| format!("invariant `{name}`: {e}"))?,
                weights: IndexMap::new(),
            })
        })
        .register_fn("weight", Invariant::weight)
        .register_type_with_name::<paths::LinearInvariant>("LinearInvariant")
        .register_fn("finish", Invariant::finish)
        .register_type_with_name::<paths::Goal>("Goal")
        .register_fn("goal", paths::Goal::new)
        .register_fn(
            "require",
            |goal: paths::Goal,
             account: AccountPath,
             amounts: paths::Basket|
             -> Result<_, Failure> {
                if goal.requirements().contains_key(&account) {
                    return Err(format!("goal already has requirements for {account}").into());
                }
                Ok(goal.require(account, amounts))
            },
        )
        .register_type_with_name::<paths::Exchange>("Exchange")
        .register_fn("exchange", |rate: RatePath, units: Quantity| {
            paths::Exchange::new(rate, units)
        })
        .register_fn(
            "exchange",
            |rate: RatePath, units: i64| -> Result<_, Failure> {
                Ok(paths::Exchange::new(rate, quantity(units)?))
            },
        )
        .register_fn(
            "bind",
            |exchange: paths::Exchange,
             role: RolePath,
             account: AccountPath|
             -> Result<_, Failure> {
                if exchange.bindings().contains_key(&role) {
                    return Err(format!("exchange already binds {role}").into());
                }
                Ok(exchange.bind(role, account))
            },
        );
}
