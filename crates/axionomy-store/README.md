# axionomy-store

`axionomy-store` keeps what Axionomy runs produce: states, the exchanges
between them, and the tags, groups, metrics and refs that organize them, in
one [redb](https://crates.io/crates/redb) database, on disk or in memory, and
as planned (D-047) in a browser Worker, in memory or on the origin private
file system. It stores economies of any `axionomy::Ontology` whose identifiers
serialize: a problem's own Rust types, or paths (`axionomy::paths::Paths`). A
run records its ontology's name and is read back only as that ontology.

The kernel decides; the store remembers. A state is what replaying a trace
produces, so stored states are a cache of replay and replay verifies them.

- **States** are canonical trees (`axionomy-tree`): equal states have equal
  roots, whatever their history, and share their nodes.
- **Runs** start from a stored state, or fork from a point of another run, and
  record each exchange with the digest of its receipt. Their checkpoint rule
  says which states are stored; any other state is the nearest stored one
  plus replay. A `Recorder` implements `axionomy::Apply`, so a rollout,
  simulation or policy steps it as it would an economy, and its caller
  decides whether the steps are kept.
- **One writer** commits many callers' batches per transaction. Each write
  states its durability: `Immediate` for callers that wait, `Eventual` for
  batches nobody waits on, durable within a second.
- **Tags, metrics, groups and refs** organize runs, whatever produced them:
  search, simulation, evaluation, Monte Carlo, MPPI, agents or learning; none
  of them changes what an exchange does. Planned (D-046): an application
  keeps a ref on the point its game or simulation is actually at; refs gain
  compare-and-set, so several proposers move one without overwriting each
  other, and readers follow it step by step.
  A `GroupRecorder` records a group's members as they end, such as one
  Monte Carlo estimate's rollouts, and waits on disk once, when it finishes.
- **Reading back** gives any state of a run, its steps as replay-buffer
  transitions between state roots, and the first step where a lasting
  condition holds, by bisecting the stored states.

```rust
use axionomy::{Quantity, Rate, basket, path::Path, paths::{self, Paths}};
use axionomy_store::{Checkpoints, Store};

fn path<K>(text: &str) -> Path<K> {
    text.parse().unwrap()
}

let economy = paths::EconomyBuilder::new()
    .account(path("bank"), basket([(path("cash"), 100)]).into())
    .account(path("alice"), basket([]).into())
    .rate(
        path("pay"),
        Rate::new()
            .consume(path("from"), basket([(path("cash"), 1)]))
            .produce(path("to"), basket([(path("cash"), 1)])),
    )
    .build()?;

let store = Store::memory()?;
let mut recorder = store.record::<Paths>(economy, Checkpoints::Ends);
recorder.apply(
    paths::Exchange::new(path("pay"), Quantity::new(30))
        .bind(path("from"), path("bank"))
        .bind(path("to"), path("alice")),
)?;
let end = recorder.finish()?;

let reader = store.read()?;
let state = reader.state::<Paths>(end)?.expect("the run reached this point");
assert_eq!(state.economy.balance(&path("alice"), &path("cash")), Quantity::new(30));
reader.verify::<Paths>(end.run)?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

See [`docs/STORAGE.md`](../../docs/STORAGE.md) for the design.
