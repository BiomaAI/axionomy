# axionomy-problems

Canonical closed-problem encodings for
[Axionomy](https://github.com/BiomaAI/axionomy) and the packages they are
written as.

Problem models exercise Axionomy exclusively through its public API. They are
reference encodings and conformance fixtures, not privileged engine semantics.
They are intentionally concrete examples, not reusable domain frameworks:
Axionomy users create their own economies. The fixtures prove that the common
axioms generalize, and their adversarial tests ensure accepted exchanges remain
domain-valid even when a caller bypasses the examples' action helpers and
rebinds roles directly.

The applicable packages also expose multi-objective decisions. The Pareto
strategies of maze, workshop, scheduling, bridge, marketplace, and
perishables exhaust finite state spaces and return exact fronts of
replay-verified traces. The experiments of logistics, rescue, and mission
compare evaluated policies and therefore give explicitly approximate fronts.
In both cases the objective values come from encoded outcomes; dominance is
disposable comparison policy, never a parallel domain model.

Every model's `World` has an `Ontology` naming its identifier types, such as
`maze::Ontology` or `logistics::fleet::Ontology`, and every identifier
serializes, so `axionomy-store` can record its runs (`tests/storage.rs`
stores every model's economies and reads them back).

## One folder per problem

Each problem lives in one folder of `src`, named as its module (`maze`,
`work_league`):

- `README.md` explains the problem: its specification, the results its tests
  require, and the pressure it puts on the API. It is also the module's
  documentation.
- `mod.rs` is the model, with the model's own submodules beside it
  (`maze/dynamics.rs`): its ontology, economy builders, goals, typed
  exchange builders, and the constants its package reads.
- `package.rs` writes the problem's package into an engine's store, behind
  the `packages` feature: its economies as runs under refs, its goals, its
  Rhai scripts (`scripts/`), its strategies, its experiments and its view.
  The package's strategies are the problem's only solvers, policies and
  games; they run through the engine.

Sub-ontologies such as `logistics/fleet` follow the same layout inside their
problem's folder. A model uses neither its package nor its view, so it remains
a pure domain encoding usable without the engine or Studio, and no viewer
becomes part of a problem's semantics.

`packages()` lists every sample's package builder, in the order Studio lists
them; `axionomy package samples` writes them all, zipped, with the
`index.json` Studio lists them by. The suite's index, with its conformance
matrix and instance profiles, is
[`PROBLEMS.md`](https://github.com/BiomaAI/axionomy/blob/main/PROBLEMS.md).

## The samples as packages

A package is checked against what it found when it was pinned, never
against another implementation of it. `tests/outcomes` exports each package,
loads it alone into a fresh store, runs its experiments and requires what
`tests/outcomes/pins.rs` records: for each arm, how its trials ended, their
steps, each objective summed over them and a digest of what they did; for
each ref, its steps and a digest of its history. Micro is checked in every
test run, Showcase and Stress in a release build (`--ignored`). A change
that means to change what a package finds rewrites the pins
(`pins_as_found`) and says why in its commit.

`tests/views.rs` draws every run of every experiment with its package's view
and checks each document: a frame for every step, each explained, scenes that
join up, evidence in the snapshot it explains, and formats for the assets the
view reads in units.

`tests/sweep.rs` runs every strategy Studio offers at every ref of every
package, with a small effort, and requires each to end with an outcome, never
an engine error.

```rust
# #[cfg(feature = "packages")] {
use axionomy::path;
use axionomy_engine::Engine;
use axionomy_problems::maze;
use axionomy_store::Store;
use std::ops::ControlFlow;

let engine = Engine::new(Store::memory()?)?;
maze::package::write(&engine)?;
let results = engine.run_experiment(&path!("maze/micro"), &mut |_, _| ControlFlow::Continue(()))?;
let searched = results
    .arms
    .iter()
    .find(|arm| arm.arm.strategy.as_str() == "maze/breadth-first")
    .expect("the maze's experiment searches breadth first");
let (_, exchanges) = engine.history(searched.trials[0].end)?;
// Breadth first, the package takes the detour: two moves, then the finish.
assert_eq!(exchanges.len(), 3);
# }
# Ok::<(), Box<dyn std::error::Error>>(())
```

A problem of your own is a package of your own, written the same way:
record its economies as runs under refs, keep its goals, scripts, strategies,
experiments and view as definitions, and export them with `Engine::export`
or `axionomy package export`
([D-048, D-049](https://github.com/BiomaAI/axionomy/blob/main/PDD.md)).
Any sample's `package.rs` shows each step.
