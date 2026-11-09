//! Economies as trees: one canonical tree per state, whose keys say what each
//! entry holds.
//!
//! | Key | Value |
//! | --- | --- |
//! | `a` account | empty: the account exists |
//! | `b` account `\0` asset | the balance, 32 bytes big-endian, never zero |
//! | `r` rate | the rate, as the kernel serializes it |
//! | `v` invariant, as the kernel serializes it | empty |
//!
//! Every identifier is written as its JSON. JSON never contains `\0`, which
//! sorts before every other byte, so an account's balances are contiguous and
//! in the same order as the accounts; and a JSON string sorts as its text
//! does, so the accounts of a [`Paths`](axionomy::paths::Paths) economy are in
//! path order. The tree
//! holds the economy's logical state, not the order its accounts were created
//! in: a rebuilt economy lists accounts in key order.

use crate::ontology::{Id, Recordable};
use axionomy::{AccountPath, AssetPath, Economy, Quantity, Receipt};
use axionomy_tree::Tree;
use serde::{Serialize, de::DeserializeOwned};
use std::ops::Range;
use thiserror::Error;

const ACCOUNT: u8 = b'a';
const BALANCE: u8 = b'b';
const RATE: u8 = b'r';
const INVARIANT: u8 = b'v';

/// Why a stored tree is not an economy's state.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("a stored state is not an economy: {0}")]
pub struct StateError(&'static str);

/// The tree of an economy's state.
pub fn tree<O: Recordable>(economy: &Economy<O>) -> Tree {
    let mut entries = Vec::new();
    for (account, holdings) in economy.accounts() {
        entries.push((account_key(account), Vec::new()));
        for (asset, quantity) in holdings.balances().iter() {
            entries.push((balance_key(account, asset), quantity.to_be_bytes().to_vec()));
        }
    }
    for rate in economy.rate_ids() {
        let definition = economy.rate(rate).expect("listed rates exist");
        entries.push((key(RATE, rate), json(definition)));
    }
    for invariant in economy.invariants() {
        entries.push((key(INVARIANT, invariant), Vec::new()));
    }
    entries.into_iter().collect()
}

/// Updates `tree` from the state before `receipt`'s exchange to `economy`,
/// the state after it. The receipt names every balance and account that
/// changed; their new values come from the kernel.
pub fn advance<O: Recordable>(tree: &mut Tree, economy: &Economy<O>, receipt: &Receipt<O>) {
    let mut edits: Vec<(Vec<u8>, Option<Vec<u8>>)> = receipt
        .opened()
        .iter()
        .map(|account| (account_key(account), Some(Vec::new())))
        .collect();
    for delta in receipt.deltas() {
        let account = delta.account();
        for (asset, _) in delta.consumed().iter().chain(delta.produced().iter()) {
            let quantity = economy.balance(account, asset);
            edits.push((
                balance_key(account, asset),
                (!quantity.is_zero()).then(|| quantity.to_be_bytes().to_vec()),
            ));
        }
    }
    // Last, so an account opened and closed by one exchange ends absent.
    edits.extend(
        receipt
            .closed()
            .iter()
            .map(|account| (account_key(account), None)),
    );
    tree.update(edits);
}

/// The economy whose state `tree` holds. Only a tree that [`tree`] could have
/// produced is accepted.
pub fn economy<O: Recordable>(tree: &Tree) -> Result<Economy<O>, StateError> {
    let mut builder = axionomy::EconomyBuilder::new();
    let mut read = 0u64;
    let mut balances = tree.prefix(&[BALANCE]).peekable();
    for (key, value) in tree.prefix(&[ACCOUNT]) {
        read += 1;
        if !value.is_empty() {
            return Err(StateError("an account entry has a value"));
        }
        let account: O::Account = decode(&key[1..], "an account")?;
        let prefix = [&[BALANCE][..], &key[1..], &[0]].concat();
        let mut holdings = Vec::new();
        while let Some((key, value)) = balances.next_if(|(key, _)| key.starts_with(&prefix)) {
            read += 1;
            let asset: O::Asset = decode(&key[prefix.len()..], "an asset")?;
            let quantity = quantity(value)?;
            if quantity.is_zero() {
                return Err(StateError("a balance is zero"));
            }
            holdings.push((asset, quantity));
        }
        let basket = axionomy::Basket::try_from_entries(holdings)
            .map_err(|_| StateError("an account holds an asset twice"))?;
        builder = builder.account(account, basket.into());
    }
    if balances.next().is_some() {
        return Err(StateError("a balance belongs to no account"));
    }
    for (key, value) in tree.prefix(&[RATE]) {
        read += 1;
        builder = builder.rate(decode(&key[1..], "a rate")?, decode(value, "a rate")?);
    }
    for (key, value) in tree.prefix(&[INVARIANT]) {
        read += 1;
        if !value.is_empty() {
            return Err(StateError("an invariant entry has a value"));
        }
        builder = builder.invariant(decode(&key[1..], "an invariant")?);
    }
    if read != tree.len() {
        return Err(StateError("an entry holds nothing an economy has"));
    }
    builder
        .build()
        .map_err(|_| StateError("the entries do not make a valid economy"))
}

/// The positions in a state's tree of the accounts whose paths start with
/// `prefix`, in path order.
pub fn accounts(tree: &Tree, prefix: &str) -> Range<u64> {
    span(tree, &[&[ACCOUNT][..], &text_prefix(prefix)].concat())
}

/// The accounts at `positions`, each with how many assets it holds.
pub fn account_rows(
    tree: &Tree,
    positions: Range<u64>,
) -> impl Iterator<Item = Result<(u64, AccountPath, u64), StateError>> + '_ {
    tree.iter_at(positions.start)
        .zip(positions)
        .map(move |((key, _), position)| {
            let account: AccountPath = key
                .strip_prefix(&[ACCOUNT])
                .ok_or(StateError("a position is not an account"))
                .and_then(|bytes| decode(bytes, "an account"))?;
            let balances = span(tree, &balances_key(&account));
            Ok((position, account, balances.end - balances.start))
        })
}

