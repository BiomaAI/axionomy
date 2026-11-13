# Simulation: primitives, scripts and packages

Status: steps 1 to 5 below are built (step 2 for MCP). Of the
[plan](#plan-axionomy-on-primitives) that follows them, P1 to P8 are done and
P9 to P13 follow in order.

The economy owns commitments and their consequences. A policy reads what an
actor may see and proposes exchanges. Whoever calls the rollout (an
application, a script, an MCP client, an outside harness) decides when
execution advances. Where an action has got to lives in assets, not in policy
state: a worker
holding a job, travelling, busy until a tick or awaiting an outcome is a
balance, so every simulation resumes, replays and forks for free.

Agents, models, prompts and learning stay outside, in the Autonomy Harness
that [CONTINUOUS_AGENT_SYSTEMS.md](CONTINUOUS_AGENT_SYSTEMS.md) describes.
This plan only changes Axionomy's side of that boundary, and adds as little
as it can: each step reuses what exists.

## What exists

| Need | Already there |
| --- | --- |
| Possible actions | Rates, `Economy::applicable`, `ActionSource` |
| Whether an action applies, and why not | `Economy::assess`, with its shortfalls |
| What an actor may see | Actor views (`agent_view`, `responder_view`), ISMCTS information states |
| Choosing | Plain Rust: `Option::or_else` for priorities, `Iterator::min_by_key` for ranking by an ordered key, ties broken by id |
| Stepping, in memory or recorded | `rollout::run` over `&mut impl Apply`; `Episodes` for repeated experiments, with `Unrecorded` or a `GroupRecorder` |
| Bounded computation | Search sessions: `advance(WorkBudget)`, interrupted and resumed |
| Several exchanges, all or nothing | `Economy::replay` and `replayed`, Lean's `Execution.replay`, MCP's `axionomy_trace_replay` |

Ordered keys, not floating-point scores, rank choices: today's policies rank
by tuples such as `(time + travel, urgency, u64::MAX - value)`, and a scalar
utility is the special case of a one-element key.

## Names

Each term means one thing. Axionomy's machinery prefers words a user's
ontology is unlikely to need, but an occasional overlap with a domain word is
fine (problems call their state a world; search has sessions); what matters
is that overlaps stay few and no Axionomy term means two things.

| Term | Means |
| --- | --- |
| Economy (`World` in a sample) | The authoritative state; the only thing that changes |
| Exchange | One firing of a rate |
| Receipt | What the kernel returns for an exchange it accepted: what moved, opened and closed |
| Proposal | One exchange offered for applying; never a trace |
| Proposer | Anything that proposes exchanges: a person, an AI agent, a simulator, a script. The kernel decides what applies |
| Trace | Exchanges in order |
| Replay | Applying a trace, all of it or none of it: `replayed` gives the economy after it, `replay` applies it in place |
| Run | A trajectory: a trace from a state, kept in memory or recorded in the store |
| Point | One saved state of the economy: the economy after a given step of a recorded run, named by the run and the step. Every point has one history, the exchanges that led to it |
| Fork | An independent economy starting from a chosen state of a run (a clone, constant-time) |
| Counterfactual | A forked run that examines an alternative to what the run did |
| Rollout | A run a policy drives, to a goal, a stop or a horizon |
| Episode | One rollout of a Monte Carlo estimate: one trial |
| Trial | One seeded run: one sample of a Monte Carlo estimate, or of an experiment's arm |
| Group | The runs produced together, such as an experiment's or one Monte Carlo estimate's |
| Policy | How an actor chooses; in a package, a Rhai script |
| Decision | One choice a policy makes |
| Strategy | A built-in algorithm, the scripts it calls and its parameters (seed, budget, horizon, actor) |
| Objective | A measure of a state with a direction, minimize or maximize |
| Experiment | A start point, strategies as arms, objectives, a seed, a budget and a number of trials; it produces runs, recorded as a group |
| Comparison | Objective vectors, dominance and the Pareto front, over any runs |
| Thinking | Trying exchanges at a point and playing them out before choosing, with the same `apply` as the step finally chosen; the search observations of how a run was found |
| Ref | A named label on a point, like a Git branch name on a commit. An application keeps a ref on the point its game or simulation is actually at, and moving the ref forward is what makes a step count; only the steps a ref is moved over are delivered |
| Learning | Updating a policy from experience; done outside Axionomy, by the Autonomy Harness ([CONTINUOUS_AGENT_SYSTEMS.md](CONTINUOUS_AGENT_SYSTEMS.md)) |
| Nature response | What happens without anyone choosing: an outcome resolves, a vehicle arrives, a deadline expires; a proposer like any other |
| Tick | The clock advancing |
| Horizon | The most steps a rollout may take |
| Script | Rhai source a package carries for anything a generic algorithm cannot know: candidates, policies, Nature responses, heuristics, objectives, view logic |
| Package | A redb file, zipped optionally, holding everything above for an ontology; loaded, it is what an engine continues from |
| Engine | `axionomy-engine`: the work every front end shares, natively and in the browser, with nothing registered |
| Study | Retiring in P9: today's request to study a problem, replaced by an experiment on a package |
| Path | An identifier's one written form, such as `edge/key-room/gate`, and its order. Typed identifiers derive `Identifier`, so they are written and ordered as their paths, and a typed run written down reads as a `paths` run that MCP can explore (D-044) |

An experiment is named for what it is: the same start, seed and budget,
strategies as arms, each measured on the same objectives. Monte Carlo's
single sample, which its code still calls an experiment, is a trial.

None of these names is a mode or a separate execution system. A game or a
simulation is an application built from the same operations: it applies
exchanges, forks, rolls out and records runs, and schedules that work in its
own loop. Live is not a kind of run either: it is a ref an application
advances. A package is not a mode: loaded, it is data the same operations use.

## Step 1: simulations run through `rollout::run` (done)

Five simulations hand-wrote their loop. Three shared one shape, one proposal
per step taken from a priority order:

```rust
// logistics::fleet, mission::expedition and rescue::incident, as controllers
let exchange = nature_response(world)
    .or_else(|| close_if_applicable(world))
    .or_else(|| actors.find_map(|actor| decision(&view(world, actor), actor, policy)))
    .unwrap_or_else(|| tick(world));
```

That body is now a rollout controller, and the goal its terminal condition:
`fleet::run`, `expedition::run` and `incident::run` take
`&mut impl Apply` and return the rollout's result, so a horizon or a
rejection reaches the service as an error instead of a panic, and every
Studio replay stayed byte for byte the same. `work_league`'s continuous
season (due events, then every worker once in a rotation, then the tick) and
`cold_chain`'s day plan keep their own loops until a second problem shares
their shape.

Names became consistent on the way: `response` and
`required_nature_response` are `nature_response`, and the six
`type Proposal = Trace<Ontology>` aliases are gone, since a proposal is one
exchange.

## Step 2: studies record their runs (built for MCP)

A study records each document's trajectory as a store run and rebuilds its
frames from the stored steps when they are read, instead of keeping the whole
view as JSON. What is not a trajectory (Pareto fronts, telemetry, Monte Carlo
estimates, search observations) is small and stays with the study.

- MCP keeps runs in its store instead of whole studies as JSON.
- The Studio server opens a store (in memory unless a file is given, as MCP
  does), so a study can be reopened and scrubbed from stored roots.
- GitHub Pages keeps its pre-built replays: it has no server.

This step does not depend on step 1: every study already builds its documents
from an initial state and a trace, so it records them the same way.

Done when MCP's store holds no view JSON and the Studio server reopens a
recorded study.

Built for MCP, which needed nothing new in the store for it:

- `StudyObserver::trajectory` reports each document's initial economy and
  trace written as paths (D-044), from the shared helpers every problem
  builds its documents with. Studio and the browser build ignore it, and
  their replays are byte for byte what they were.
- MCP records each document as two economies with the code that records
  every other: its initial state as a run of its own, like an authored
  economy, and its trace as a run forked from it, one step per exchange, as
  `axionomy_trace_replay` would. A study's summary lists both handles for
  every document, so a client steps into any state of a study, tries another
  exchange there, and diffs it against the study's next step.
- MCP's store keeps a study's key and its documents' handles, not its views.
  Rather than rebuilding frames from stored steps, a study missing from memory
  is computed again from its key, which is deterministic, and its documents
  are served only if they replay the trajectories the study recorded; a study
  recorded by another version is refused and studied again. Work-league's
  showcase study took a 135 MB store file as JSON and takes 8.4 MB as runs.

The Studio server's store comes with the engine (P2); studies themselves are
replaced by experiments on packages (P9).

## Step 3: studies are named studies (done)

Studio's and MCP's computation is named a study, in one commit with the Python
interop suite. The study runtime computes a `Study` from a `StudyRequest`, keyed by
its `StudyKey` and controlled and observed through `StudyControl` and
`StudyObserver`. The Studio server serves `/api/studies`, mints
`StudioStudyId`s (`study-7`) and streams `study_started` … `study_failed`
events; MCP's `axionomy_problem_study` returns a `StudySummary` whose `stu-`
handle is read under `axionomy://study/{study_id}`; the CLI's command is
`axionomy study`. "Run" means a trajectory everywhere.

## Step 4: replay is all or nothing

`replayed` kept nothing when an exchange failed, but `replay` kept the
exchanges before the failure, so two names that read as one operation, in
place and on a copy, meant different things. Both now apply a whole trace or
none of it:

- A failed `replay` leaves the economy unchanged, and `ReplayError` names the
  exchange that failed, counted from zero.
- Lean's `Execution.replay` states it; `replay_rejection_atomic`,
  `replay_success` and `replay_error_index` prove it, and the conformance
  fixtures are regenerated from Lean.
- Several exchanges committed together are a trace given to `replay`. `Apply`
  gains `replay` when a recorder first needs to commit several exchanges at
  once.

A commitment that fits one rate stays one rate: a rate is already an atomic
transition across accounts.

## Step 5: outside work, as one example

Work done outside the economy (a robot, a model, a person) is modelled, not
orchestrated:

- Asking is an exchange that produces a request asset, such as
  `Requested(id)`.
- The result is a rate that consumes it, so a duplicate report is infeasible
  and the kernel rejects it.
- A timeout is a Nature response that turns it into `Lapsed(id)`: the outcome
  is unknown, not failed.
- A crash between asking and doing loses nothing: the request is in the
  state, and recording made the state durable.

`test_outside_work_is_a_request_the_economy_holds` in
`crates/axionomy-mcp/interop/test_mcp.py` shows the pattern with a stand-in
for the outside party, through MCP's existing tools: a duplicate report and a
report after the lapse are refused, and the lapse cannot fire before its
deadline. MCP needed nothing new for it.

## Plan: Axionomy on primitives

Axionomy has to be complete without problems or studies. Points, runs, refs,
goals, objectives, strategies, experiments and comparison are primitives
every front end uses with nothing registered; what a user puts together for
an ontology (economies to start from, goals, objectives, the scripts that
propose exchanges, experiments and views) is a package: a redb file the
engine loads and continues from. The samples become packages like any user's.

### Model

| Layer | Holds | Knows |
| --- | --- | --- |
| Kernel (`axionomy`) | Economy, exchange, receipt, trace, goal, and the rules for what a rate means, specified in Lean; rates themselves are data in the economy | Nothing above it |
| Store (`axionomy-store`) | Runs, points, refs, groups and the state tree's nodes, in redb, every record binary (postcard) | The kernel |
| Search (`axionomy-search`) | Generic algorithms over any ontology, experiments, comparison | The kernel |
| Engine (`axionomy-engine`) | Operations on points, runs and refs; runs scripts, strategies and experiments; draws views | Store, search, Rhai; no package, no registry |
| Scripts (Rhai 1.27.0) | Every proposer a package carries | The engine's host API |
| Packages | redb files, zipped optionally: economies as runs, refs, goals, scripts, strategies, experiments with their results, views, a manifest | Nothing; they are data |
| Transports (`axionomy-mcp`, `axionomy-studio-server`) | Protocols over the engine | No package |
| Applications (the `axionomy` binary, Studio's browser Worker) | Open stores, load packages, serve | The packages they are given |

Retired as the plan completes (P9): `Problem`, `Registry`, `Study` and
Studio's study events, and with them `axionomy-study`.

### Rules

- One apply: `apply(point, exchanges)` applies all of them or none and gives
  a new point; the point it started from never changes. Making a move,
  trying one out, branching and a counterfactual are this one operation.
- Every point has one history: the exchanges that led to it.
- A ref moves only by `advance(ref, expected, to)`, refused, with the point
  the ref is on, when it is no longer on `expected`. Only the steps a ref is
  moved over are delivered, each with the point it led to.
- Every proposer a package carries is a Rhai script. Rust is the machinery
  (kernel, store, engine, the generic algorithms, fast host helpers); data
  is the structure (which algorithm, its parameters, an experiment's arms,
  view layout, the manifest).
- Rates and goals stay kernel data. A script proposes an exchange of an
  existing rate; the kernel decides whether it applies and what it does.
- Outside proposers (people in Studio, AI agents over MCP or HTTP,
  simulators such as MuJoCo) send exchanges through a transport, generated
  however they like. Only a valid exchange gets through.
- Nothing is registered: an engine works with whatever its store holds.
- No JSON in storage. JSON stays only where a protocol defines it (MCP's
  JSON-RPC, the HTTP API to the browser) and in the Lean conformance
  fixtures, which are the specification's test output.

### How points and refs work, in connect-four

A game is a run: the empty board, then one exchange per move. Each board
along the way is a point, such as the board after move 7.

- To make a move, a player applies its exchange to the board. That gives a
  new board; the board it started from is unchanged.
- The application keeps a ref, `live`, on the board the game is actually at.
  Moving `live` to the new board is what makes the move count.
- If two players both move from board 7, the first to move `live` wins. The
  second is refused and told the game is now at board 8, so it looks again
  and chooses again.
- A player thinks by applying moves to board 7 and playing them out without
  moving `live`. Those boards are saved like any others, but nothing that
  follows `live` sees them.
- Whatever follows `live` (Studio, a learner, a robot or other executor)
  receives each move `live` is moved over, with the board it led to. A move
  that was only tried out is never delivered, so it cannot reach an executor.
- A player is any proposer: a person in Studio, an AI agent over MCP or
  HTTP, a policy in the same process, a loop applying Nature responses. The
  engine does not know which.

Any economy works the same way, with an economy for the board and an
exchange for the move.

### Scripts

Rhai scripts are how a package says anything a generic algorithm cannot
know: candidate generators, policies and decision rules, Nature and chance
responses, heuristics, objectives that are not a plain balance, and the
logic in views (anchors, status and tone ladders, metrics). The engine gives
them a host API: balances, the accounts under a prefix, rates, `applicable`,
`assess`, building exchanges, shortest paths, calling a built-in rollout or
search, and a seeded random generator.

- Quantities enter Rhai as an exact type with checked operations, never as
  floats.
- Randomness comes only from the host's seeded generator, so an experiment
  replays from its seed.
- A package's manifest records the host API version its scripts were
  written against.
- A script called at every node of a large search, or millions of times in
  rollouts, can be slow; hot steps move into Rust host helpers, and
  WebAssembly modules stay a later option.

The inventory of the samples sized this: their generic searches (BFS,
Dijkstra, A*, best-first, Pareto) need only candidate scripts, heuristics and
objectives; their tactical policies (triage, dispatch, bidding, daily plans,
market actors, connect-four tactics) are domain programs, which is what the
scripts are for; and branch-and-bound, beam search, minimax with alpha-beta
and ordered depth-first search (Algorithm X) are generic algorithms the
samples wrote by hand, which join `axionomy-search`.

### Steps

Each step is one or more validated commits, pushed with CI green before the
next. Built already: the engine records points and MCP keeps only handles
(`36ce883`); the transports know no problem and the `axionomy` binary serves
everything (`c59a521`).

**P1. Binary records (done).** The kernel's values have a complete binary
form: every field, quantities as big-endian bytes, expressions tagged by
variant, while JSON keeps its shape (serde's `is_human_readable` tells the
two apart). The store keys a state tree by identifiers' paths and writes its
values, records and receipt digests in postcard, and records its format (1),
refusing any other; MCP's handle tables and the operations its handles are
minted from are postcard too. JSON is left only in protocols.

**P2. One engine, no registry (done).** `Engine::new(store)`. MCP takes the
registry its study tools use beside its engine until P9 replaces them. The
CLI and the Studio server hold an engine as soon as they use one: the CLI for
packages (P8), the Studio server for points and refs (P10, P12).

**P3. Rhai scripts in the engine (done).** `rhai = "1.27.0"`; the host API
above, exact quantities, seeded randomness, and scripts kept in the store as
definitions (`script/<path>`), a new store table merged like refs. Maze's A*
heuristic, sokoban's candidates and incident's Nature, view and triage
(`crates/axionomy-problems/scripts`) give exactly what their Rust code gives
on every state checked, and the scripted incident runs the same trace. Per
call, in release, scripts take 3 to 5 times the Rust code's time (maze's
heuristic 3.6 µs against 1.4 µs, sokoban's candidates 283 µs against 87 µs,
an incident turn 340 µs against 65 µs), since most of a call is the host
API's kernel work; `examples/script_speed.rs` measures it.

**P4. Goals, objectives and candidates (done).** Goals are kernel data kept
by path (`goal/<path>`); an objective is a script measure with a direction,
of the state (`fn(state)`) or summed over a point's whole history, across
the runs it forked through (`fn(before, exchange, after)`, such as interest
accrued); a candidate set is a script function returning exchanges. The
engine evaluates them at any point (`holds`, `measure`, `candidates`) and on
any economy in memory, as searches need (`measure_state`, `step_cost`,
`propose`). Transports expose them with packages and experiments (P9) and
Studio (P12).

**P5. Strategies (done, ISMCTS with P9).** A strategy is a built-in algorithm (priority, BFS,
Dijkstra, A*, best-first, rollout, Monte Carlo, MCTS, ISMCTS, Pareto search,
and the new branch-and-bound, beam search and minimax with alpha-beta) with
the scripts it calls and its parameters (seed, budget, horizon, actor). The
engine runs one from a point and gets a run and its thinking. MCP's
breadth-first search becomes one such strategy.

Built so far: a strategy is a definition (`strategy/<path>`) naming its goal
and candidate set, and a candidate set is a script or a list of exchanges
reconsidered at each state. `run_strategy` runs a policy turn by turn
(`fn(state, random)`, ending at the goal, when it proposes nothing, at a
refusal or at its horizon), breadth-first search, or the cheapest search in
Dijkstra's or A*'s order over a script's step costs and heuristic, within a
budget of expansions that a watcher may also stop, and records what it
reaches as a run forked from its start. Maze's A* and Dijkstra find routes
as cheap as the Rust solvers', A* the same trace; sokoban's breadth-first
search the shortest solutions; incident's policies the same incidents; and
MCP's `axionomy_search` runs the breadth-first strategy over its goal and
candidates, kept as definitions named by their content.

`axionomy-search` gained the algorithms the samples wrote by hand, generic
over the ontology (`bounded`): depth-first branch-and-bound for the cheapest
goal state under a lower bound, beam search keeping the best-scoring states
of each depth, and minimax with alpha-beta pruning for a two-player choice.
Strategies use them, and a game strategy plays turn by turn: Nature's
response first, then the player an actor script names, choosing by its own
policy, by minimax, or by Monte Carlo tree search over every player's
payoffs, seeded per turn so a game replays. ISMCTS, which needs a package to
say what an actor observes and how hidden worlds are sampled, joins with the
mission sample in P9.

**P6. Experiments and comparison (done).** An experiment is a start point,
strategies as arms, objectives, a seed and a budget, with Monte Carlo over
strategies and weighted scenarios, receding-horizon replanning, and
counterfactual variants (a trace played first, an actor dropped). Running one
records a store group of runs. Comparison works over any runs: objective
vectors, dominance, the Pareto front, and picking a run by lexicographic
objectives. Monte Carlo's `Experiment` becomes `Trial`.

Built: an experiment is a definition (`experiment/<path>`) naming the ref
it starts from, its arms, objectives, seed, budget and number of trials. An
arm is a strategy, from the experiment's start or from a ref of its own,
which is how a counterfactual variant is run: a trace played first, or an
actor's holdings withdrawn, is a point with a ref. Every arm runs the same
trials, trial `i` seeded from the experiment's seed and `i`, so arms meet
the same chances. `run_experiment` records every trial's run in a store
group of kind `experiment`, with each objective as a metric named
`<strategy>/<objective>`, and keeps the results (each trial's seed, end
point, how it ended and its objective values, exactly) beside the
experiment. A game's Nature draws from a random stream of its own,
`fn(state, random)`, so weighted scenarios are Nature sampling them and
receding-horizon replanning is a player that searches again every turn,
and how hard the players think never changes what Nature does. `compare`
gives any points' objective vectors and the front no point dominates, and
`pick` the best by objectives in order. The same seed re-runs the same
trials; transports run experiments once packages carry them (P9).

**P7. Views (done).** A view per ontology: labels, formats, scene structure (surface
kind, node or cell source, layout, edges) and script logic (anchors, status
and tone ladders, metrics, classes that depend on the trace); leaderboards
are ranked expressions. Any run is drawn with its ontology's view, or with
its paths when there is none. Runs keep the ontology they were written from,
and any run reads as `Paths` (D-044).

Built: a view is a definition kept per ontology (`view/<ontology>`) naming
script functions, each optional: `label(item)` for an account, asset, rate
or role written `account/<path>` and so on; `format(asset)`;
`carry(carried, before, exchange, after)` for what a view carries from frame
to frame, such as each actor's running contribution; `scene(state, frame)`,
where `frame` is `#{ index, exchanges, carried }`, so classes may depend on
the whole trace; and `leaderboards(state, frame)`, each entry with an integer
score the engine ranks, ties sharing a rank. A scene is a map shaped like the
view crate's `Scene`, built on the `scene` module's constructors
(`scene::graph`, `grid`, `matrix`, `timeline`), which infer entities, paths
and legend as every view does. A view's functions live in a script of their
own, so a policy or search never compiles them. `draw(point)` draws a
point's history as a document with the view of its run's ontology, or with
its paths. The store reads any run as `Paths`, and a fork written as paths
keeps the ontology of the run it forked from. Maze's view, written as a
script, draws every document of every maze study exactly as its Rust view
does.

Drawing maze from the store found that an economy iterated its accounts and
rates in creation order while the store reads them back in identifier order,
so a strategy could decide differently from the same point depending on
whether it was in memory. An economy now keeps both in identifier order,
specified and proved in Lean (D-051).

**P8. Packages (done).** A manifest (name, version, format version, host API
version, contents). Export copies what is reachable from chosen refs (the
points, the runs they come from, the tree nodes under their states, the
definitions they use) into a new store file, which redb compacts. Load
merges a file into the engine's store, its refs under the package's name.
Zip is optional both ways. `axionomy package export`, `load` and `inspect`.
Done when an exported package, loaded into a fresh store, re-runs its
experiments with identical results.

Built: `export(name, version, contents, to)` takes refs and definitions,
everything named under the package's name when nothing narrower is chosen,
and closes over what they reach: the definitions they use (an experiment's
strategies, objectives and results; a strategy's goal, candidates and
scripts; a view's scripts), the refs an experiment starts from, the view of
every ontology whose runs come along, and the points an experiment's
results name. The store copies those refs, definitions and groups, every run
they name back through the runs they forked from, with steps, states, pins,
tags and metrics, and every tree node under those states; nothing else.
Every ref must be named under the package's name, so loading never moves
anyone else's refs. The manifest is a definition (`package/<name>`) naming
the package, its version, the store format, the script host API and what it
carries. `load` refuses a package whose scripts need a newer host API, then
merges it; names the store already gives something else keep what they are
and are reported. A package file is a redb file or a zip holding one, told
apart by its first bytes, so `package(bytes)` opens either in memory, as
the browser will. A package exported, compacted, zipped and loaded into a
fresh store carries its experiment's results and re-runs the experiment to
the same trials.

**P9. The samples become packages.** The 15 samples' builders write their
packages: economies for each instance and each seed shipped, goals, scripts,
strategies, experiments, views. A strategy key means one thing at every
size. Done when every experiment reproduces today's study outcomes
(objectives and traces) from its package alone; then `Problem`, `Registry`,
`Study`, `axionomy-study` and `export-studies` go, MCP's problem tools become
package and experiment tools, Studio's catalog lists the loaded packages, and
Pages ships the sample packages, built in CI and never committed.

Built so far: each sample's builder (`<sample>::package::write`, behind
`axionomy-problems`' `packages` feature) writes its package into an engine's
store, and `axionomy package samples <directory>` writes every one, zipped.
Maze's package (its three instances as maze runs, its search and view
scripts, goal, candidates, energy and time objectives, five strategies and an
experiment per instance) reproduces every document of every maze study
exactly, traces and objectives, loaded alone into a fresh store. It needed
one more built-in strategy: an exact Pareto search over state objectives
that reaches the front's best point by those objectives in order.
Sokoban's (A* over a crate-to-goal lower bound and breadth-first search,
and each instance's legal but losing plan recorded under a ref)
reproduces its studies the same way, and so does exact cover's
(breadth-first search, and Algorithm X as a plan a script makes whole,
reading set memberships from what each selection consumes; each instance's
unsatisfiable twin, where search runs out and Algorithm X plans nothing).
Workshop's (breadth-first search, best-first search over its waste, and
Pareto picks for least waste, least time and highest revenue from a front
over all three) needed best-first search as a strategy, and a Pareto
strategy that ranks its front by some of the objectives that span it.
Bridge's (breadth-first routing, its first-come, auction and
deadline-responsive policies played turn by turn, and Micro's exact Pareto
picks for each traveller) needed nothing new: where timed traffic's study
prefers, for a traveller, the best of the policies it evaluates, `pick`
finds it among the experiment's runs. Rescue's (Micro's weighted prior with
the scenario its study plays, and the Showcase and Stress incidents, an
ontology of their own whose goal and policies are named under
`rescue/incident`) reproduces every policy's document, and a Monte Carlo
experiment of 128 trials plays each sampled scenario of its study's front,
which needed scripts to sample weighted outcomes with the tickets
`axionomy_search`'s sampler draws. Scheduling's (best-first search over the
makespan, the branch optimizer as branch-and-bound under the clock, and
Pareto picks for either job finishing first, over a script dispatching the
executed shop's events in the study's order; each instance's horizon too
short for the work, where every search runs out) needed nothing new. The
micro instances are checked with
every test run; the showcase and stress instances, minutes in a debug build,
run in a release build in CI. Rhai is optimised in debug builds too, since package
scripts run in its interpreter.

