#![cfg(feature = "packages")]
use axionomy_engine::{Engine, source};
use axionomy_store::Store;

/// Check the entire authored gallery against the existing typed writers
/// before switching the consumers, including fork histories and native views.
#[test]
fn gallery_reproduces_every_named_history_and_definition() {
    for (name, write) in axionomy_problems::packages() {
        let original = Engine::new(Store::memory().unwrap()).unwrap();
        write(&original).unwrap();
        let built = source::build(&axionomy_problems::source(&name))
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        let expected = original.named_under(&name).unwrap();
        assert_eq!(expected, built.engine.named_under(&name).unwrap(), "{name}");
        for reference in expected.refs {
            let a = original.referred(&reference).unwrap();
            let b = built.engine.referred(&reference).unwrap();
            assert_eq!(
                original.ontology(a).unwrap(),
                built.engine.ontology(b).unwrap(),
                "{reference}"
            );
            let history = |e: &Engine, at| postcard::to_allocvec(&e.history(at).unwrap()).unwrap();
            assert_eq!(
                blake3::hash(&history(&original, a)),
                blake3::hash(&history(&built.engine, b)),
                "{reference}"
            );
        }
        for def in expected.definitions {
            assert_eq!(
                original.store().read().unwrap().definition(&def).unwrap(),
                built
                    .engine
                    .store()
                    .read()
                    .unwrap()
                    .definition(&def)
                    .unwrap(),
                "{def}"
            );
        }
    }
}
