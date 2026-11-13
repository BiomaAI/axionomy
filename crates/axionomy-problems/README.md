# axionomy-problems

Canonical closed-problem encodings for
[Axionomy](https://github.com/BiomaAI/axionomy), their solvers, and the
packages they are written as.

Problem models exercise Axionomy exclusively through its public API. They are
reference encodings and conformance fixtures, not privileged engine semantics.
They are intentionally concrete examples, not reusable domain frameworks:
Axionomy users create their own economies. The fixtures prove that the common
axioms generalize, and their adversarial tests ensure accepted exchanges remain
domain-valid even when a caller bypasses the examples' action helpers and
rebinds roles directly. The mission additionally proves caller-owned belief
filtering and repeated planning across information sets.

The applicable fixtures also expose multi-objective decisions. Maze, workshop,
scheduling, bridge, marketplace, and perishables exhaust finite state spaces
and return exact fronts of replay-verified traces. Logistics, rescue, and
mission compare sampled policy statistics and therefore return explicitly
approximate fronts. In both cases the objective values come from encoded
outcomes; dominance is disposable comparison policy, never a parallel domain
model.

Every model's `World` has an `Ontology` naming its identifier types, such as
`maze::Ontology` or `logistics::fleet::Ontology`, and every identifier
serializes, so `axionomy-store` can record its runs: whatever explores a
model records each rollout as a run and a Monte Carlo estimate as a group of
them
(`tests/storage.rs`).

## One folder per problem

Each problem lives in one folder of `src`, named as its module (`maze`,
`work_league`):

- `README.md` explains the problem: its specification, the results its tests
  require, and the pressure it puts on the API. It is also the module's
  documentation.
- `mod.rs` is the model, with the model's own submodules beside it
  (`maze/dynamics.rs`): its economy builders and its solvers, policies,
  planners and games, written in Rust.
- `package.rs` writes the problem's package into an engine's store, behind
  the `packages` feature: its economies as runs under refs, its goals, its
  Rhai scripts (`scripts/`), its strategies, its experiments and its view.

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

A sample's Rust code is the oracle its package is checked against.
`tests/packages.rs` exports each package, loads it alone into a fresh store,
runs its experiments and requires exactly what the sample's Rust code gives:
the traces its solvers, policies, planners and games make and their values on
every objective, the plans it keeps beside them (a legal but losing Sokoban
plan, an unsatisfiable cover, too short a horizon) from the economies it
starts at, and the totals its Monte Carlo fronts average. Micro is checked in
every test run, Showcase and Stress in a release build (`--ignored`).
`tests/views.rs` draws every run of every experiment with its package's view
and checks each document: a frame for every step, each explained, scenes that
join up, evidence in the snapshot it explains, and formats for the assets the
view reads in units.

```rust
# #[cfg(feature = "packages")] {
use axionomy::path;
use axionomy_engine::Engine;
use axionomy_problems::maze;
use axionomy_store::Store;
use std::ops::ControlFlow;

let engine = Engine::new(Store::memory()?)?;
maze::package::write(&engine)?;
let results = engine.run_experiment(&path!("maze/micro"), &mut |_| ControlFlow::Continue(()))?;
let searched = results
    .arms
    .iter()
    .find(|arm| arm.arm.strategy.as_str() == "maze/breadth-first")
    .expect("the maze's experiment searches breadth first");
let (_, exchanges) = engine.history(searched.trials[0].end)?;
// The package's breadth-first search finds the route the Rust solver finds.
let route = maze::solve_bfs(&maze::initial()).expect("the smallest maze has an exit");
assert_eq!(exchanges.len(), route.trace().exchanges().len());
# }
# Ok::<(), Box<dyn std::error::Error>>(())
```

A problem of your own is a package of your own, written the same way:
record its economies as runs under refs, keep its goals, scripts, strategies,
experiments and view as definitions, and export them with `Engine::export`
or `axionomy package export`
([D-048, D-049](https://github.com/BiomaAI/axionomy/blob/main/PDD.md)).
Any sample's `package.rs` shows each step.

## Examples

Each model has a matching runnable example:

```console
cargo run -p axionomy-problems --example maze
cargo run -p axionomy-problems --example sokoban
cargo run -p axionomy-problems --example exact_cover
cargo run -p axionomy-problems --example workshop
cargo run -p axionomy-problems --example scheduling
cargo run -p axionomy-problems --example rescue
cargo run -p axionomy-problems --example bridge
cargo run -p axionomy-problems --example amm
cargo run -p axionomy-problems --example marketplace
cargo run -p axionomy-problems --example logistics
cargo run -p axionomy-problems --example connect_four
cargo run -p axionomy-problems --example mission
cargo run -p axionomy-problems --example perishables
cargo run -p axionomy-problems --example work_league
cargo run -p axionomy-problems --example lending
```

Examples emit structured console events through `tracing`. `INFO` is the
default and explains model construction, strategy choice, encoded outcomes,
and replay verification. Set `RUST_LOG=debug` to include complete accepted
exchange traces and assessments:

```console
RUST_LOG=debug cargo run -p axionomy-problems --example maze
```

Logging is initialized only by these example binaries. Problem modules,
search, and kernel libraries never install a global subscriber.

Performance probes are Cargo bench targets rather than examples:

```console
cargo bench -p axionomy-problems --bench rollout_throughput
```