/// The positions in a state's tree of an account's balances whose asset
/// paths start with `prefix`, in path order.
pub fn balances(tree: &Tree, account: &AccountPath, prefix: &str) -> Range<u64> {
    span(tree, &[balances_key(account), text_prefix(prefix)].concat())
}

/// The balances at `positions`, which [`balances`] gave for `account`.
pub fn balance_rows<'a>(
    tree: &'a Tree,
    account: &AccountPath,
    positions: Range<u64>,
) -> impl Iterator<Item = Result<(u64, AssetPath, Quantity), StateError>> + 'a {
    let prefix = balances_key(account);
    tree.iter_at(positions.start)
        .zip(positions)
        .map(move |((key, value), position)| {
            let asset = key
                .strip_prefix(prefix.as_slice())
                .ok_or(StateError(
                    "a position is not one of the account's balances",
                ))
                .and_then(|bytes| decode(bytes, "an asset"))?;
            Ok((position, asset, quantity(value)?))
        })
}

/// How one state differs from another.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change<Account, Asset, Rate> {
    Opened(Account),
    Closed(Account),
    Balance {
        account: Account,
        asset: Asset,
        before: Quantity,
        after: Quantity,
    },
    Rate(Rate),
    Invariants,
}

/// A change between states of an ontology's economies.
pub type ChangeOf<O> = Change<
    <O as axionomy::Ontology>::Account,
    <O as axionomy::Ontology>::Asset,
    <O as axionomy::Ontology>::Rate,
>;

