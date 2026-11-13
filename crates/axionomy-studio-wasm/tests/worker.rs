//! The Worker's engine, natively: a package loads from its bytes, an
//! experiment runs, and a store kept on a backend that outlives it finds
//! both again, with the URL the package came from and what it said.

use axionomy_engine::{Engine, Version, zipped};
use axionomy_store::Store;
use axionomy_studio_wasm::{Validators, WorkerEngine};
use redb::{StorageBackend, backends::InMemoryBackend};
use std::{io, ops::ControlFlow, sync::Arc};

fn path<K>(text: &str) -> axionomy::path::Path<K> {
    text.parse().unwrap()
}

/// Bytes that outlive the stores opened on them, as the origin private file
/// system's do.
#[derive(Debug, Clone, Default)]
struct Kept(Arc<InMemoryBackend>);

impl Kept {
    fn bytes(&self) -> Vec<u8> {
        let mut bytes = vec![0; usize::try_from(self.0.len().unwrap()).unwrap()];
        self.0.read(0, &mut bytes).unwrap();
        bytes
    }
}

impl StorageBackend for Kept {
    fn len(&self) -> Result<u64, io::Error> {
        self.0.len()
    }

    fn read(&self, offset: u64, out: &mut [u8]) -> Result<(), io::Error> {
        self.0.read(offset, out)
    }

    fn set_len(&self, len: u64) -> Result<(), io::Error> {
        self.0.set_len(len)
    }

    fn sync_data(&self) -> Result<(), io::Error> {
        self.0.sync_data()
    }

    fn write(&self, offset: u64, data: &[u8]) -> Result<(), io::Error> {
        self.0.write(offset, data)
    }
}

/// The maze sample's package file, zipped, as Studio serves it.
fn maze() -> Vec<u8> {
    let name = path("maze");
    let (_, write) = axionomy_problems::packages()
        .into_iter()
        .find(|(sample, _)| *sample == name)
        .unwrap();
    let engine = Engine::new(Store::memory().unwrap()).unwrap();
    write(&engine).unwrap();
    let file = Kept::default();
    {
        let package = Store::on(file.clone()).unwrap();
        let contents = engine.named_under(&name).unwrap();
        engine
            .export(
                &name,
                "0.1.0".parse::<Version>().unwrap(),
                contents,
                &package,
            )
            .unwrap();
    }
    zipped(&file.bytes()).unwrap()
}

const URL: &str = "https://studio.example/packages/maze.zip";

#[test]
fn a_package_and_its_experiment_are_found_again_with_where_it_came_from() {
    let bytes = maze();
    let validators = Validators {
        etag: Some("\"maze-1\"".to_owned()),
        last_modified: None,
    };
    let kept = Kept::default();
    let ran = {
        let worker = WorkerEngine::over(Store::on(kept.clone()).unwrap()).unwrap();
        assert_eq!(worker.source_of(URL).unwrap(), None);
        let loaded = worker
            .load_from(&bytes, Some((URL, validators.clone())))
            .unwrap();
        assert_eq!(loaded.manifests[0].name, path("maze"));
        assert!(loaded.kept.refs.is_empty() && loaded.kept.definitions.is_empty());
        assert_eq!(worker.source_of(URL).unwrap(), Some(validators.clone()));
        assert_eq!(
            worker
                .source_of("https://elsewhere.example/maze.zip")
                .unwrap(),
            None
        );
        let experiments = worker.engine().experiments().unwrap();
        assert!(experiments.contains(&path("maze/micro")), "{experiments:?}");
        worker
            .engine()
            .run_experiment(&path("maze/micro"), &mut |_| ControlFlow::Continue(()))
            .unwrap()
    };

    // Opened again, as a reloaded Worker opens it.
    let worker = WorkerEngine::over(Store::on(kept).unwrap()).unwrap();
    let manifests = worker.engine().manifests().unwrap();
    assert_eq!(manifests.len(), 1);
    assert_eq!(manifests[0].name, path("maze"));
    assert_eq!(worker.engine().results(&path("maze/micro")).unwrap(), ran);
    assert_eq!(worker.source_of(URL).unwrap(), Some(validators));
    let end = ran.arms[0].trials[0].end;
    assert!(!worker.engine().draw(end).unwrap().frames.is_empty());
}

#[test]
fn a_package_without_a_url_loads_and_names_no_source() {
    let worker = WorkerEngine::over(Store::memory().unwrap()).unwrap();
    let loaded = worker.load_from(&maze(), None).unwrap();
    assert_eq!(loaded.manifests[0].name, path("maze"));
    assert_eq!(worker.source_of(URL).unwrap(), None);
}
