//! The samples' views as data (P7, P9): each package's view, a Rhai script,
//! draws every document of every study of its sample exactly as the sample's
//! Rust view does, from runs recorded as the sample's ontology and read as
//! paths.
#![cfg(feature = "packages")]

use axionomy::{Economy, Exchange, path, paths};
use axionomy_engine::Engine;
use axionomy_problems::{WritePackage, bridge, exact_cover, maze, registry, sokoban, workshop};
use axionomy_store::{Checkpoints, Recordable, Store};
use axionomy_study::{Progress, StudyControl, StudyObserver, StudyRequest};
use axionomy_view::{DocumentPath, ViewDocument};
use serde::{Serialize, de::DeserializeOwned};

/// `value` written in binary and read back as `T`: an economy or exchange
/// written as paths read as the sample's, which D-044 makes the same bytes.
fn written<T: DeserializeOwned>(value: &impl Serialize) -> T {
    postcard::from_bytes(&postcard::to_allocvec(value).unwrap()).unwrap()
}

#[derive(Default)]
struct Trajectories(Vec<(DocumentPath, paths::Economy, paths::Trace)>);

impl StudyObserver for Trajectories {
    fn progress(&mut self, _: Progress) {}

    fn trajectory(
        &mut self,
        document: &DocumentPath,
        initial: &paths::Economy,
        trace: &paths::Trace,
    ) {
        self.0
            .push((document.clone(), initial.clone(), trace.clone()));
    }
}

/// What a view draws of a document: everything but what the study attaches
/// to its frames itself.
fn drawn(document: &ViewDocument) -> ViewDocument {
    let mut drawn = document.clone();
    for frame in &mut drawn.frames {
        frame.observations.clear();
    }
    drawn
}

/// Every document of the studies of `problem` at `instances`, one for each
/// strategy its catalog lists, drawn by `write`'s package's view from runs
/// recorded as `O`'s, equals the document the study drew in Rust: the same
/// formats, initial snapshot and frames. Returns how many it compared.
fn draws_like_rust<O: Recordable>(problem: &str, write: WritePackage, instances: &[&str]) -> usize {
    let engine = Engine::new(Store::memory().unwrap()).unwrap();
    write(&engine).unwrap();
    let key = problem.parse().unwrap();
    let strategies: Vec<_> = registry()
        .problem(&key)
        .unwrap()
        .strategies
        .iter()
        .map(|strategy| strategy.key.clone())
        .collect();
    let mut compared = 0;
    for instance in instances {
        let mut seen = Vec::new();
        for strategy in &strategies {
            let request = StudyRequest::new(key.clone())
                .with_instance(instance.parse().unwrap())
                .with_strategy(strategy.clone());
            let mut observed = Trajectories::default();
            let study = registry()
                .study_with(&request, &StudyControl::default(), &mut observed)
                .unwrap();
            for ((id, initial, trace), document) in observed.0.iter().zip(&study.documents) {
                assert_eq!(id, &document.id);
                if seen.contains(id) {
                    continue;
                }
                seen.push(id.clone());
                let mut recorder = engine
                    .store()
                    .record::<O>(written::<Economy<O>>(initial), Checkpoints::Ends);
                for exchange in trace.exchanges() {
                    recorder.apply(written::<Exchange<O>>(exchange)).unwrap();
                }
                let end = recorder.finish().unwrap();
                let scripted = drawn(&engine.draw(end).unwrap());
                let rust = drawn(document);
                let at = format!("{problem} {instance} {id}");
                assert_eq!(scripted.formats, rust.formats, "{at}: formats");
                assert_eq!(scripted.initial, rust.initial, "{at}: the initial snapshot");
                assert_eq!(scripted.frames.len(), rust.frames.len(), "{at}: frames");
                for (scripted, rust) in scripted.frames.iter().zip(&rust.frames) {
                    assert_eq!(scripted, rust, "{at}: frame {}", rust.index);
                }
                compared += 1;
            }
        }
    }
    compared
}

/// Every sample whose package carries a view, and how it is drawn.
type Drawn = (&'static str, fn(&[&str]) -> usize);

const VIEWS: [Drawn; 5] = [
    ("maze", |instances| {
        draws_like_rust::<maze::Ontology>("maze", maze::package::write, instances)
    }),
    ("sokoban", |instances| {
        draws_like_rust::<sokoban::Ontology>("sokoban", sokoban::package::write, instances)
    }),
    ("exact-cover", |instances| {
        draws_like_rust::<exact_cover::Ontology>(
            "exact-cover",
            exact_cover::package::write,
            instances,
        )
    }),
    ("workshop", |instances| {
        draws_like_rust::<workshop::Ontology>("workshop", workshop::package::write, instances)
    }),
    ("bridge", |instances| {
        draws_like_rust::<bridge::Ontology>("bridge", bridge::package::write, instances)
    }),
];

/// Each view draws its sample's studies at `instances` as Rust does.
fn views_draw_like_rust(instances: &[&str]) {
    for (name, draws) in VIEWS {
        assert!(draws(instances) > 0, "{name} drew nothing");
    }
}

#[test]
fn every_view_draws_its_micro_studies_as_rust_does() {
    views_draw_like_rust(&["micro"]);
}

#[test]
#[ignore = "minutes in a debug build; CI runs it in a release build"]
fn every_view_draws_its_larger_studies_as_rust_does() {
    views_draw_like_rust(&["showcase", "stress"]);
}

#[test]
fn a_run_without_a_view_is_drawn_by_its_paths() {
    let engine = Engine::new(Store::memory().unwrap()).unwrap();
    let initial = maze::initial();
    let first: paths::Exchange = written(&maze::candidates(&initial)[0]);
    let start = engine
        .store()
        .record::<maze::Ontology>(initial, Checkpoints::Ends)
        .finish()
        .unwrap();
    let document = engine.draw(start).unwrap();
    assert_eq!(document.title, "maze");
    assert!(document.initial.scene.is_none());
    let agent = &document.initial.accounts[0].account;
    assert_eq!(agent.key.as_str(), "maze/account/agent");
    assert_eq!(agent.label, "account/agent");

    // A fork the engine applies keeps maze's ontology, so maze's view draws
    // it, its history and all.
    maze::package::write(&engine).unwrap();
    let forked = engine.apply(start, &[first]).unwrap();
    assert_eq!(engine.ontology(forked).unwrap().as_str(), "maze");
    let document = engine.draw(forked).unwrap();
    assert_eq!(document.title, "Key-door maze");
    assert_eq!(document.frames.len(), 1);
    assert_eq!(document.initial.accounts[0].account.label, "Explorer");
    assert!(document.frames[0].after.scene.is_some());
    let start = engine.referred(&path!("maze/micro")).unwrap();
    assert_eq!(engine.draw(start).unwrap().title, "Key-door maze");
}
