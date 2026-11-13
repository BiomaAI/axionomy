//! Scripts: the Rhai source a package carries for anything a generic
//! algorithm cannot know (D-049): candidates, policies, Nature responses,
//! heuristics, objectives, view logic.
//!
//! A script reads an economy through the host API and proposes exchanges; it
//! never changes one. Quantities are exact ([`Quantity`], with checked
//! arithmetic), and the only randomness is a [`Random`] the host seeds, so a
//! script gives the same answer for the same arguments everywhere. Rhai's
//! maps iterate in key order, and scripts have no clock.

use crate::EngineError;
use axionomy::{
    AccountPath, AssessmentStatus, AssetPath, PathKind, Quantity, RatePath, RolePath, Rounding,
    path::Path, paths,
};
use axionomy_search::sampling::splitmix;
use rhai::{Array, Dynamic, EvalAltResult, ImmutableString, Map};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

/// The version of the host API this engine gives scripts. A package records
/// the version its scripts were written against.
pub const HOST_API: u64 = 1;

/// The most operations one call of a script may run before it is stopped.
const MAX_OPERATIONS: u64 = 50_000_000;
/// The deepest a script's expressions may nest at its top level; half that
/// inside its functions.
const MAX_DEPTH: usize = 256;

/// The kind of path a script is named by.
pub mod kind {
    use axionomy::PathKind;

    /// A script a package carries.
    pub enum Script {}

    impl PathKind for Script {
        const SCHEMA: &'static str = "ScriptPath";
        const EXAMPLE: &'static str = "maze/heuristic";
    }
}

/// A script's name, such as `maze/heuristic`.
pub type ScriptPath = Path<kind::Script>;

type Failure = Box<EvalAltResult>;

/// An economy as a script reads it: all of it, or only the accounts an actor
/// may see.
#[derive(Clone)]
pub struct ScriptState {
    economy: paths::Economy,
    visible: Option<Arc<BTreeSet<AccountPath>>>,
}

impl ScriptState {
    pub fn new(economy: paths::Economy) -> Self {
        Self {
            economy,
            visible: None,
        }
    }

    pub fn economy(&self) -> &paths::Economy {
        &self.economy
    }

    fn sees(&self, account: &AccountPath) -> bool {
        self.visible
            .as_ref()
            .is_none_or(|visible| visible.contains(account))
    }

    /// The whole economy, refusing a view: whether an exchange applies may
    /// depend on accounts an actor cannot see.
    fn whole(&self, asked: &str) -> Result<&paths::Economy, Failure> {
        match self.visible {
            None => Ok(&self.economy),
            Some(_) => Err(format!("{asked} is not observable from a view").into()),
        }
    }
}

/// The host's seeded random numbers: the only randomness a script has.
/// Clones share one sequence, so a generator passed to a script call after
/// call keeps drawing where it left off.
#[derive(Debug, Clone)]
pub struct Random {
    seed: u64,
    drawn: Arc<AtomicU64>,
}

impl Random {
    pub fn new(seed: u64) -> Self {
        Self {
            seed,
            drawn: Arc::new(AtomicU64::new(0)),
        }
    }

    /// A number below `bound`, the next of this generator's sequence.
    fn below(&self, bound: u64) -> u64 {
        let value = splitmix(self.seed, self.drawn.fetch_add(1, Ordering::Relaxed));
        // The high half of a 128-bit product: uniform enough, and exact.
        ((u128::from(value) * u128::from(bound)) >> 64) as u64
    }
}

/// The Rhai engine with the host API registered, shared by every call.
pub(crate) fn host() -> rhai::Engine {
    let mut rhai = rhai::Engine::new();
    rhai.set_max_operations(MAX_OPERATIONS);
    // Rhai's defaults reject the nesting an ordinary policy reaches.
    rhai.set_max_expr_depths(MAX_DEPTH, MAX_DEPTH / 2);
    rhai.on_print(|_| {});
    rhai.on_debug(|_, _, _| {});
    quantities(&mut rhai);
    states(&mut rhai);
    exchanges(&mut rhai);
    rhai.register_type_with_name::<Random>("Random")
        .register_fn("next", |random: &mut Random, bound: i64| {
            let bound = u64::try_from(bound)
                .ok()
                .filter(|bound| *bound > 0)
                .ok_or_else(|| Failure::from(format!("{bound} is not a positive bound")))?;
            Ok::<_, Failure>(random.below(bound) as i64)
        });
    rhai.register_fn("distances", distances);
    rhai
}

