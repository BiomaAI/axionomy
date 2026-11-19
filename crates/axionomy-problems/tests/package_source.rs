#![cfg(feature = "packages")]
use axionomy_engine::{Engine, source};
use axionomy_store::Store;

/// An exported sample carries the exact source an author can edit and
/// rebuild in isolation. Rebuilding retains every reference's economic
/// history and its native ontology, independent of the new run handles.
#[test]
fn standalone_gallery_sources_rebuild_the_same_histories() {
    for (name, write) in axionomy_problems::packages() {
        let installed = Engine::new(Store::memory().unwrap()).unwrap();
        write(&installed).unwrap();
        let files = installed.package_source(&name).unwrap();
        assert_eq!(files, axionomy_problems::source(&name));
        let rebuilt = source::build(&files).unwrap();
        for reference in installed.named_under(&name).unwrap().refs {
            let history = |engine: &Engine| {
                let at = engine.referred(&reference).unwrap();
                (
                    engine.ontology(at).unwrap(),
                    blake3::hash(&postcard::to_allocvec(&engine.history(at).unwrap()).unwrap()),
                )
            };
            assert_eq!(history(&installed), history(&rebuilt.engine), "{reference}");
        }
    }
}