**P10. Live refs.** The store's refs gain compare-and-set and an index of each
run's branches; the engine gains `refer`, `advance`, `watch` and
`frames(after)`, which pulls from the last point a reader handled. Every
transport offers them; MCP mints a handle for a ref as it does for a point.
A frame is one step as delivered: the point it led to, its exchange frame,
the objectives there and whether the goal holds, so a learner receives whole
transitions as they happen.

Built in the store and engine: refs move by compare-and-set, and the store
indexes every fork (format 2). `refer` names a point with a new ref;
`advance(ref, expected, to)` moves it forward over the steps between, on
`to`'s history, refused with where the ref is when it is not on `expected`;
`frames(ref, after, following, limit)` delivers the steps it was moved over
after `after`, each with its point, exchange, receipt, and the follower's
objectives and goal there; `wait(ref, after, timeout)` blocks until it moves.
The transports follow.

**P11. The engine in the browser.** The store's writer runs inline where
there are no threads, `getrandom`'s `wasm_js` feature is on for wasm only,
`Store::on(backend)` opens any redb backend, an origin private file system
(OPFS) backend keeps the store across reloads, and Rhai runs on wasm32. A
package loads from its bytes wherever they come from: natively from a file,
in the browser from a URL the Worker fetches (unzipping a zipped one),
opened as a read-only store over the bytes and merged into the Worker's
store, its manifest's format and host API versions checked first; with OPFS
a loaded package stays across reloads and is fetched again only when the URL
serves a newer version. A package on another host needs that host to allow
cross-origin requests. Done when a Worker loads a package from a URL, runs
an experiment and finds both again after a reload.

**P12. Studio on packages, points, refs and experiments.** Load a package
(from a file, or from a URL, which a link such as `?package=<url>` opens;
Pages lists its sample packages by URL), scrub, follow a ref, act (a candidate or a written exchange, assessed first;
on the point a ref is on it also moves the ref, anywhere else it makes a
branch), continue from here, run an experiment, compare. The native and the
browser engine offer the same.

**P13. Acceptance.** A Python test drives the native server over HTTP with the
connect-four package: two players advance `live`, one thinking by applying
moves at `live`'s point without moving it, while a learner follows `live`
through `frames(after)`.

## Not planned

- A runner or scheduler framework: P13's loop is the first application loop,
  and one is extracted when a second application shares it.
- WebAssembly strategy modules, unless Rhai's speed demands them.
- MCAP or RRD files.
- Learning inside Axionomy, and bridges to robots or other executors: the
  Autonomy Harness's. A bridge follows a ref, resumes with `frames(after)` and
  stops on `Switched`.
- MuJoCo or language-model executors, which are outside proposers.
