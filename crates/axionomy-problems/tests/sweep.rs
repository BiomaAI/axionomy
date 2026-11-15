//! Every strategy Studio offers at every ref of every sample package runs:
//! given a small effort from the point the ref names, it ends with an
//! outcome, whatever it is, never an engine error. Studio offers at a point
//! the strategies of the experiments' arms that start from a ref in the
//! same history tree, or, when none does, those of the experiments on the
//! same ontology; and every strategy no experiment compares, toward the
//! first goal it offers, at its default effort (`playable` in
//! `studio/src/PackagesApp.tsx`).
#![cfg(feature = "packages")]

use axionomy::{PathKind, path::Path};
use axionomy_engine::{Effort, Engine, GoalPath, Manifest, PackagePath, StrategyPath, Version};
use axionomy_store::{Point, RefPath, RunId, Store};
use std::ops::ControlFlow;

/// The effort Studio gives a strategy no experiment compares.
const DEFAULT_EFFORT: Effort = Effort {
    seed: 17,
    budget: 1_000_000,
    turns: 1_000,
};

/// The most states a search expands here, and the most turns a policy or a
/// game plays: enough to start every kind of run, little enough that none
/// runs long.
const BUDGET: u64 = 200;
const TURNS: u64 = 6;

/// `write`'s package, exported and loaded alone into a fresh store, as
/// Studio loads a sample.
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

/// The run the history of `point` grows from.
fn root(engine: &Engine, point: Point) -> RunId {
    engine
        .histories(&[point])
        .unwrap()
        .into_iter()
        .find(|segment| segment.from.is_none())
        .expect("a history has a root")
        .tip
        .run
}

/// The definitions of a kind `manifest` carries, by the prefix their names
/// are kept under, such as `strategy/`.
fn carried<K: PathKind>(manifest: &Manifest, prefix: &str) -> Vec<Path<K>> {
    manifest
        .definitions
        .iter()
        .filter_map(|definition| definition.as_str().strip_prefix(prefix))
        .map(|path| path.parse().unwrap())
        .collect()
}

/// One arm of an experiment: its strategy, the ref it starts from, and the
/// goal and effort its experiment gives it.
struct Arm {
    strategy: StrategyPath,
    start: RefPath,
    goal: GoalPath,
    effort: Effort,
}

/// What Studio offers at `point` of `manifest`'s package: each strategy
/// with the goal and the effort it is asked to run with.
fn offered(
    engine: &Engine,
    manifest: &Manifest,
    point: Point,
) -> Vec<(StrategyPath, GoalPath, Effort)> {
    let arms: Vec<Arm> = carried(manifest, "experiment/")
        .into_iter()
        .flat_map(|path| {
            let experiment = engine.experiment(&path).unwrap();
            experiment
                .arms
                .into_iter()
                .map(move |arm| Arm {
                    strategy: arm.strategy,
                    start: arm.start.unwrap_or_else(|| experiment.start.clone()),
                    goal: experiment.goal.clone(),
                    effort: experiment.effort,
                })
                .collect::<Vec<_>>()
        })
        .collect();
    let tree = root(engine, point);
    let ontology = engine.ontology(point).unwrap();
    let starts = |arm: &&Arm| engine.referred(&arm.start).unwrap();
    let mut here: Vec<&Arm> = arms
        .iter()
        .filter(|arm| root(engine, starts(arm)) == tree)
        .collect();
    if here.is_empty() {
        here = arms
            .iter()
            .filter(|arm| engine.ontology(starts(arm)).unwrap() == ontology)
            .collect();
    }
    let mut goals: Vec<GoalPath> = Vec::new();
    for arm in &here {
        if !goals.contains(&arm.goal) {
            goals.push(arm.goal.clone());
        }
    }
    goals.extend(
        carried(manifest, "goal/")
            .into_iter()
            .filter(|goal| !arms.iter().any(|arm| arm.goal == *goal)),
    );
    let mut offered: Vec<(StrategyPath, GoalPath, Effort)> = Vec::new();
    for arm in here {
        if !offered
            .iter()
            .any(|(strategy, ..)| *strategy == arm.strategy)
        {
            offered.push((arm.strategy.clone(), arm.goal.clone(), arm.effort));
        }
    }
    for strategy in carried(manifest, "strategy/") {
        if !arms.iter().any(|arm| arm.strategy == strategy) {
            let goal = goals.first().expect("a package offers a goal").clone();
            offered.push((strategy, goal, DEFAULT_EFFORT));
        }
    }
    offered
}

/// Runs every strategy offered at every ref of every sample for which
/// `which` holds, each with at most a small effort, and says how many ran.
fn sweep(which: impl Fn(&RefPath) -> bool) -> usize {
    let mut ran = 0;
    for (name, write) in axionomy_problems::packages() {
        let engine = loaded(&name, write);
        for manifest in engine.manifests().unwrap() {
            for reference in manifest.refs.iter().filter(|reference| which(reference)) {
                let point = engine.referred(reference).unwrap();
                for (strategy, goal, asked) in offered(&engine, &manifest, point) {
                    let effort = Effort {
                        seed: asked.seed,
                        budget: asked.budget.min(BUDGET),
                        turns: asked.turns.min(TURNS),
                    };
                    let definition = engine.strategy(&strategy).unwrap();
                    engine
                        .run_strategy(point, &definition, &goal, effort, &mut |_| {
                            ControlFlow::Continue(())
                        })
                        .unwrap_or_else(|error| {
                            panic!("{strategy} toward {goal} at {reference}: {error}")
                        });
                    ran += 1;
                }
            }
        }
    }
    ran
}

/// A ref of a sample's micro instance, such as `rescue/micro/scenario`.
fn micro(reference: &RefPath) -> bool {
    reference.as_str().split('/').nth(1) == Some("micro")
}

#[test]
fn every_strategy_offered_at_every_micro_ref_runs() {
    assert!(sweep(micro) > 0);
}

#[test]
#[ignore = "a minute in a debug build; CI runs it in a release build"]
fn every_strategy_offered_at_every_larger_ref_runs() {
    assert!(sweep(|reference| !micro(reference)) > 0);
}
