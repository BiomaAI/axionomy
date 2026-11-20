//! Typed account, asset, role, rate and parameter declarations returned by an
//! ontology program. The package compiler checks every reference against them.
use axionomy::{AccountPath, AssetPath, ParameterPath, Pattern, RatePath, RolePath};
use rhai::{Engine, EvalAltResult};
use serde::Serialize;
use std::{collections::BTreeSet, fmt::Display};

#[derive(Clone, Default, Serialize)]
pub(crate) struct Vocabulary {
    accounts: BTreeSet<AccountPath>,
    assets: BTreeSet<AssetPath>,
    roles: BTreeSet<RolePath>,
    rates: BTreeSet<Pattern<RatePath>>,
    parameters: BTreeSet<ParameterPath>,
}

fn declare<T: Ord + Display>(set: &mut BTreeSet<T>, id: T) -> Result<(), Box<EvalAltResult>> {
    if set.contains(&id) {
        return Err(format!("duplicate declaration `{id}`").into());
    }
    set.insert(id);
    Ok(())
}

pub(crate) fn register(engine: &mut Engine) {
    engine
        .register_type_with_name::<Vocabulary>("Ontology")
        .register_fn("ontology", Vocabulary::default)
        .register_fn("declare", |v: &mut Vocabulary, id: AccountPath| {
            declare(&mut v.accounts, id)
        })
        .register_fn("declare", |v: &mut Vocabulary, id: AssetPath| {
            declare(&mut v.assets, id)
        })
        .register_fn("declare", |v: &mut Vocabulary, id: RolePath| {
            declare(&mut v.roles, id)
        })
        .register_fn("declare", |v: &mut Vocabulary, id: RatePath| {
            declare(&mut v.rates, Pattern::literal(id))
        })
        .register_fn(
            "rate_pattern",
            |v: &mut Vocabulary, text: &str| -> Result<(), Box<EvalAltResult>> {
                declare(
                    &mut v.rates,
                    text.parse()
                        .map_err(|e| format!("rate pattern `{text}`: {e}"))?,
                )
            },
        )
        .register_fn(
            "parameter",
            |v: &mut Vocabulary, text: &str| -> Result<(), Box<EvalAltResult>> {
                declare(
                    &mut v.parameters,
                    text.parse()
                        .map_err(|e| format!("parameter `{text}`: {e}"))?,
                )
            },
        )
        .register_fn("finish", |v: Vocabulary| rhai::serde::to_dynamic(v));
}
