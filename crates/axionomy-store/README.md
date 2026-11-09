# axionomy-store

`axionomy-store` keeps what Axionomy runs produce: states, the exchanges
between them, and the tags, groups, metrics and refs that organize them. It
stores economies identified by paths (`axionomy::paths`) in one
[redb](https://crates.io/crates/redb) database, on disk or in memory.

The kernel decides; the store remembers. A state is what replaying a trace
produces, so stored states are a cache of replay and replay verifies them.

- **States** are canonical trees (`axionomy-tree`): equal states have equal
  roots, whatever their history, and share their nodes.
- **Runs** start from a stored state, or fork from a point of another run, and
  record each exchange with the digest of its receipt. Their checkpoint rule
  says which states are stored; any other state is the nearest stored one
  plus replay.
- **One writer** commits many callers' batches per transaction. Each write
  states its durability: `Immediate` for callers that wait, `Eventual` for
  batches nobody waits on, durable within a second.
- **Tags, metrics, groups and refs** organize runs for evaluation, Monte
  Carlo, MPPI and training; none of them changes what an exchange does.

```rust
use axionomy::{Quantity, Rate, basket, path::Path, paths};
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
let mut recorder = store.record(economy, Checkpoints::Ends);
recorder.apply(
    paths::Exchange::new(path("pay"), Quantity::new(30))
        .bind(path("from"), path("bank"))
        .bind(path("to"), path("alice")),
)?;
let end = recorder.finish()?;

let reader = store.read()?;
let state = reader.state(end)?.expect("the run reached this point");
assert_eq!(state.economy.balance(&path("alice"), &path("cash")), Quantity::new(30));
reader.verify(end.run)?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

See [`docs/STORAGE.md`](../../docs/STORAGE.md) for the design.
