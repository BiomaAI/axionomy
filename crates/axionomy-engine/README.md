# axionomy-engine

`axionomy-engine` is the work every Axionomy front end shares. An `Engine`
holds a `Store` and nothing registered, and works on points: a point is one
saved state of an economy, the economy after a given step of a recorded run,
named by the run and the step. A point has one name, in the run whose step
reached it; an authored economy is the first point of a run of its own. A
run forked from a point starts from that point's state, and the state keeps
its name: the engine never names it by the fork, and refuses it so. Scripts, strategies, experiments, views and
packages join it as [the plan](../../docs/SIMULATION.md#plan-axionomy-on-primitives)
goes.

- `author` records an economy as the first point of a run of its own.
- `apply` applies exchanges at a point, all of them or none, and records them
  as a run forked from that point; the point it started from never changes. A
  refusal names the exchange that failed and its assessment against the
  economy it met.
- `reach` gives a point's state and the receipts of the steps its run took to
  reach it.
- `ancestor` names the point at an earlier step of a point's history, by its
  one name, in whichever run of that history took the step.
- `divergence` compares two points' histories: the last point on both (refused
  with `Unrelated` when there is none, as for economies authored apart), and
  for each step after it in a window, paged like `frames` with `after` and a
  limit, the exchange each side applied (none once its history has ended, its
  state staying where it was), and the balances, exact, and the accounts
  that differ between the two states after it, read from their trees' diff so
  a step costs what differs, not what the economy holds. It names the first
  step after which the two states are equal again, by their tree hashes,
  wherever it is on the histories, and measures objectives at both points.
  `shared` names the point they part at alone.
- `assess` explains whether an exchange would apply at a point, and
  `present` gives exchanges as people read them at a point, with the view
  of its run's ontology, each assessed there, as Studio shows a candidate
  before anyone applies it.
- `keep_script` keeps a Rhai script by its path, refusing one that does not
  compile or whose imports do not check out, and `call` runs one of its
  functions (D-049).
- `keep_goal`, `keep_objective` and `keep_candidates` keep a goal, an
  objective (a script's measure of a state, or its step costs summed over a
  history, with a direction) and a candidate set (a script function returning
  exchanges); `holds`, `measure` and `candidates` evaluate them at a point, and
  `measure_state`, `step_cost` and `propose` on an economy in memory.
