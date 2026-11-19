//! Read the shipped model as the problem's own account, asset, role and
//! rate types. Unknown typed identifiers fail while the fixture is built.
use axionomy::{Account, Basket, Economy, Ontology, OntologyPath, Quantity, path::Path};
use axionomy_authoring::{Instance, LawSet, LawSetPath};
use serde::de::DeserializeOwned;
use std::collections::BTreeMap;

pub(super) const GALLERY: &[(&str, &str, &str)] = include!(concat!(env!("OUT_DIR"), "/gallery.rs"));

pub(super) enum Fixture {}
impl axionomy::PathKind for Fixture {
    const SCHEMA: &'static str = "FixturePath";
}

pub(super) fn initial<O>(reference: Path<Fixture>) -> Economy<O>
where
    O: Ontology<
            Account: DeserializeOwned,
            Asset: DeserializeOwned,
            Rate: DeserializeOwned,
            Role: DeserializeOwned,
        >,
{
    variant(reference, |_| {})
}

pub(super) fn variant<O>(
    reference: Path<Fixture>,
    edit: impl FnOnce(&mut Instance<O>),
) -> Economy<O>
where
    O: Ontology<
            Account: DeserializeOwned,
            Asset: DeserializeOwned,
            Rate: DeserializeOwned,
            Role: DeserializeOwned,
        >,
{
    let package = reference
        .as_str()
        .split('/')
        .next()
        .expect("a package name");
    let mut instance: Instance<O> =
        serde_json::from_str(file(package, &format!("instances/{reference}.json")))
            .expect("typed instance");
    assert_eq!(instance.ontology, OntologyPath::of::<O>());
    edit(&mut instance);
    let laws: BTreeMap<LawSetPath, LawSet<O>> = instance
        .laws
        .iter()
        .map(|name| (name.clone(), laws(name)))
        .collect();
    instance
        .build(|set| laws.get(set))
        .expect("well-formed shipped model")
}

pub(super) fn account<O: Ontology>(
    instance: &mut Instance<O>,
    id: O::Account,
) -> &mut Account<O::Asset> {
    &mut instance
        .accounts
        .iter_mut()
        .find(|(name, _)| *name == id)
        .expect("a declared fixture account")
        .1
}

pub(super) fn set<O: Ontology>(
    instance: &mut Instance<O>,
    id: O::Account,
    values: impl IntoIterator<Item = (O::Asset, u64)>,
) {
    let account = account(instance, id);
    let mut balances: Basket<O::Asset> = account.balances().clone();
    for (asset, value) in values {
        balances.insert(asset, Quantity::new(value));
    }
    *account = Account::from(balances);
}

fn file(package: &str, relative: &str) -> &'static str {
    GALLERY
        .iter()
        .find(|(p, name, _)| *p == package && *name == relative)
        .unwrap_or_else(|| panic!("missing {relative} in {package}"))
        .2
}

pub(super) fn laws<O>(id: &LawSetPath) -> LawSet<O>
where
    O: Ontology<
            Account: DeserializeOwned,
            Asset: DeserializeOwned,
            Rate: DeserializeOwned,
            Role: DeserializeOwned,
        >,
{
    let package = id.as_str().split('/').next().expect("a package name");
    let declarations =
        serde_json::from_str(file(package, &format!("laws/{id}.json"))).expect("typed rate laws");
    axionomy_authoring::expand(declarations).expect("finite checked rate families")
}