fn quantity_of(value: i64) -> Result<Quantity, Failure> {
    u64::try_from(value)
        .map(Quantity::new)
        .map_err(|_| format!("{value} is negative, and quantities are not").into())
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
    rhai.register_type_with_name::<Quantity>("Quantity")
        .register_fn("quantity", quantity_of)
        .register_fn("quantity", |text: &str| -> Result<Quantity, Failure> {
            text.parse()
                .map_err(|_| format!("`{text}` is not a quantity").into())
        })
        .register_fn("to_string", |q: &mut Quantity| q.to_string())
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
                checked(a.$method(quantity_of(b)?), concat!("a quantity ", $operator))
            });
            rhai.register_fn($operator, |a: i64, b: Quantity| {
                checked(quantity_of(a)?.$method(b), concat!("a quantity ", $operator))
            });
        )*};
    }
    arithmetic!("+" => checked_add, "-" => checked_sub, "*" => checked_mul);
    macro_rules! comparisons {
        ($($operator:literal => $compare:expr),*) => {$(
            rhai.register_fn($operator, |a: Quantity, b: Quantity| $compare(a, b));
            rhai.register_fn($operator, |a: Quantity, b: i64| -> Result<bool, Failure> {
                Ok($compare(a, quantity_of(b)?))
            });
            rhai.register_fn($operator, |a: i64, b: Quantity| -> Result<bool, Failure> {
                Ok($compare(quantity_of(a)?, b))
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

fn parsed<K: PathKind>(text: &str) -> Result<Path<K>, Failure> {
    text.parse()
        .map_err(|_| format!("`{text}` is not a {}", K::SCHEMA).into())
}

fn states(rhai: &mut rhai::Engine) {
    rhai.register_type_with_name::<ScriptState>("State")
        .register_fn(
            "balance",
            |state: &mut ScriptState, account: &str, asset: &str| -> Result<Quantity, Failure> {
                let account: AccountPath = parsed(account)?;
                let asset: AssetPath = parsed(asset)?;
                Ok(if state.sees(&account) {
                    state.economy.balance(&account, &asset)
                } else {
                    Quantity::ZERO
                })
            },
        )
        .register_fn(
            "has_account",
            |state: &mut ScriptState, account: &str| -> Result<bool, Failure> {
                let account: AccountPath = parsed(account)?;
                Ok(state.sees(&account) && state.economy.account(&account).is_some())
            },
        )
        .register_fn(
            "accounts",
            |state: &mut ScriptState, prefix: &str| -> Array {
                state
                    .economy
                    .accounts()
                    .map(|(account, _)| account)
                    .filter(|account| state.sees(account) && account.as_str().starts_with(prefix))
                    .map(|account| Dynamic::from(account.to_string()))
                    .collect()
            },
        )
        .register_fn(
            "balances",
            |state: &mut ScriptState, account: &str| -> Result<Map, Failure> {
                let account: AccountPath = parsed(account)?;
                let Some(holder) = state
                    .economy
                    .account(&account)
                    .filter(|_| state.sees(&account))
                else {
                    return Ok(Map::new());
                };
                Ok(holder
                    .balances()
                    .iter()
                    .map(|(asset, quantity)| (asset.as_str().into(), Dynamic::from(*quantity)))
                    .collect())
            },
        )
        .register_fn("rates", |state: &mut ScriptState, prefix: &str| -> Array {
            state
                .economy
                .rate_ids()
                .filter(|rate| rate.as_str().starts_with(prefix))
                .map(|rate| Dynamic::from(rate.to_string()))
                .collect()
        })
        .register_fn(
            "roles",
            |state: &mut ScriptState, rate: &str| -> Result<Array, Failure> {
                let rate: RatePath = parsed(rate)?;
                let definition = state
                    .economy
                    .rate(&rate)
                    .ok_or_else(|| Failure::from(format!("the economy has no rate `{rate}`")))?;
                Ok(definition
                    .roles()
                    .map(|role| Dynamic::from(role.to_string()))
                    .collect())
            },
        )
        .register_fn(
            "view",
            |state: &mut ScriptState, accounts: Array| -> Result<ScriptState, Failure> {
                let visible = accounts
                    .into_iter()
                    .map(|account| parsed(&text(account)?))
                    .collect::<Result<BTreeSet<_>, _>>()?;
                Ok(ScriptState {
                    economy: state.economy.clone(),
                    visible: Some(Arc::new(visible)),
                })
            },
        )
        .register_fn(
            "applicable",
            |state: &mut ScriptState, exchange: paths::Exchange| -> Result<bool, Failure> {
                Ok(state
                    .whole("whether an exchange applies")?
                    .is_applicable(&exchange))
            },
        )
        .register_fn(
            "assess",
            |state: &mut ScriptState, exchange: paths::Exchange| -> Result<Map, Failure> {
                let assessment = state.whole("an assessment")?.assess(&exchange);
                let status = match assessment.status() {
                    AssessmentStatus::Applicable => "applicable",
                    AssessmentStatus::Infeasible => "infeasible",
                    AssessmentStatus::Invalid => "invalid",
                };
                let missing: Array = assessment
                    .shortfalls()
                    .iter()
                    .flat_map(|shortfall| {
                        shortfall.missing().iter().map(move |(asset, quantity)| {
                            let mut line = Map::new();
                            line.insert("account".into(), shortfall.account().to_string().into());
                            line.insert("asset".into(), asset.to_string().into());
                            line.insert("quantity".into(), Dynamic::from(*quantity));
                            Dynamic::from(line)
                        })
                    })
                    .collect();
                let mut result = Map::new();
                result.insert("status".into(), status.into());
                result.insert("missing".into(), missing.into());
                Ok(result)
            },
        );
}

fn text(value: Dynamic) -> Result<String, Failure> {
    value
        .into_immutable_string()
        .map(|text| text.to_string())
        .map_err(|kind| format!("expected a path, found {kind}").into())
}

fn exchanges(rhai: &mut rhai::Engine) {
    fn exchange(rate: &str, bindings: Map, units: Quantity) -> Result<paths::Exchange, Failure> {
        let mut exchange = paths::Exchange::new(parsed(rate)?, units);
        for (role, account) in bindings {
            let role: RolePath = parsed(role.as_str())?;
            exchange = exchange.bind(role, parsed(&text(account)?)?);
        }
        Ok(exchange)
    }
    rhai.register_type_with_name::<paths::Exchange>("Exchange")
        .register_fn("exchange", exchange)
        .register_fn("exchange", |rate: &str, bindings: Map, units: i64| {
            exchange(rate, bindings, quantity_of(units)?)
        })
        .register_fn(
            "parameter",
            |exchange: paths::Exchange, name: &str, value: Quantity| -> Result<_, Failure> {
                Ok(exchange.parameter(parsed(name)?, value))
            },
        )
        .register_fn("rate", |exchange: &mut paths::Exchange| {
            ImmutableString::from(exchange.rate().as_str())
        })
        .register_fn("units", |exchange: &mut paths::Exchange| exchange.units())
        .register_fn(
            "account",
            |exchange: &mut paths::Exchange, role: &str| -> Result<Dynamic, Failure> {
                let role: RolePath = parsed(role)?;
                Ok(exchange
                    .bindings()
                    .get(&role)
                    .map_or(Dynamic::UNIT, |account| account.to_string().into()))
            },
        )
        .register_fn("to_string", |exchange: &mut paths::Exchange| {
            exchange.rate().to_string()
        })
        .register_fn("==", |a: paths::Exchange, b: paths::Exchange| a == b)
        .register_fn("!=", |a: paths::Exchange, b: paths::Exchange| a != b);
}

/// The fewest edges from `from` to every node it reaches, over undirected
/// `edges` given as pairs of names.
fn distances(edges: Array, from: &str) -> Result<Map, Failure> {
    let mut neighbours: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for edge in edges {
        let pair: Array = edge
            .try_cast()
            .ok_or_else(|| Failure::from("an edge is not a pair"))?;
        let [a, b]: [Dynamic; 2] = pair
            .try_into()
            .map_err(|_| Failure::from("an edge is not a pair"))?;
        let (a, b) = (text(a)?, text(b)?);
        neighbours.entry(a.clone()).or_default().push(b.clone());
        neighbours.entry(b).or_default().push(a);
    }
    let mut reached = Map::new();
    reached.insert(from.into(), Dynamic::from(0_i64));
    let mut pending = VecDeque::from([(from.to_owned(), 0_i64)]);
    while let Some((at, distance)) = pending.pop_front() {
        for next in neighbours.get(&at).into_iter().flatten() {
            if !reached.contains_key(next.as_str()) {
                reached.insert(next.as_str().into(), Dynamic::from(distance + 1));
                pending.push_back((next.clone(), distance + 1));
            }
        }
    }
    Ok(reached)
}

/// The exchanges in what a script returned.
pub fn exchanges_of(returned: Array) -> Result<Vec<paths::Exchange>, EngineError> {
    returned
        .into_iter()
        .map(|value| {
            let kind = value.type_name();
            value
                .try_cast::<paths::Exchange>()
                .ok_or_else(|| EngineError::Returned {
                    expected: "an exchange",
                    found: kind.to_owned(),
                })
        })
        .collect()
}
