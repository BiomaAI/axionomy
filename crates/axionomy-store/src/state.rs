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
//! Paths never contain `\0`, which sorts before every path character, so an
//! account's balances are contiguous and in the same order as the accounts.
//! The tree holds the economy's logical state, not the order its accounts
//! were created in: a rebuilt economy lists accounts in path order.

use axionomy::{AccountPath, AssetPath, Quantity, RatePath, paths};
use axionomy_tree::Tree;
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
pub fn tree(economy: &paths::Economy) -> Tree {
    let mut entries = Vec::new();
    for (account, holdings) in economy.accounts() {
        entries.push((account_key(account), Vec::new()));
        for (asset, quantity) in holdings.balances().iter() {
            entries.push((
                balance_key(account, asset.as_str()),
                quantity.to_be_bytes().to_vec(),
            ));
        }
    }
    for rate in economy.rate_ids() {
        let definition = economy.rate(rate).expect("listed rates exist");
        entries.push((rate_key(rate), json(definition)));
    }
    for invariant in economy.invariants() {
        entries.push((invariant_key(invariant), Vec::new()));
    }
    entries.into_iter().collect()
}

/// Updates `tree` from the state before `receipt`'s exchange to `economy`,
/// the state after it. The receipt names every balance and account that
/// changed; their new values come from the kernel.
pub fn advance(tree: &mut Tree, economy: &paths::Economy, receipt: &paths::Receipt) {
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
                balance_key(account, asset.as_str()),
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
pub fn economy(tree: &Tree) -> Result<paths::Economy, StateError> {
    let mut builder = paths::EconomyBuilder::new();
    let mut read = 0u64;
    let mut balances = tree.prefix(&[BALANCE]).peekable();
    for (key, value) in tree.prefix(&[ACCOUNT]) {
        read += 1;
        if !value.is_empty() {
            return Err(StateError("an account entry has a value"));
        }
        let account: AccountPath = path(&key[1..])?;
        let prefix = balance_key(&account, "");
        let mut holdings = Vec::new();
        while let Some((key, value)) = balances.next_if(|(key, _)| key.starts_with(&prefix)) {
            read += 1;
            let asset: AssetPath = path(&key[prefix.len()..])?;
            let bytes: [u8; 32] = value
                .try_into()
                .map_err(|_| StateError("a balance is not 32 bytes"))?;
            let quantity = Quantity::from_be_bytes(bytes);
            if quantity.is_zero() {
                return Err(StateError("a balance is zero"));
            }
            holdings.push((asset, quantity));
        }
        let basket = paths::Basket::try_from_entries(holdings)
            .map_err(|_| StateError("an account holds an asset twice"))?;
        builder = builder.account(account, basket.into());
    }
    if balances.next().is_some() {
        return Err(StateError("a balance belongs to no account"));
    }
    for (key, value) in tree.prefix(&[RATE]) {
        read += 1;
        let rate: RatePath = path(&key[1..])?;
        let definition: paths::Rate =
            serde_json::from_slice(value).map_err(|_| StateError("a rate does not decode"))?;
        if json(&definition) != value {
            return Err(StateError("a rate is not in the kernel's encoding"));
        }
        builder = builder.rate(rate, definition);
    }
    for (key, value) in tree.prefix(&[INVARIANT]) {
        read += 1;
        let invariant: paths::LinearInvariant = serde_json::from_slice(&key[1..])
            .map_err(|_| StateError("an invariant does not decode"))?;
        if json(&invariant) != key[1..] || !value.is_empty() {
            return Err(StateError("an invariant is not in the kernel's encoding"));
        }
        builder = builder.invariant(invariant);
    }
    if read != tree.len() {
        return Err(StateError("an entry holds nothing an economy has"));
    }
    builder
        .build()
        .map_err(|_| StateError("the entries do not make a valid economy"))
}

fn account_key(account: &AccountPath) -> Vec<u8> {
    [&[ACCOUNT], account.as_str().as_bytes()].concat()
}

fn balance_key(account: &AccountPath, asset: &str) -> Vec<u8> {
    [
        &[BALANCE],
        account.as_str().as_bytes(),
        &[0],
        asset.as_bytes(),
    ]
    .concat()
}

fn rate_key(rate: &RatePath) -> Vec<u8> {
    [&[RATE], rate.as_str().as_bytes()].concat()
}

fn invariant_key(invariant: &paths::LinearInvariant) -> Vec<u8> {
    [&[INVARIANT][..], &json(invariant)].concat()
}

fn json(value: &impl serde::Serialize) -> Vec<u8> {
    serde_json::to_vec(value).expect("kernel values serialize")
}

fn path<K>(bytes: &[u8]) -> Result<axionomy::path::Path<K>, StateError> {
    std::str::from_utf8(bytes)
        .ok()
        .and_then(|text| text.parse().ok())
        .ok_or(StateError("a key is not a path"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axionomy::{Rate, basket};

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
        let mut stored = tree(&economy);
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
        for exchange in steps {
            let before = stored.clone();
            let receipt = economy.apply(exchange).unwrap();
            advance(&mut stored, &economy, &receipt);
            assert_eq!(stored.hash(), tree(&economy).hash());
            assert!(!stored.diff(&before).is_empty());
            let rebuilt = super::economy(&stored).unwrap();
            assert_eq!(rebuilt.state_key(), economy.state_key());
            assert_eq!(tree(&rebuilt).hash(), stored.hash());
        }
    }

    #[test]
    fn only_trees_an_economy_has_decode() {
        let economy = paths::EconomyBuilder::new()
            .account(path("bank"), basket([(path("cash"), 100)]).into())
            .build()
            .unwrap();
        let valid = tree(&economy);
        for (key, value) in [
            (&b"bbank\0gold"[..], &[0u8; 32][..]),
            (b"bbank\0gold", b"7"),
            (b"bnobody\0cash", &[1; 32]),
            (b"aBank", b""),
            (b"abank/vault", b"x"),
            (b"zother", b""),
        ] {
            let mut forged = valid.clone();
            forged.insert(key, value);
            assert!(super::economy(&forged).is_err(), "{key:?}");
        }
    }
}
