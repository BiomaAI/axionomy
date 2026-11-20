//! Read the shipped model as the problem's own account, asset, role and
//! rate types. Unknown typed identifiers fail while the fixture is built.
use axionomy::{Account, Basket, Economy, Ontology, OntologyPath, Quantity, path::Path};
use axionomy_authoring::program::{LibraryPath, Program};
use axionomy_authoring::{Instance, LawSet, LawSetPath};
use serde::de::DeserializeOwned;
use std::{
    collections::BTreeMap,
    sync::{Arc, LazyLock, Mutex},
};

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
    let mut instance: Instance<O> = document(package, &format!("instances/{reference}.rhai"));
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

// The source is embedded and immutable. Evaluate each program once, then
// decode its result through each problem's own strongly typed ontology.
type SourceKey = (&'static str, &'static str);
type ValueCache = Mutex<BTreeMap<SourceKey, Arc<axionomy_authoring::program::Value>>>;

fn document<T: DeserializeOwned>(package: &str, relative: &str) -> T {
    static HOSTS: LazyLock<BTreeMap<&'static str, Program>> = LazyLock::new(|| {
        let mut libraries = BTreeMap::<_, Vec<(LibraryPath, &str)>>::new();
        for &(package, file, text) in GALLERY {
            let modules = libraries.entry(package).or_default();
            if let Some(id) = file
                .strip_prefix("lib/")
                .and_then(|p| p.strip_suffix(".rhai"))
            {
                modules.push((id.parse().unwrap(), text));
            }
        }
        libraries
            .into_iter()
            .map(|(id, modules)| {
                (
                    id,
                    Program::with_modules(modules.iter().map(|(id, text)| (id, *text)))
                        .expect("gallery authoring modules"),
                )
            })
            .collect()
    });
    static VALUES: LazyLock<ValueCache> = LazyLock::new(Mutex::default);
    let &(package, file, source) = GALLERY
        .iter()
        .find(|(p, name, _)| *p == package && *name == relative)
        .unwrap_or_else(|| panic!("missing {relative} in {package}"));
    let value = VALUES
        .lock()
        .expect("fixture values")
        .entry((package, file))
        .or_insert_with(|| {
            Arc::new(
                HOSTS[package]
                    .evaluate(source)
                    .unwrap_or_else(|e| panic!("{file}: {e}")),
            )
        })
        .clone();
    axionomy_authoring::program::decode(&value)
        .unwrap_or_else(|e| panic!("{file} {}: {e}", e.field))
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
    let declarations = document(package, &format!("laws/{id}.rhai"));
    axionomy_authoring::expand(declarations).expect("finite checked rate families")
}