/// What changed from `before` to `after`, in key order: accounts opened and
/// closed, balances whose quantity differs, rates added, removed or redefined,
/// and whether the invariants differ. Subtrees the states share are skipped.
pub fn changes<O: Recordable>(before: &Tree, after: &Tree) -> Result<Vec<ChangeOf<O>>, StateError> {
    let mut changes = Vec::new();
    let mut invariants = false;
    for change in before.diff(after) {
        let (&space, rest) = change
            .key
            .split_first()
            .ok_or(StateError("an entry has an empty key"))?;
        match space {
            ACCOUNT => changes.push(match change.after {
                Some(_) => Change::Opened(decode(rest, "an account")?),
                None => Change::Closed(decode(rest, "an account")?),
            }),
            BALANCE => {
                let split = rest
                    .iter()
                    .position(|&byte| byte == 0)
                    .ok_or(StateError("a balance key has no asset"))?;
                let held = |value: Option<&[u8]>| value.map_or(Ok(Quantity::ZERO), quantity);
                changes.push(Change::Balance {
                    account: decode(&rest[..split], "an account")?,
                    asset: decode(&rest[split + 1..], "an asset")?,
                    before: held(change.before)?,
                    after: held(change.after)?,
                });
            }
            RATE => changes.push(Change::Rate(decode(rest, "a rate")?)),
            INVARIANT => invariants = true,
            _ => return Err(StateError("an entry holds nothing an economy has")),
        }
    }
    if invariants {
        changes.push(Change::Invariants);
    }
    Ok(changes)
}

/// The positions of the keys that start with `prefix`.
fn span(tree: &Tree, prefix: &[u8]) -> Range<u64> {
    let start = tree.rank(prefix);
    let end = match successor(prefix) {
        Some(end) => tree.rank(&end),
        None => tree.len(),
    };
    start..end
}

/// The least key greater than every key that starts with `prefix`.
fn successor(prefix: &[u8]) -> Option<Vec<u8>> {
    let mut end = prefix.to_vec();
    while let Some(last) = end.pop() {
        if last < u8::MAX {
            end.push(last + 1);
            return Some(end);
        }
    }
    None
}

fn quantity(value: &[u8]) -> Result<Quantity, StateError> {
    let bytes: [u8; 32] = value
        .try_into()
        .map_err(|_| StateError("a balance is not 32 bytes"))?;
    Ok(Quantity::from_be_bytes(bytes))
}

fn key(space: u8, value: &impl Serialize) -> Vec<u8> {
    [&[space][..], &json(value)].concat()
}

fn account_key(account: &impl Id) -> Vec<u8> {
    key(ACCOUNT, account)
}

fn balance_key(account: &impl Id, asset: &impl Id) -> Vec<u8> {
    [key(BALANCE, account), vec![0], json(asset)].concat()
}

/// The prefix of every balance key of `account`.
fn balances_key(account: &AccountPath) -> Vec<u8> {
    [key(BALANCE, account), vec![0]].concat()
}

/// What the JSON of every string starting with `prefix` starts with: its
/// opening quote and the prefix, each character escaped as JSON escapes it.
fn text_prefix(prefix: &str) -> Vec<u8> {
    let mut quoted = json(&prefix);
    quoted.pop();
    quoted
}

fn json(value: &impl Serialize) -> Vec<u8> {
    serde_json::to_vec(value).expect("kernel values serialize")
}