- `keep_strategy` keeps a strategy, an algorithm naming the candidates and
  scripts it uses: how to choose, never what for. `run_strategy` runs one
  from a point toward a goal the store keeps, with an effort: a seed, a
  budget of expansions, which only searches spend, and turns, which only
  policies and games play, a run ending `Horizon` when it has played them
  all. The start, the goal and the effort are the caller's question
  (D-052); what a strategy keeps of its own, such as a beam's width or a
  player's iterations, describes how it chooses. It runs a policy turn by
  turn, a plan a script makes whole (such as a
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
  of two proposers moving from one point the second looks again. Asking again
  for a move already made, the ref at `to`, succeeds: a ref only moves
  forward, so it cannot be at `to` by another route. `delete_ref` deletes a
  ref the same way, from where its deleter saw it; its points stay. A ref a
  package ships is fixed: `shipped_by` names the package, and moving or
  deleting it is refused with `Shipped`. `frames` delivers the steps a ref
  was moved over after the point a follower last handled, each with its
  point, named as its mover advanced the ref to it, its exchange and receipt
  and the follower's objectives and goal there; a step only tried out is
  never delivered.
  `wait` blocks until a ref moves, or a time is up.
- `keep_experiment` keeps an experiment: a ref to start from, the goal every
  arm works toward, strategies as arms (each from that start or a ref of its
  own), objectives, the effort every arm is given and a number of trials,
  and `experiments` lists those the store keeps. `run_experiment` runs every
  arm's trials toward the goal with that effort over the same seeds,
  records the runs as a store group of kind `experiment` and keeps the
  results, which `results` reads back. Its watcher hears each run's
  progress with the arm and trial it is `Playing`; a watcher that stops a
  search stops the experiment, which keeps no results then, so the results
  it kept before stay.
- `compare` gives points' objective vectors and the front no other point
  dominates; `pick` the best point by objectives in order.
- `export` copies a package (refs and definitions and everything they reach,
  every script its scripts import among them) into a new store with a
  manifest; `load` installs one in the engine's store after checking each
  manifest's store format and script host API, in one transaction. A
  package is a release: the store keeps its manifest, less the names it
  already gave something else, as what that version shipped, and another
  version replaces it, removing what the old one shipped (the scripts its
  scripts import and the results of its experiments among it) and the new
  one does not carry, and forgetting every script compiled and module
  evaluated; the same version again changes nothing. Runs and steps stay,
  and so do refs and definitions a person made. `manifests` lists the
  packages the store holds; `package` opens a package file's bytes, a redb
  file or a zip of one, wherever they came from, and `zipped` zips one.
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
randomness is only a `Random` the host seeds (below); scripts have no clock;
and a call stops after a bounded
number of operations, its calls nested at most 64 deep in every build. Scripts are kept in the
store as definitions (`script/<path>`), so they travel with a package.

A `Random` draws two kinds of chance. The world's chance is drawn by named
event: `random.next(event, bound)` draws a number below `bound`, and
`random.sample(event, weights)` the index of one of `weights`, integers or
quantities, where `event` is a path naming the occurrence the draw is for,
such as `weather/direct-out/12`, the weather of the direct road out met at
time 12. The same event drawn with the same seed gives the same outcome
whatever else was drawn before it, and different events are drawn
independently, so a run that makes another choice meets the same luck
wherever it meets the same event, and a counterfactual compares the choice,
not the luck. A name tells each occurrence apart: the weather of a road at
each time it is travelled, an attempt at a job by its count, a scenario
drawn once as `scenario`. A name that is not a path is a script error. The
draw is `axionomy_search::sampling::EventTickets` of the seed and the
event's path, so Rust code draws the same event alike, and
`sample(event, weights)` samples exactly as `sampling::sample` does, with
its refusals; `next(event, bound)` is the index `sample(event, weights)`
draws from `bound` equal weights. A decision-maker's own randomness, such as
a rollout exploring, is drawn in sequence: `random.next(bound)` and
`random.sample(weights)` take the next of their sequences, `sample` drawing
the tickets `axionomy_search`'s sampler of the same seed draws. Draws by
event never move a sequence.

Every generator of a run draws the world's events from the run's seed: a
policy's, a plan's, a game's Nature's, and a game's players', whose own
sequence is apart from Nature's. A tree search's rollouts draw from the
search's seed, a world the search imagines rather than the run's.

A script imports another the store keeps by its path,
`import "exact-cover/algorithm-x" as algorithm;`, and calls its functions
through that name, `algorithm::cover(...)`. Imports resolve over the
scripts the store keeps and nothing else, never a file or the network; each
script imported is compiled once and evaluated once as a Rhai module, and
sees the same host API and the same guarantees as the script importing it.
An import names its script by a literal path, so the scripts a script
imports are known before it runs: `export` carries every one, transitively,
and a package that loads runs. `keep_script` refuses a script that imports
one the store does not keep, so a script is kept after the scripts it
imports; one whose imports lead back to it, directly or through others;
and one that, or any script it imports, directly or not, defines a function
of the same name and number of parameters as another of them, since Rhai
resolves a function by its name and number of parameters once per call, for
the script called and every script it imports alike, and could call either
for the other. A script a package brings is checked the same way when it is
first called.

Rhai calls a script's own function in method style before a host method of
the same name: a script that defines `view(state)` makes `state.view(...)`
call it, with `state` as `this`. Name script functions apart from the host's
(`balance`, `balances`, `accounts`, `rates`, `roles`, `consumed`, `produced`,
`preserved`, `view`, `applicable`, `assess`, `has_account`, `parameter`,
`rate`, `next`, `sample`). A loop variable
needs a name: Rhai refuses `for _ in ...`. A view's script builds its scene on
the `scene` module (`scene::graph(title, nodes, edges, focus)`, `grid`,
`matrix`, `timeline`), which infers entities, paths and legend from the
surface; Rhai functions do not see a script's top-level constants, so a
table a function needs is itself a function. Inside a closure, a variable
named like one of the script's functions reads as a pointer to that
function: name variables apart from functions. `array.sort(comparer)`
treats a comparer that fails as finding its two items equal, so a broken
comparer sorts wrongly rather than failing. `shared` is a keyword, not a name
a variable may have. An imported script's functions see neither the
importing script's variables nor their own script's top-level ones, and a
call they make to a function their script does not define finds the
importing script's before the host's: an imported script calls its own
functions and the host's only. Each call into an imported script is a call
level of its own.

The host API's version, `HOST_API`, stays 1 while Axionomy is in
development: scripts are written against the API as it stands, and the
version starts counting with the first release packages are published
against.

The engine is synchronous and knows no protocol: MCP calls it from blocking
tasks, and so do the Studio server and the CLI (D-045). It builds for
`wasm32-unknown-unknown` too, Rhai included, and runs in Studio's browser
Worker over a store on the origin private file system (D-047); `wait`,
which blocks a thread until a ref moves, is for hosts with threads. A
transport never implements an operation the engine has.

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
