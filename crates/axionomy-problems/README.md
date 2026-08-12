# axionomy-problems

Canonical closed-problem encodings for
[Axionomy](https://github.com/BiomaAI/axionomy).

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

Each model has a matching runnable example:

```console
cargo run -p axionomy-problems --example maze
cargo run -p axionomy-problems --example sokoban
cargo run -p axionomy-problems --example exact_cover
cargo run -p axionomy-problems --example workshop
cargo run -p axionomy-problems --example scheduling
cargo run -p axionomy-problems --example rescue
cargo run -p axionomy-problems --example bridge
cargo run -p axionomy-problems --example marketplace
cargo run -p axionomy-problems --example logistics
cargo run -p axionomy-problems --example connect_four
cargo run -p axionomy-problems --example mission
cargo run -p axionomy-problems --example perishables
cargo run -p axionomy-problems --example work_league
```

The perishables fixture also demonstrates structural fungibility: thousands of
equivalent claims aggregate into ordinary quantities, while unique cohort
condition facts behave non-fungibly through unit supply and lifecycle
invariants. A derived event agenda and holdings index accelerate decisions but
never authorize state changes.

Examples emit structured console events through `tracing`. `INFO` is the
default and explains model construction, strategy choice, encoded outcomes,
and replay verification. Set `RUST_LOG=debug` to include complete accepted
exchange traces and assessments:

```console
RUST_LOG=debug cargo run -p axionomy-problems --example maze
```

Logging is initialized only by these example binaries. Problem modules,
search, and kernel libraries never install a global subscriber.

Replay-derived presentation adapters for all thirteen problems live uniformly
in `axionomy-service`. Keeping them out of this crate prevents a singled-out
viewer from becoming part of a problem's semantics: these modules remain pure
domain encodings and strategy helpers usable without Studio.

Performance probes are Cargo bench targets rather than examples:

```console
cargo bench -p axionomy-problems --bench rollout_throughput
```
