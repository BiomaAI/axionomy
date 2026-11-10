//! Adds authored entries to an economy. The result is built by the kernel's
//! `EconomyBuilder`, so the kernel decides what a valid economy is; this module
//! only refuses entries that would replace something the economy already has.

use crate::messages::{Direction, EconomyEntries, RateEntries};
use axionomy::InvariantPath;
use axionomy::{AccountPath, AssetPath, RatePath, RolePath, paths};
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
        rate: RatePath,
        direction: Direction,
        role: RolePath,
        asset: AssetPath,
    },
    #[error("invariant `{invariant}` already weights `{asset}`")]
    InvariantWeights {
        invariant: InvariantPath,
        asset: AssetPath,
    },
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
        accounts + rates + invariants
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
        .rate_ids()
        .filter_map(|id| economy.rate(id))
        .map(|rate| {
            1 + crate::read::amounts(rate).count()
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
    accounts + rates + invariants
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
    let mut rates: IndexMap<RatePath, paths::Rate> = base
        .into_iter()
        .flat_map(|economy| {
            economy
                .rate_ids()
                .filter_map(|id| Some((id.clone(), economy.rate(id)?.clone())))
        })
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
    let builder = rates
        .into_iter()
        .fold(builder, |builder, (id, rate)| builder.rate(id, rate));
    let builder = invariants
        .into_values()
        .fold(builder, paths::EconomyBuilder::invariant);
    Ok(builder
        .build()
        .expect("authored IDs are unique keys, so the model has no duplicate identifiers"))
}

fn extend_rate(
    id: &RatePath,
    mut rate: paths::Rate,
    added: RateEntries,
) -> Result<paths::Rate, AuthoringError> {
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
            let basket: paths::Basket = amounts
                .into_iter()
                .filter(|(_, quantity)| !quantity.is_zero())
                .collect();
            // The basket's assets are new to this role, so adding it cannot overflow.
            rate = match direction {
                Direction::Consume => rate.consume(role, basket),
                Direction::Produce => rate.produce(role, basket),
                Direction::Preserve => rate.preserve(role, basket),
            };
        }
    }
    for (left, right) in added.distinct {
        rate = rate.distinct(left, right);
    }
    for computed in added.computed {
        rate = match computed.direction {
            Direction::Consume => {
                rate.consume_computed(computed.role, computed.asset, computed.quantity)
            }
            Direction::Produce => {
                rate.produce_computed(computed.role, computed.asset, computed.quantity)
            }
            Direction::Preserve => {
                rate.preserve_computed(computed.role, computed.asset, computed.quantity)
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
