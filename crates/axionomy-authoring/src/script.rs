//! Economic values a Rhai function can construct and return to Rust. These
//! bindings are shared by package construction and the live engine.
use axionomy::{
    AccountPath, AssetAmount, AssetPath, InvariantPath, PathKind, Quantity, RatePath, RolePath,
    Rounding, path::Path, paths,
};
use indexmap::{IndexMap, map::Entry};
use rhai::{Array, Engine, EvalAltResult, ImmutableString, Map};

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

fn exchange_with_bindings(
    rate: &str,
    bindings: Map,
    units: Quantity,
) -> Result<paths::Exchange, Failure> {
    let mut exchange = paths::Exchange::new(
        rate.parse().map_err(|e| format!("rate `{rate}`: {e}"))?,
        units,
    );
    for (role, account) in bindings {
        let role = role.parse().map_err(|e| format!("role `{role}`: {e}"))?;
        let account = if account.is::<AccountPath>() {
            account.cast::<AccountPath>()
        } else {
            let name = account
                .into_immutable_string()
                .map_err(|kind| format!("expected account path, found {kind}"))?;
            name.parse().map_err(|e| format!("account `{name}`: {e}"))?
        };
        exchange = exchange.bind(role, account);
    }
    Ok(exchange)
}

pub fn register(engine: &mut Engine) {
    use axionomy::path::kind;
    quantities(engine);
    for (name, comparison) in [
        ("balance_equals", axionomy::QuantityComparison::Equal),
        (
            "balance_at_least",
            axionomy::QuantityComparison::GreaterThanOrEqual,
        ),
        (
            "balance_at_most",
            axionomy::QuantityComparison::LessThanOrEqual,
        ),
    ] {
        engine.register_fn(
            name,
            move |account: AccountPath, asset: AssetPath, quantity: Quantity| BalanceCondition {
                account,
                asset,
                comparison,
                quantity,
            },
        );
    }
    engine.register_type_with_name::<BalanceCondition>("BalanceCondition");
    crate::model::register(engine);
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
        .register_fn("exchange", exchange_with_bindings)
        .register_fn("exchange", |rate: &str, bindings: Map, units: i64| {
            exchange_with_bindings(rate, bindings, quantity(units)?)
        })
        .register_fn(
            "parameter",
            |exchange: paths::Exchange, name: &str, value: Quantity| -> Result<_, Failure> {
                Ok(exchange.parameter(
                    name.parse()
                        .map_err(|e| format!("parameter `{name}`: {e}"))?,
                    value,
                ))
            },
        )
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

fn checked(value: Option<Quantity>, what: &str) -> Result<Quantity, Failure> {
    value.ok_or_else(|| format!("{what} is out of range").into())
}

fn rounding(name: &str) -> Result<Rounding, Failure> {
    match name {
        "floor" => Ok(Rounding::Floor),
        "ceil" => Ok(Rounding::Ceil),
        other => Err(format!("rounding `{other}` is neither `floor` nor `ceil`").into()),
    }
}

fn quantities(rhai: &mut rhai::Engine) {
    rhai.register_fn("to_string", |q: &mut Quantity| q.to_string())
        .register_fn("to_debug", |q: &mut Quantity| q.to_string())
        .register_fn("is_zero", |q: &mut Quantity| q.is_zero())
        .register_fn("to_int", |q: &mut Quantity| -> Result<i64, Failure> {
            i64::try_from(u64::try_from(*q).map_err(|_| Failure::from("too large"))?)
                .map_err(|_| format!("{q} does not fit an integer").into())
        })
        .register_fn(
            "mul_div",
            |a: Quantity, b: Quantity, c: Quantity, round: &str| {
                checked(a.checked_mul_div(b, c, rounding(round)?), "a mul_div")
            },
        )
        .register_fn("min", |a: Quantity, b: Quantity| a.min(b))
        .register_fn("max", |a: Quantity, b: Quantity| a.max(b));
    macro_rules! arithmetic {
        ($($operator:literal => $method:ident),*) => {$(
            rhai.register_fn($operator, |a: Quantity, b: Quantity| {
                checked(a.$method(b), concat!("a quantity ", $operator))
            });
            rhai.register_fn($operator, |a: Quantity, b: i64| {
                checked(a.$method(quantity(b)?), concat!("a quantity ", $operator))
            });
            rhai.register_fn($operator, |a: i64, b: Quantity| {
                checked(quantity(a)?.$method(b), concat!("a quantity ", $operator))
            });
        )*};
    }
    arithmetic!("+" => checked_add, "-" => checked_sub, "*" => checked_mul);
    macro_rules! comparisons {
        ($($operator:literal => $compare:expr),*) => {$(
            rhai.register_fn($operator, |a: Quantity, b: Quantity| $compare(a, b));
            rhai.register_fn($operator, |a: Quantity, b: i64| -> Result<bool, Failure> {
                Ok($compare(a, quantity(b)?))
            });
            rhai.register_fn($operator, |a: i64, b: Quantity| -> Result<bool, Failure> {
                Ok($compare(quantity(a)?, b))
            });
        )*};
    }
    comparisons!(
        "==" => |a: Quantity, b: Quantity| a == b,
        "!=" => |a: Quantity, b: Quantity| a != b,
        "<" => |a: Quantity, b: Quantity| a < b,
        "<=" => |a: Quantity, b: Quantity| a <= b,
        ">" => |a: Quantity, b: Quantity| a > b,
        ">=" => |a: Quantity, b: Quantity| a >= b
    );
}

#[derive(Clone)]
pub struct BalanceCondition {
    pub account: AccountPath,
    pub asset: AssetPath,
    pub comparison: axionomy::QuantityComparison,
    pub quantity: Quantity,
}
