# axionomy-engine

`axionomy-engine` is the work every Axionomy front end shares. An `Engine`
holds a `Store` and nothing registered, and works on points: a point is one
saved state of an economy, the economy after a given step of a recorded run,
named by the run and the step. Scripts, strategies, experiments, views and
packages join it as [the plan](../../docs/SIMULATION.md#plan-axionomy-on-primitives)
goes.

- `author` records an economy as the first point of a run of its own.
- `apply` applies exchanges at a point, all of them or none, and records them
  as a run forked from that point; the point it started from never changes. A
  refusal names the exchange that failed and its assessment against the
  economy it met.
- `reach` gives a point's state and the receipts of the steps its run took to
  reach it.
- `ancestor` names the point at an earlier step of a point's history.
- `assess` explains whether an exchange would apply at a point.
- `keep_script` keeps a Rhai script by its path, refusing one that does not
  compile, and `call` runs one of its functions (D-049).
- `keep_goal`, `keep_objective` and `keep_candidates` keep a goal, an
  objective (a script's measure of a state, or its step costs summed over a
  history, with a direction) and a candidate set (a script function returning
  exchanges); `holds`, `measure` and `candidates` evaluate them at a point, and
  `measure_state`, `step_cost` and `propose` on an economy in memory.
- `keep_strategy` keeps a strategy, an algorithm naming the goal, candidates
  and scripts it uses, and `run_strategy` runs one from a point with a seed
  and a budget: a policy turn by turn, a plan a script makes whole (such as a
  specialised solver's answer, or a match whose chances it draws from the
  run's seed), applied all or none, breadth-first search,
  the cheapest search (Dijkstra's as `axionomy_search::dijkstra` runs it,
  breaking ties between equally cheap plans as the samples' Rust solvers do,
  or A* given a heuristic), best-first search over a state's cost,
  branch-and-bound, beam search (answering with the first or the last of the
  goal states scoring best alike), an exact Pareto search that
  reaches the front's best point by some of its objectives in order, or a
  game whose players choose by policy, minimax or Monte Carlo tree search,
  which may draw Nature's weighted outcomes as chance in its tree and
  simulate by a rollout script drawing the search's own tickets. A player
  who sees only part of the economy
  chooses by information-set tree search: a script says what it observes,
  a view, and its tree's nodes, candidates and simulations read only that;
  each iteration searches a world drawn from what it believes, the point's
  history replayed with every one of Nature's draws drawn again every way
  Nature could have drawn it, kept while the player would have seen it as
  it saw the real one. What it reaches is recorded as a run forked from its start, with how it
  ended and what the search explored.
- `refer` names a point with a new ref and `referred` reads it. `advance`
  moves a ref forward from the point its mover expects to a point whose
  history passes through it, and refuses with where the ref is otherwise, so
  of two proposers moving from one point the second looks again. `frames`
  delivers the steps a ref was moved over after the point a follower last
  handled, each with its point, exchange and receipt and the follower's
  objectives and goal there; a step only tried out is never delivered.
  `wait` blocks until a ref moves, or a time is up.
- `keep_experiment` keeps an experiment: a ref to start from, strategies as
  arms (each from that start or a ref of its own), objectives, a seed, a
  budget and a number of trials. `run_experiment` runs every arm's trials
  over the same seeds, records the runs as a store group of kind
  `experiment` and keeps the results, which `results` reads back.
- `compare` gives points' objective vectors and the front no other point
  dominates; `pick` the best point by objectives in order.
- `export` copies a package (refs and definitions and everything they reach)
  into a new store with a manifest; `load` merges one into the engine's store
  after checking it; `manifests` lists those the store holds; `package`
  opens a package file's bytes, a redb file or a zip of one, and `zipped`
  zips one.
- `keep_view` keeps how people read an ontology's runs: script functions that
  label its identifiers, format its quantities, carry what the view needs
  from frame to frame, draw a scene and give leaderboard scores. `draw`
  draws a point's history as a document with the view of its run's
  ontology, or with its paths when there is none.

Scripts are how a package says what a generic algorithm cannot know. They
read a `ScriptState`, the whole economy or a view of the accounts an actor may
see, through the host API (balances, accounts and rates by prefix, a rate's
roles and what it fixes for a role to consume, produce or preserve,
`exchange(...)`, `applicable`, `assess`, `distances` over a list of edges),
and propose exchanges; they never change an economy. Quantities are
exact, with checked arithmetic and `mul_div` that names its rounding;
randomness is only a `Random` the host seeds (`next(bound)` draws a number
below a bound, and `sample(weights)` the index of a weight, drawn as
`axionomy_search::sampling` samples weighted outcomes with a sampler of the
same seed, so a script samples Nature's scenarios as a Rust Monte Carlo
seeded alike does, and `keyed(key, bound)` the ticket keyed by what it is
for, `splitmix(seed ^ key, 1) % bound`, the same whatever else is drawn);
scripts have no clock; and a call stops after a bounded
number of operations. Scripts are kept in the
store as definitions (`script/<path>`), so they travel with a package.

Rhai calls a script's own function in method style before a host method of
the same name: a script that defines `view(state)` makes `state.view(...)`
call it, with `state` as `this`. Name script functions apart from the host's
(`balance`, `balances`, `accounts`, `rates`, `roles`, `consumed`, `produced`,
`preserved`, `view`, `applicable`, `assess`, `has_account`, `parameter`,
`rate`, `next`, `sample`, `keyed`). A loop variable
needs a name: Rhai refuses `for _ in ...`. A view's script builds its scene on
the `scene` module (`scene::graph(title, nodes, edges, focus)`, `grid`,
`matrix`, `timeline`), which infers entities, paths and legend from the
surface; Rhai functions do not see a script's top-level constants, so a
table a function needs is itself a function.

The engine is synchronous and knows no protocol: MCP calls it from blocking
tasks, and so will the Studio server, the CLI and Studio's browser Worker
(D-045). A transport
never implements an operation the engine has.

```rust
use axionomy::{Quantity, Rate, basket, path::Path, paths};
use axionomy_engine::{Engine, EngineError};
use axionomy_store::Store;

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

let engine = Engine::new(Store::memory()?)?;
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
