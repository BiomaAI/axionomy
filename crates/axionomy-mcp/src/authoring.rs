//! Adds authored entries to an economy. The result is built by the kernel's
//! `EconomyBuilder`, so the kernel decides what a valid economy is; this module
//! only refuses entries that would replace something the economy already has.

use crate::messages::{Direction, EconomyEntries, RateEntries};
use axionomy::InvariantPath;
use axionomy::{AccountPath, AssetPath, Pattern, RatePath, RolePath, Segment, VariablePath, paths};
use indexmap::IndexMap;
use std::collections::{BTreeMap, HashSet};
use thiserror::Error;

/// Entries one call may add.
pub(crate) const MAX_ENTRIES: usize = 10_000;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub(crate) enum AuthoringError {
    #[error(
        "{entries} entries exceed the limit of {MAX_ENTRIES} per call; add the rest with axionomy_economy_extend"
    )]
    TooLarge { entries: usize },
    #[error("account `{account}` already holds `{asset}`")]
    AccountHolds {
        account: AccountPath,
        asset: AssetPath,
    },
    #[error("rate `{rate}` already has a {direction} amount of `{asset}` for role `{role}`")]
    RateHas {
        rate: Pattern<RatePath>,
        direction: Direction,
        role: RolePath,
        asset: Pattern<AssetPath>,
    },
    #[error("rate `{rate}` already declares the variable `{variable}`")]
    RateDeclares {
        rate: Pattern<RatePath>,
        variable: VariablePath,
    },
    #[error("rate `{rate}` already fixes the account of role `{role}`")]
    RateFixes {
        rate: Pattern<RatePath>,
        role: RolePath,
    },
    #[error("the economy already names the constant `{constant}`")]
    ConstantNamed { constant: VariablePath },
    #[error("the entries make no economy: {0}")]
    Model(String),
    #[error("invariant `{invariant}` already weights `{asset}`")]
    InvariantWeights {
        invariant: InvariantPath,
        asset: AssetPath,
    },
}

impl From<axionomy_authoring::model::Model> for EconomyEntries {
    fn from(model: axionomy_authoring::model::Model) -> Self {
        Self {
            accounts: model
                .accounts
                .into_iter()
                .map(|(account, opening)| {
                    (
                        account,
                        opening
                            .balances()
                            .iter()
                            .map(|(a, q)| (a.clone(), *q))
                            .collect(),
                    )
                })
                .collect(),
            constants: model.constants.into_iter().collect(),
            invariants: model
                .invariants
                .into_iter()
                .map(|(id, rule)| (id, rule.weights().map(|(a, w)| (a.clone(), w)).collect()))
                .collect(),
            rates: model
                .rates
                .into_iter()
                .map(|(id, law)| (id, RateEntries::from(law)))
                .collect(),
        }
    }
}

impl From<paths::Rate> for RateEntries {
    fn from(law: paths::Rate) -> Self {
        let amounts = |get: for<'a> fn(
            &'a paths::Rate,
            &RolePath,
        )
            -> Option<&'a axionomy::Basket<Pattern<AssetPath>>>| {
            law.roles()
                .filter_map(|role| {
                    Some((
                        role.clone(),
                        get(&law, role)?
                            .iter()
                            .map(|(a, q)| (a.clone(), *q))
                            .collect(),
                    ))
                })
                .collect()
        };
        Self {
            variables: law.variables().to_vec(),
            distinct_variables: law.distinct_variable_pairs().cloned().collect(),
            relations: law.relations().to_vec(),
            derived: law.derived().to_vec(),
            accounts: law
                .fixed_accounts()
                .map(|(r, a)| (r.clone(), a.clone()))
                .collect(),
            consume: amounts(paths::Rate::consumed),
            produce: amounts(paths::Rate::produced),
            preserve: amounts(paths::Rate::preserved),
            distinct: law.distinct_roles().cloned().collect(),
            computed: [
                (Direction::Consume, law.computed_consumed()),
                (Direction::Produce, law.computed_produced()),
                (Direction::Preserve, law.computed_preserved()),
            ]
            .into_iter()
            .flat_map(|(direction, entries)| {
                entries.iter().map(move |c| crate::messages::ComputedEntry {
                    direction,
                    role: c.role().clone(),
                    asset: c.asset().clone(),
                    quantity: c.quantity().clone(),
                })
            })
            .collect(),
            conditions: law.conditions().to_vec(),
            opens: law.opened_roles().cloned().collect(),
            closes: law.closed_roles().cloned().collect(),
        }
    }
}