/// Reads what `bytes` encode, accepting only the encoding [`json`] writes.
fn decode<T: Serialize + DeserializeOwned>(
    bytes: &[u8],
    what: &'static str,
) -> Result<T, StateError> {
    let value: T = serde_json::from_slice(bytes).map_err(|_| StateError(what))?;
    if json(&value) != bytes {
        return Err(StateError("an entry is not in the store's encoding"));
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axionomy::{
        Rate, RatePath, basket,
        paths::{self, Paths},
    };

    fn path<K>(text: &str) -> axionomy::path::Path<K> {
        text.parse().unwrap()
    }

    #[test]
    fn exchanges_update_the_tree_to_the_one_their_state_has() {
        let open: RatePath = path("accounts/open");
        let fund: RatePath = path("accounts/fund");
        let close: RatePath = path("accounts/close");
        let mut economy = paths::EconomyBuilder::new()
            .account(path("bank"), basket([(path("cash"), 100)]).into())
            .account(path("bank/vault"), basket([(path("gold"), 5)]).into())
            .rate(open.clone(), Rate::new().opens(path("new")))
            .rate(
                fund.clone(),
                Rate::new()
                    .consume(path("from"), basket([(path("cash"), 10)]))
                    .produce(path("to"), basket([(path("cash"), 10)])),
            )
            .rate(
                close.clone(),
                Rate::new()
                    .consume(path("gone"), basket([(path("cash"), 10)]))
                    .produce(path("bank"), basket([(path("cash"), 10)]))
                    .closes(path("gone")),
            )
            .invariant(paths::LinearInvariant::new("cash").weight(path("cash"), 1))
            .build()
            .unwrap();
        let mut stored = tree::<Paths>(&economy);
        let initial = stored.clone();
        let steps = [
            paths::Exchange::new(open, Quantity::new(1)).bind(path("new"), path("agents/1")),
            paths::Exchange::new(fund, Quantity::new(2))
                .bind(path("from"), path("bank"))
                .bind(path("to"), path("agents/1")),
            paths::Exchange::new(close.clone(), Quantity::new(2))
                .bind(path("gone"), path("agents/1"))
                .bind(path("bank"), path("bank")),
        ];
        assert!(economy.apply(steps[2].clone()).is_err());
        let mut states = vec![stored.clone()];
        for exchange in steps {
            let receipt = economy.apply(exchange).unwrap();
            advance::<Paths>(&mut stored, &economy, &receipt);
            assert_eq!(stored.hash(), tree::<Paths>(&economy).hash());
            let rebuilt = super::economy::<Paths>(&stored).unwrap();
            assert_eq!(rebuilt.state_key(), economy.state_key());
            assert_eq!(tree::<Paths>(&rebuilt).hash(), stored.hash());
            states.push(stored.clone());
        }

        let (bank, agent): (AccountPath, AccountPath) = (path("bank"), path("agents/1"));
        let cash: AssetPath = path("cash");
        let balance = |account: &AccountPath, before, after| Change::Balance {
            account: account.clone(),
            asset: cash.clone(),
            before: Quantity::new(before),
            after: Quantity::new(after),
        };
        assert_eq!(
            changes::<Paths>(&states[0], &states[1]).unwrap(),
            [Change::Opened(agent.clone())]
        );
        assert_eq!(
            changes::<Paths>(&states[1], &states[2]).unwrap(),
            [balance(&agent, 0, 20), balance(&bank, 100, 80)]
        );
        assert_eq!(
            changes::<Paths>(&states[2], &states[3]).unwrap(),
            [
                Change::Closed(agent.clone()),
                balance(&agent, 20, 0),
                balance(&bank, 80, 100)
            ]
        );
        // Closing what was opened returns to the same state, and the same root.
        assert!(changes::<Paths>(&initial, &stored).unwrap().is_empty());
        assert_eq!(initial.hash(), stored.hash());

        let listed: Vec<_> = account_rows(&states[2], accounts(&states[2], "ba"))
            .map(|row| row.unwrap())
            .map(|(_, account, assets)| (account.to_string(), assets))
            .collect();
        assert_eq!(listed, [("bank".into(), 1), ("bank/vault".into(), 1)]);
        let held: Vec<_> = balance_rows(&states[2], &agent, balances(&states[2], &agent, "c"))
            .map(|row| row.unwrap())
            .map(|(_, asset, quantity)| (asset, quantity))
            .collect();
        assert_eq!(held, [(cash, Quantity::new(20))]);
    }

    #[test]
    fn only_trees_an_economy_has_decode() {
        let economy = paths::EconomyBuilder::new()
            .account(path("bank"), basket([(path("cash"), 100)]).into())
            .build()
            .unwrap();
        let valid = tree::<Paths>(&economy);
        assert!(valid.get(br#"a"bank""#).is_some());
        for (key, value) in [
            (&b"b\"bank\"\0\"gold\""[..], &[0u8; 32][..]),
            (b"b\"bank\"\0\"gold\"", b"7"),
            (b"b\"nobody\"\0\"cash\"", &[1; 32]),
            (b"a\"Bank\"", b""),
            (b"a\"bank/vault\"", b"x"),
            (b"a \"bank/vault\"", b""),
            (b"abank", b""),
            (b"zother", b""),
        ] {
            let mut forged = valid.clone();
            forged.insert(key, value);
            assert!(super::economy::<Paths>(&forged).is_err(), "{key:?}");
        }
    }
}
