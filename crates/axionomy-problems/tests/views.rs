//! The samples' views as data (P7, P9): each package's view, a Rhai script,
//! draws every run of every experiment its package carries, from runs
//! recorded as the sample's ontology and read as paths, into documents whose
//! every step is a frame, whose scenes join up and whose evidence is in the
//! snapshot they explain.
#![cfg(feature = "packages")]

use axionomy::{path, paths};
use axionomy_engine::{Engine, rhai::Dynamic};
use axionomy_problems::{WritePackage, maze, packages};
use axionomy_store::{Checkpoints, Point, Store};
use serde::{Serialize, de::DeserializeOwned};
use std::ops::ControlFlow;

/// `value` written in binary and read back as `T`: an economy or exchange
/// written as paths read as the sample's, which D-044 makes the same bytes.
fn written<T: DeserializeOwned>(value: &impl Serialize) -> T {
    postcard::from_bytes(&postcard::to_allocvec(value).unwrap()).unwrap()
}

/// The point `at` drawn with the view of its run's ontology: the document
/// carries that view's title, the model it was drawn from, a frame for
/// every step of the point's history in order, each explained by a cue, a
/// scene at every snapshot that joins up and whose evidence is in that
/// snapshot's accounts, and the format of exactly the assets of the
/// economy the view reads in units. Returns how big the document is.
fn draws(engine: &Engine, at: Point, run: &str) -> Size {
    let ontology = engine.ontology(at).unwrap();
    let view = engine
        .view(&ontology)
        .unwrap()
        .unwrap_or_else(|| panic!("{run}: no view of {ontology}"));
    let (initial, exchanges) = engine.history(at).unwrap();
    let document = engine.draw(at).unwrap();
    assert_eq!(document.title, view.title, "{run}: the view's title");
    let model = document.model.as_ref().unwrap();
    assert!(!model.rates.is_empty(), "{run}: the model's rates");
    assert_eq!(document.frames.len(), exchanges.len(), "{run}: frames");
    for (index, frame) in document.frames.iter().enumerate() {
        assert_eq!(frame.index, index as u64, "{run}: frame {index}");
        assert!(
            !frame.cues.is_empty(),
            "{run}: frame {index} explains nothing"
        );
    }
    let snapshots =
        std::iter::once(&document.initial).chain(document.frames.iter().map(|frame| &frame.after));
    for (index, snapshot) in snapshots.enumerate() {
        let at = format!("{run}: snapshot {index}");
        assert_eq!(snapshot.index, index as u64, "{at}");
        assert!(!snapshot.accounts.is_empty(), "{at}: accounts");
        let scene = snapshot
            .scene
            .as_ref()
            .unwrap_or_else(|| panic!("{at}: no scene"));
        scene
            .validate()
            .unwrap_or_else(|error| panic!("{at}: {error}"));
        scene
            .validate_evidence(&snapshot.accounts)
            .unwrap_or_else(|error| panic!("{at}: {error}"));
    }
    // An asset the view's format function reads in units has its format;
    // one it leaves a count has none.
    let formatted: Vec<String> = match &view.formats {
        None => Vec::new(),
        Some(formats) => initial
            .asset_keys()
            .into_iter()
            .filter(|asset| {
                let format: Dynamic = engine
                    .call(
                        &formats.script,
                        formats.function.as_str(),
                        (asset.to_string(),),
                    )
                    .unwrap();
                !format.is_unit()
            })
            .map(|asset| format!("{ontology}/asset/{asset}"))
            .collect(),
    };
    let formats: Vec<&str> = document
        .formats
        .keys()
        .map(|asset| asset.as_str())
        .collect();
    assert_eq!(formats, formatted, "{run}: formats");
    Size {
        frames: document.frames.len(),
        accounts: document.initial.accounts.len(),
        rates: model.rates.len(),
    }
}

/// How big a drawn document is: its frames, the accounts it starts with and
/// its model's rates; of several, the most of each.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Size {
    frames: usize,
    accounts: usize,
    rates: usize,
}

impl Size {
    fn most(self, other: Self) -> Self {
        Self {
            frames: self.frames.max(other.frames),
            accounts: self.accounts.max(other.accounts),
            rates: self.rates.max(other.rates),
        }
    }
}

/// The least each sample's Showcase runs must draw, so no refactor quietly
/// collapses Showcase back into a toy: the longest run's frames, the most
/// accounts a run starts with, and the most rates its model has.
fn showcase_pressure(sample: &str) -> Size {
    let (frames, accounts, rates) = match sample {
        "maze" => (8, 2, 19),
        "sokoban" => (20, 36, 100),
        "exact-cover" => (5, 1, 40),
        "workshop" => (7, 1, 4),
        "scheduling" => (20, 6, 80),
        "rescue" => (7, 2, 70),
        "bridge" => (10, 3, 18),
        "marketplace" => (4, 14, 4),
        "logistics" => (40, 7, 50),
        "connect-four" => (15, 50, 200),
        "mission" => (6, 5, 39),
        "perishables" => (10, 5, 13),
        "work-league" => (60, 20, 900),
        "amm" => (14, 11, 10),
        "lending" => (14, 10, 14),
        other => panic!("no Showcase pressure for {other}"),
    };
    Size {
        frames,
        accounts,
        rates,
    }
}

/// Every run of every experiment of `write`'s package at the instances
/// `larger` says, Micro's or the others', drawn by its view as [`draws`]
/// checks, Showcase's as big as [`showcase_pressure`] requires. Returns how
/// many runs it drew.
fn draws_every_run(name: &str, write: WritePackage, larger: bool) -> usize {
    let engine = Engine::new(Store::memory().unwrap()).unwrap();
    write(&engine).unwrap();
    let mut drawn = 0;
    let mut showcase = Size::default();
    for experiment in engine.experiments().unwrap() {
        // An experiment is named for its sample and instance first, as
        // `rescue/micro/monte-carlo`.
        let instance = experiment.as_str().split('/').nth(1).unwrap();
        if (instance != "micro") != larger {
            continue;
        }
        let results = engine
            .run_experiment(&experiment, &mut |_, _| ControlFlow::Continue(()))
            .unwrap();
        for arm in &results.arms {
            for (index, trial) in arm.trials.iter().enumerate() {
                let run = format!("{name} {experiment} {} {index}", arm.arm.strategy);
                let size = draws(&engine, trial.end, &run);
                if instance == "showcase" {
                    showcase = showcase.most(size);
                }
                drawn += 1;
            }
        }
    }
    if larger {
        let least = showcase_pressure(name);
        assert!(showcase.frames >= least.frames, "{name}: {showcase:?}");
        assert!(showcase.accounts >= least.accounts, "{name}: {showcase:?}");
        assert!(showcase.rates >= least.rates, "{name}: {showcase:?}");
        // The standard board stays compact.
        if name == "connect-four" {
            assert!(showcase.rates < 300, "{name}: {showcase:?}");
        }
    }
    drawn
}

/// Each sample's view draws every run of its experiments at Micro or at
/// the larger instances.
fn views_draw_every_run(larger: bool) {
    for (name, write) in packages() {
        assert!(
            draws_every_run(name.as_str(), write, larger) > 0,
            "{name} drew nothing"
        );
    }
}

#[test]
fn every_view_draws_every_micro_run() {
    views_draw_every_run(false);
}

#[test]
#[ignore = "minutes in a debug build; CI runs it in a release build"]
fn every_view_draws_every_larger_run() {
    views_draw_every_run(true);
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