impl EconomyEntries {
    /// Counts every ID and every amount, condition or role pair, so a call's
    /// size is bounded however its entries are nested.
    pub(crate) fn len(&self) -> usize {
        let accounts: usize = self
            .accounts
            .values()
            .map(|amounts| 1 + amounts.len())
            .sum();
        let rates: usize = self.rates.values().map(RateEntries::len).sum();
        let invariants: usize = self
            .invariants
            .values()
            .map(|weights| 1 + weights.len())
            .sum();
        accounts + self.constants.len() + rates + invariants
    }
}

impl RateEntries {
    fn len(&self) -> usize {
        let amounts = [&self.consume, &self.produce, &self.preserve]
            .into_iter()
            .flat_map(BTreeMap::values)
            .map(BTreeMap::len)
            .sum::<usize>();
        1 + amounts
            + self.variables.len()
            + self.distinct_variables.len()
            + self.relations.len()
            + self.derived.len()
            + self.accounts.len()
            + self.distinct.len()
            + self.computed.len()
            + self.conditions.len()
            + self.opens.len()
            + self.closes.len()
    }
}

/// How many entries authoring `economy` from nothing takes, counted as
/// [`EconomyEntries::len`] counts a call's.
pub(crate) fn entries(economy: &paths::Economy) -> usize {
    let accounts: usize = economy
        .accounts()
        .map(|(_, account)| 1 + account.balances().len())
        .sum();
    let rates: usize = economy
        .rates()
        .map(|(_, rate)| {
            1 + crate::read::amounts(rate).count()
                + rate.variables().len()
                + rate.distinct_variable_pairs().count()
                + rate.relations().len()
                + rate.derived().len()
                + rate.fixed_accounts().count()
                + rate.distinct_roles().count()
                + rate.conditions().len()
                + rate.opened_roles().count()
                + rate.closed_roles().count()
        })
        .sum();
    let invariants: usize = economy
        .invariants()
        .map(|invariant| 1 + invariant.weights().count())
        .sum();
    accounts + economy.constants().count() + rates + invariants
}

/// The economy `base` becomes with `entries` added; `None` starts from an empty
/// economy.
pub(crate) fn extend(
    base: Option<&paths::Economy>,
    entries: EconomyEntries,
) -> Result<paths::Economy, AuthoringError> {
    let size = entries.len();
    if size > MAX_ENTRIES {
        return Err(AuthoringError::TooLarge { entries: size });
    }

    let mut accounts: IndexMap<AccountPath, paths::Account> = base
        .into_iter()
        .flat_map(paths::Economy::accounts)
        .map(|(id, account)| (id.clone(), account.clone()))
        .collect();
    let mut constants: BTreeMap<VariablePath, Segment> = base
        .into_iter()
        .flat_map(paths::Economy::constants)
        .map(|(name, value)| (name.clone(), value.clone()))
        .collect();
    let mut rates: IndexMap<Pattern<RatePath>, paths::Rate> = base
        .into_iter()
        .flat_map(paths::Economy::rates)
        .map(|(id, rate)| (id.clone(), rate.clone()))
        .collect();
    let mut invariants: IndexMap<InvariantPath, paths::LinearInvariant> = base
        .into_iter()
        .flat_map(paths::Economy::invariants)
        .map(|invariant| (invariant.name().clone(), invariant.clone()))
        .collect();

    for (id, amounts) in entries.accounts {
        let account = accounts.entry(id.clone()).or_default();
        let mut balances = account.balances().clone();
        for (asset, quantity) in amounts {
            if balances.get(&asset).is_some() {
                return Err(AuthoringError::AccountHolds { account: id, asset });
            }
            balances.insert(asset, quantity);
        }
        *account = paths::Account::new(balances);
    }

    for (name, value) in entries.constants {
        if constants.contains_key(&name) {
            return Err(AuthoringError::ConstantNamed { constant: name });
        }
        constants.insert(name, value);
    }

    for (id, added) in entries.rates {
        let rate = rates.shift_remove(&id).unwrap_or_default();
        let rate = extend_rate(&id, rate, added)?;
        rates.insert(id, rate);
    }

    for (path, weights) in entries.invariants {
        let mut invariant = invariants
            .shift_remove(&path)
            .unwrap_or_else(|| paths::LinearInvariant::new(path.clone()));
        let weighted: HashSet<AssetPath> = invariant
            .weights()
            .map(|(asset, _)| asset.clone())
            .collect();
        for (asset, weight) in weights {
            if weighted.contains(&asset) {
                return Err(AuthoringError::InvariantWeights {
                    invariant: path,
                    asset,
                });
            }
            invariant = invariant.weight(asset, weight);
        }
        invariants.insert(path, invariant);
    }

    let builder = accounts
        .into_iter()
        .fold(paths::EconomyBuilder::new(), |builder, (id, account)| {
            builder.account(id, account)
        });
    let builder = constants
        .into_iter()
        .fold(builder, |builder, (name, value)| {
            builder.constant_value(name, value)
        });
    let builder = rates
        .into_iter()
        .fold(builder, |builder, (id, rate)| builder.rate_at(id, rate));
    let builder = invariants
        .into_values()
        .fold(builder, paths::EconomyBuilder::invariant);
    builder
        .build()
        .map_err(|error| AuthoringError::Model(error.to_string()))
}

