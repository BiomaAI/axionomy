//! Rhai's native economic model. The same laws and holdings can be used
//! in a package or submitted to an interactive economy operation.
use crate::program::{decode, normalize};
use axionomy::{
    AccountPath, InvariantPath, OntologyPath, Pattern, RatePath, Segment, VariablePath, paths,
};
use indexmap::IndexMap;
use rhai::{Array, Dynamic, Engine, EvalAltResult, Map};
use serde::Serialize;

type Failure = Box<EvalAltResult>;

#[derive(Clone, Serialize)]
pub struct NamedLaw(pub Pattern<RatePath>, pub paths::Rate);

#[derive(Clone, Default)]
pub struct Model {
    pub accounts: IndexMap<AccountPath, paths::Account>,
    pub constants: IndexMap<VariablePath, Segment>,
    pub rates: IndexMap<Pattern<RatePath>, paths::Rate>,
    pub invariants: IndexMap<InvariantPath, paths::LinearInvariant>,
}

fn insert<K: Eq + std::hash::Hash + std::fmt::Display, V>(
    entries: &mut IndexMap<K, V>,
    key: K,
    value: V,
) -> Result<(), Failure> {
    if entries.contains_key(&key) {
        return Err(format!("duplicate model entry `{key}`").into());
    }
    entries.insert(key, value);
    Ok(())
}

pub(crate) fn register(engine: &mut Engine) {
    engine
        .register_type_with_name::<NamedLaw>("Law")
        .register_fn(
            "law",
            |name: &str, terms: Map| -> Result<NamedLaw, Failure> {
                let key = name
                    .parse()
                    .map_err(|e| format!("rate pattern `{name}`: {e}"))?;
                let rate = decode(&normalize(Dynamic::from(terms))?)
                    .map_err(|e| format!("law {name} {}: {e}", e.field))?;
                Ok(NamedLaw(key, rate))
            },
        )
        .register_type_with_name::<Model>("Model")
        .register_fn("model", Model::default)
        .register_fn(
            "open",
            |model: &mut Model, id: AccountPath, basket: paths::Basket| {
                insert(&mut model.accounts, id, basket.into())
            },
        )
        .register_fn("add", |model: &mut Model, law: NamedLaw| {
            insert(&mut model.rates, law.0, law.1)
        })
        .register_fn("add", |model: &mut Model, rule: paths::LinearInvariant| {
            insert(&mut model.invariants, rule.name().clone(), rule)
        })
        .register_fn(
            "constant",
            |model: &mut Model, name: &str, value: &str| -> Result<(), Failure> {
                insert(
                    &mut model.constants,
                    name.parse()
                        .map_err(|e| format!("constant `{name}`: {e}"))?,
                    value
                        .parse()
                        .map_err(|e| format!("constant value `{value}`: {e}"))?,
                )
            },
        )
        .register_fn(
            "instance",
            |model: Model, ontology: &str, laws: Array| -> Result<Map, Failure> {
                let ontology: OntologyPath = ontology
                    .parse()
                    .map_err(|e| format!("ontology `{ontology}`: {e}"))?;
                let mut out = Map::new();
                out.insert("ontology".into(), Dynamic::from(ontology.to_string()));
                out.insert("laws".into(), Dynamic::from(laws));
                out.insert(
                    "accounts".into(),
                    rhai::serde::to_dynamic(model.accounts.into_iter().collect::<Vec<_>>())?,
                );
                out.insert(
                    "rates".into(),
                    rhai::serde::to_dynamic(model.rates.into_iter().collect::<Vec<_>>())?,
                );
                out.insert(
                    "invariants".into(),
                    rhai::serde::to_dynamic(model.invariants.into_values().collect::<Vec<_>>())?,
                );
                out.insert(
                    "constants".into(),
                    rhai::serde::to_dynamic(model.constants)?,
                );
                Ok(out)
            },
        );
}
