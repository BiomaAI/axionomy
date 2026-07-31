# axionomy-problems

Canonical closed-problem encodings for
[Axionomy](https://github.com/BiomaAI/axionomy).

Problem models exercise Axionomy exclusively through its public API. They are
reference encodings and conformance fixtures, not privileged engine semantics.
The mission additionally proves that different hidden worlds with an identical
agent view produce the same public proposal set and information-set decision.

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
```

Examples emit structured console events through `tracing`. `INFO` is the
default and explains model construction, strategy choice, encoded outcomes,
and replay verification. Set `RUST_LOG=debug` to include complete accepted
exchange traces and assessments:

```console
RUST_LOG=debug cargo run -p axionomy-problems --example maze
```

Logging is initialized only by these example binaries. The reusable model,
search, and kernel libraries never install a global subscriber.

Performance probes are Cargo bench targets rather than examples:

```console
cargo bench -p axionomy-problems --bench rollout_throughput
```