fn extend_rate(
    id: &Pattern<RatePath>,
    mut rate: paths::Rate,
    added: RateEntries,
) -> Result<paths::Rate, AuthoringError> {
    for variable in added.variables {
        if rate
            .variables()
            .iter()
            .any(|declared| declared.name() == variable.name())
        {
            return Err(AuthoringError::RateDeclares {
                rate: id.clone(),
                variable: variable.name().clone(),
            });
        }
        rate = rate.variable(variable);
    }
    for (left, right) in added.distinct_variables {
        rate = rate.distinct_variables(left, right);
    }
    for relation in added.relations {
        rate = rate.relation(relation);
    }
    for derived in added.derived {
        rate = rate.derive(derived.name().clone(), derived.value().clone());
    }
    for (role, account) in added.accounts {
        if rate.fixed_accounts().any(|(fixed, _)| *fixed == role) {
            return Err(AuthoringError::RateFixes {
                rate: id.clone(),
                role,
            });
        }
        rate = rate.account(role, account);
    }
    let statics = [
        (Direction::Consume, added.consume),
        (Direction::Produce, added.produce),
        (Direction::Preserve, added.preserve),
    ];
    for (direction, roles) in statics {
        for (role, amounts) in roles {
            let existing = match direction {
                Direction::Consume => rate.consumed(&role),
                Direction::Produce => rate.produced(&role),
                Direction::Preserve => rate.preserved(&role),
            };
            if let Some(asset) = amounts
                .keys()
                .find(|asset| existing.is_some_and(|basket| basket.get(*asset).is_some()))
            {
                return Err(AuthoringError::RateHas {
                    rate: id.clone(),
                    direction,
                    role,
                    asset: asset.clone(),
                });
            }
            let entries = amounts
                .into_iter()
                .filter(|(_, quantity)| !quantity.is_zero());
            // The entries are new to this role, so adding them cannot overflow.
            rate = match direction {
                Direction::Consume => rate.consume_at(role, entries),
                Direction::Produce => rate.produce_at(role, entries),
                Direction::Preserve => rate.preserve_at(role, entries),
            };
        }
    }
    for (left, right) in added.distinct {
        rate = rate.distinct(left, right);
    }
    for computed in added.computed {
        rate = match computed.direction {
            Direction::Consume => {
                rate.consume_computed_at(computed.role, computed.asset, computed.quantity)
            }
            Direction::Produce => {
                rate.produce_computed_at(computed.role, computed.asset, computed.quantity)
            }
            Direction::Preserve => {
                rate.preserve_computed_at(computed.role, computed.asset, computed.quantity)
            }
        };
    }
    for role in added.opens {
        rate = rate.opens(role);
    }
    for role in added.closes {
        rate = rate.closes(role);
    }
    for condition in added.conditions {
        rate = rate.condition(
            condition.name().clone(),
            condition.left().clone(),
            condition.comparison(),
            condition.right().clone(),
        );
    }
    Ok(rate)
}
