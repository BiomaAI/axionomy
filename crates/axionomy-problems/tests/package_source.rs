#![cfg(feature = "packages")]
use axionomy_engine::{Engine, PackagePath, source};
use axionomy_store::Store;
use std::collections::BTreeMap;

/// Each hash covers every named starting economy, its ontology and its
/// ordered exchanges, recorded before replacing the original gallery writers.
#[test]
fn standalone_gallery_sources_reproduce_the_pinned_histories() {
    let pins: BTreeMap<PackagePath, String> =
        serde_json::from_str(include_str!("fixtures/package-histories.json")).unwrap();
    let packages = axionomy_problems::packages();
    assert_eq!(pins.len(), packages.len());
    for (name, write) in packages {
        let installed = Engine::new(Store::memory().unwrap()).unwrap();
        write(&installed).unwrap();
        let files = installed.package_source(&name).unwrap();
        assert_eq!(files, axionomy_problems::source(&name));
        let rebuilt = source::build(&files).unwrap();
        assert!(
            !rebuilt.tests.is_empty(),
            "{name} needs an economic assertion"
        );
        let mut histories = BTreeMap::new();
        for reference in installed.named_under(&name).unwrap().refs {
            let history = |engine: &Engine| {
                let at = engine.referred(&reference).unwrap();
                let (economy, exchanges) = engine.history(at).unwrap();
                serde_json::json!([engine.ontology(at).unwrap(), economy, exchanges])
            };
            let original = history(&installed);
            assert_eq!(original, history(&rebuilt.engine), "{reference}");
            histories.insert(reference, original);
        }
        let digest = blake3::hash(&serde_json::to_vec(&histories).unwrap());
        assert_eq!(digest.to_string(), pins[&name], "{name}");
    }
}
