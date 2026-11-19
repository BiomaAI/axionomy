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

## Package sources and typed APIs

Each folder in `packages/` contains a complete editable package. The shipped
CLI embeds these files so an installed binary can rebuild the gallery.
`axionomy package samples DIR` builds the 15 archives and their Studio index.
[Package source and commands](../../docs/PACKAGE_SOURCE.md)

Each folder in `src/` exposes the problem's Rust ontology, typed actions,
goals and constructors. The constructors read canonical laws into concrete
Rust types. Seeded constructors supply their scenario's private holdings or
calendar rows. The native problem tests can change those inputs and directly
bind roles to test the economic laws. The optional `packages` feature exposes
the gallery installer through the engine.

## The samples as packages

Every package has economic assertions under its own `tests/`. The gallery also pins starting economies and ordered histories for every ref. `tests/outcomes` exports each package,
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
