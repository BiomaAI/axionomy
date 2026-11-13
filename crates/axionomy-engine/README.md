# axionomy-engine

`axionomy-engine` is the work every Axionomy front end shares. An `Engine`
holds a `Registry` of problems and a `Store`, and works on points: a point is
one saved state of an economy, the economy after a given step of a recorded
run, named by the run and the step.

- `author` records an economy as the first point of a run of its own.
- `apply` applies exchanges at a point, all of them or none, and records them
  as a run forked from that point; the point it started from never changes. A
  refusal names the exchange that failed and its assessment against the
  economy it met.
- `reach` gives a point's state and the receipts of the steps its run took to
  reach it.
- `ancestor` names the point at an earlier step of a point's history.
- `assess` explains whether an exchange would apply at a point.

The engine is synchronous and knows no protocol: MCP calls it from blocking
tasks, and so will the Studio server, the CLI and Studio's browser Worker
([plan](../../docs/SIMULATION.md#interactive-execution), D-045). A transport
never implements an operation the engine has.

```rust
use axionomy::{Quantity, Rate, basket, path::Path, paths};
use axionomy_engine::{Engine, EngineError};
use axionomy_store::Store;
use axionomy_study::Registry;

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
let pay = paths::Exchange::new(path("pay"), Quantity::new(40))
    .bind(path("from"), path("bank"))
    .bind(path("to"), path("alice"));

let engine = Engine::new(Registry::new(), Store::memory()?)?;
let start = engine.author(economy)?;
let paid = engine.apply(start, &[pay.clone()])?;

// The point it started from is unchanged; the new one is a step on.
assert_eq!(paid.step, start.step + 1);
assert!(engine.reach(start)?.receipts.is_empty());
let reached = engine.reach(paid)?;
assert_eq!(reached.state.economy.balance(&path("alice"), &path("cash")), Quantity::new(40));

// Two more payments would overdraw the bank: nothing is recorded, and the
// refusal names the exchange that failed, counted from zero.
let refused = engine.apply(paid, &[pay.clone(), pay]).unwrap_err();
assert!(matches!(refused, EngineError::Refused { index: 1, .. }));
# Ok::<(), Box<dyn std::error::Error>>(())
```
