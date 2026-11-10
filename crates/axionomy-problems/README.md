# axionomy-problems

Canonical closed-problem encodings for
[Axionomy](https://github.com/BiomaAI/axionomy), and how each is studied and
shown.

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

The lending fixture finances a bill through a collateralized loan. Interest
follows pool utilization along a kinked curve and accrues in a fixed-point
borrow index, a reserve cut stays with the market, and a period cannot close
on a loan past its liquidation threshold. Every `MulDiv` rounds against the
caller, so no deposit, borrow or repayment round trip can create value and
rounding dust accrues to the market.

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

Each problem lives in one folder: its model in `mod.rs` and the model's own
submodules (`maze/dynamics.rs`), its `Problem` in `study.rs` (the catalog
entry, its strategies and the documents a study holds, each derived by
replay), and its presentation in `view.rs` (labels, units, scene and
leaderboards). Sub-ontologies such as `logistics/fleet` follow the same layout
inside their problem's folder. `registry()` registers every problem, in
catalog order, for the CLI, MCP and Studio. A model uses neither its study nor
its view, so it remains a pure domain encoding usable without Studio, and no
viewer becomes part of a problem's semantics.

Performance probes are Cargo bench targets rather than examples:

```console
cargo bench -p axionomy-problems --bench rollout_throughput
```
