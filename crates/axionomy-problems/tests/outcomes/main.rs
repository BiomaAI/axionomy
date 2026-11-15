//! What every sample's package finds, pinned: for each arm of each of its
//! experiments, how its trials ended, how many steps they took in all, each
//! objective summed over them, and a digest of what every trial did; and
//! for each of its refs, how many steps its history has and a digest of it.
//! A package is the sample: what it finds is checked against what it found
//! when it was pinned, never against another implementation of it.
//!
//! A change that means to change what a package finds rewrites the pins:
//! `AXIONOMY_PINS=$PWD/crates/axionomy-problems/tests/outcomes/pins.rs cargo
//! test --release -p axionomy-problems --features packages --test outcomes
//! pins_as_found -- --ignored`, and says in its commit why they changed.
#![cfg(feature = "packages")]

// As `pins_as_found` writes it, so writing it again changes only what changed.
#[rustfmt::skip]
mod pins;

/// A pinned arm: its experiment, its strategy, how its trials ended, their
/// steps in all, each objective summed, and the digest of what they did.
type PinnedArm = (
    &'static str,
    &'static str,
    &'static str,
    u64,
    &'static [&'static str],
    &'static str,
);

/// A pinned ref: its name, its steps and the digest of its history.
type PinnedRef = (&'static str, u64, &'static str);

use axionomy::paths;
use axionomy_engine::{Ended, Engine, ExperimentPath, PackagePath, Version};
use axionomy_store::Store;
use std::{fmt::Write as _, ops::ControlFlow};

/// What an arm of an experiment found over its trials.
#[derive(Debug, PartialEq, Eq)]
struct Found {
    experiment: String,
    strategy: String,
    /// How the trials ended, each way with how many trials ended so.
    ended: String,
    /// The steps the trials took, in all.
    steps: u64,
    /// Each objective, summed over the trials.
    totals: Vec<String>,
    /// BLAKE3 of how each trial ended, its values and its exchanges.
    digest: String,
}

/// What a ref names: how many steps its history has, and a digest of its
/// first state and its exchanges.
#[derive(Debug, PartialEq, Eq)]
struct Named {
    reference: String,
    steps: u64,
    digest: String,
}

/// `write`'s package, exported from the store it was written in and loaded
/// alone into a fresh one, as a package is shipped.
fn loaded(name: &PackagePath, write: axionomy_problems::WritePackage) -> Engine {
    let source = Engine::new(Store::memory().unwrap()).unwrap();
    write(&source).unwrap();
    let file = Store::memory().unwrap();
    source
        .export(
            name,
            "0.1.0".parse::<Version>().unwrap(),
            source.named_under(name).unwrap(),
            &file,
        )
        .unwrap();
    let engine = Engine::new(Store::memory().unwrap()).unwrap();
    engine.load(&file).unwrap();
    engine
}

fn hex(hash: blake3::Hash) -> String {
    hash.to_hex().to_string()
}

fn ended_words(ended: Ended) -> String {
    match ended {
        Ended::Reached => "reached".into(),
        Ended::Stopped => "stopped".into(),
        Ended::Horizon => "horizon".into(),
        Ended::Refused { turn } => format!("refused on {turn}"),
        Ended::Exhausted => "exhausted".into(),
        Ended::Spent => "spent".into(),
        Ended::Interrupted => "interrupted".into(),
    }
}

/// Runs `experiment` and says what each of its arms found. Every trial's
/// run replays as it was recorded, and a trial that reached the goal ends
/// where the goal holds.
fn found(engine: &Engine, experiment: &ExperimentPath) -> Vec<Found> {
    let definition = engine.experiment(experiment).unwrap();
    let results = engine
        .run_experiment(experiment, &mut |_, _| ControlFlow::Continue(()))
        .unwrap();
    results
        .arms
        .iter()
        .map(|arm| {
            let mut endings: Vec<(String, usize)> = Vec::new();
            let mut steps = 0;
            let mut digest = blake3::Hasher::new();
            for trial in &arm.trials {
                let words = ended_words(trial.ended);
                match endings.iter_mut().find(|(said, _)| *said == words) {
                    Some((_, count)) => *count += 1,
                    None => endings.push((words, 1)),
                }
                steps += trial.end.step - arm.start.step;
                let (_, exchanges) = engine.history(trial.end).unwrap();
                digest.update(
                    &postcard::to_allocvec(&(&trial.ended, &trial.values, &exchanges)).unwrap(),
                );
                engine
                    .store()
                    .read()
                    .unwrap()
                    .verify::<paths::Paths>(trial.end.run)
                    .unwrap_or_else(|error| panic!("{experiment} {}: {error}", arm.arm.strategy));
                if trial.ended == Ended::Reached {
                    assert!(
                        engine.holds(trial.end, &definition.goal).unwrap(),
                        "{experiment} {}: reached, but the goal does not hold",
                        arm.arm.strategy
                    );
                }
            }
            Found {
                experiment: experiment.to_string(),
                strategy: arm.arm.strategy.to_string(),
                ended: endings
                    .iter()
                    .map(|(words, count)| {
                        if arm.trials.len() == 1 {
                            words.clone()
                        } else {
                            format!("{words} {count}")
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(", "),
                steps,
                totals: arm.totals().iter().map(ToString::to_string).collect(),
                digest: hex(digest.finalize()),
            }
        })
        .collect()
}

/// Every ref `engine`'s package names, and its history.
fn named(engine: &Engine) -> Vec<Named> {
    let mut named = Vec::new();
    for manifest in engine.manifests().unwrap() {
        for reference in &manifest.refs {
            let point = engine.referred(reference).unwrap();
            let history = engine.history(point).unwrap();
            named.push(Named {
                reference: reference.to_string(),
                steps: point.step,
                digest: hex(blake3::hash(&postcard::to_allocvec(&history).unwrap())),
            });
        }
    }
    named
}

/// An experiment of a sample's micro instance, which a debug build runs in
/// seconds; the larger instances' run in a release build.
fn micro(experiment: &ExperimentPath) -> bool {
    experiment.as_str().split('/').nth(1) == Some("micro")
}

/// What every sample's experiments for which `which` holds find, sample by
/// sample, each in a package loaded alone.
fn found_where(which: impl Fn(&ExperimentPath) -> bool) -> Vec<Found> {
    let mut all = Vec::new();
    for (name, write) in axionomy_problems::packages() {
        let engine = loaded(&name, write);
        for experiment in engine.experiments().unwrap() {
            if which(&experiment) {
                all.extend(found(&engine, &experiment));
            }
        }
    }
    all
}

fn pinned(which: impl Fn(&str) -> bool) -> Vec<Found> {
    pins::ARMS
        .iter()
        .filter(|(experiment, ..)| which(experiment))
        .map(
            |&(experiment, strategy, ended, steps, totals, digest)| Found {
                experiment: experiment.into(),
                strategy: strategy.into(),
                ended: ended.into(),
                steps,
                totals: totals.iter().map(|total| (*total).into()).collect(),
                digest: digest.into(),
            },
        )
        .collect()
}

#[test]
fn every_micro_experiment_finds_what_is_pinned_twice_over() {
    let first = found_where(micro);
    // A package loaded again, in another store, finds the same.
    assert_eq!(first, found_where(micro));
    assert_eq!(
        first,
        pinned(|experiment| micro(&experiment.parse().unwrap()))
    );
}

#[test]
#[ignore = "minutes in a debug build; CI runs it in a release build"]
fn every_larger_experiment_finds_what_is_pinned() {
    let which = |experiment: &ExperimentPath| !micro(experiment);
    assert_eq!(
        found_where(which),
        pinned(|experiment| which(&experiment.parse().unwrap()))
    );
}

#[test]
fn every_ref_names_what_is_pinned() {
    let mut all = Vec::new();
    for (name, write) in axionomy_problems::packages() {
        all.extend(named(&loaded(&name, write)));
    }
    let pinned: Vec<Named> = pins::REFS
        .iter()
        .map(|&(reference, steps, digest)| Named {
            reference: reference.into(),
            steps,
            digest: digest.into(),
        })
        .collect();
    assert_eq!(all, pinned);
}

/// Writes what every package finds now as the pins, to the file
/// `AXIONOMY_PINS` names; without it, writes nothing.
#[test]
#[ignore = "rewrites the pins when asked"]
fn pins_as_found() {
    let Ok(file) = std::env::var("AXIONOMY_PINS") else {
        return;
    };
    let mut text = String::from(
        "//! What every sample's package found when it was pinned, as\n\
         //! `pins_as_found` writes it: each arm as (experiment, strategy, how its\n\
         //! trials ended, their steps in all, each objective summed, digest), and\n\
         //! each ref as (ref, steps, digest).\n\n\
         pub const ARMS: &[super::PinnedArm] = &[\n",
    );
    for found in found_where(|_| true) {
        let totals = found
            .totals
            .iter()
            .map(|total| format!("\"{total}\""))
            .collect::<Vec<_>>()
            .join(", ");
        writeln!(
            text,
            "    (\"{}\", \"{}\", \"{}\", {}, &[{totals}], \"{}\"),",
            found.experiment, found.strategy, found.ended, found.steps, found.digest
        )
        .unwrap();
    }
    text.push_str("];\n\npub const REFS: &[super::PinnedRef] = &[\n");
    for (name, write) in axionomy_problems::packages() {
        for named in named(&loaded(&name, write)) {
            writeln!(
                text,
                "    (\"{}\", {}, \"{}\"),",
                named.reference, named.steps, named.digest
            )
            .unwrap();
        }
    }
    text.push_str("];\n");
    std::fs::write(file, text).unwrap();
}
