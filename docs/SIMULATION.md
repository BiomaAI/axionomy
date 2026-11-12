# Simulation: policies, runs and studies

Status: planned, in the order the steps below give. Built: rollouts and
Monte Carlo episodes recording where they run, steps 1, 3, 4 and 5, and step
2 for MCP. Planned: [interactive execution](#interactive-execution), steps 6
to 13, which also gives the Studio server the store step 2 left.

The economy owns commitments and their consequences. A policy reads what an
actor may see and proposes exchanges. Whoever calls the rollout (a problem, a
study, an MCP client, an outside harness) decides when execution advances.
Where an action has got to lives in assets, not in policy state: a worker
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
| Economy (`World` in a problem) | The authoritative state; the only thing that changes |
| Exchange | One firing of a rate |
| Receipt | What the kernel returns for an exchange it accepted: what moved, opened and closed |
| Proposal | One exchange offered for applying; never a trace |
| Trace | Exchanges in order |
| Replay | Applying a trace, all of it or none of it: `replayed` gives the economy after it, `replay` applies it in place |
| Run | A trajectory: a trace from a state, kept in memory or recorded in the store |
| Point | A state a run reached: its run and step. Every point has one history: its run's steps back to where the run forked, then the history of that point |
| Fork | An independent economy starting from a chosen state of a run (a clone, constant-time) |
| Counterfactual | A forked run that examines an alternative to what the run did |
| Rollout | A run a policy drives, to a goal, a stop or a horizon |
| Episode | One rollout of a repeated experiment, such as one Monte Carlo sample |
| Group | The runs produced together, such as one Monte Carlo estimate's episodes |
| Policy | How an actor chooses; a problem's `Policy` enum names its policies |
| Decision | One choice: a problem's `decision(view, actor, policy)` |
| Thinking | Playing out other histories from a point before choosing, with the same `apply` as acting; a study keeps its search's observations as each document's `thinking` |
| Ref | A name for a point that moves. The point an application is living is a ref it advances by compare-and-set; only the steps a ref passes through are delivered |
| Learning | Updating a policy from experience; done outside Axionomy, by the Autonomy Harness ([CONTINUOUS_AGENT_SYSTEMS.md](CONTINUOUS_AGENT_SYSTEMS.md)) |
| Nature response | What happens without anyone choosing: an outcome resolves, a vehicle arrives, a deadline expires; a problem's `nature_response(world)` |
| Tick | The clock advancing |
| Horizon | The most steps a rollout may take |
| Study | One request to investigate or compare outcomes, through Studio, MCP or the CLI: a problem, an instance, strategies, a seed and a budget. A study produces runs and the evidence of how they were found |
| Engine | `axionomy-engine`: the work on studies, economies and runs that every front end shares, natively and in the browser |
| Path | An identifier's one written form, such as `edge/key-room/gate`, and its order. Typed identifiers derive `Identifier`, so they are written and ordered as their paths, and a typed run written down reads as a `paths` run that MCP can explore (D-044) |

A study is not called a job, session, experiment or scenario because each
already means something else here: work-league's workers claim jobs, search
has sessions, Monte Carlo runs experiments, and Nature encodes scenarios.

None of these names is a mode or a separate execution system. A game or a
simulation is an application built from the same operations: it applies
exchanges, forks, rolls out and records runs, and schedules that work in its
own loop. Live is not a kind of run either: it is a ref an application
advances. `axionomy_study::Problem` is an entry in the catalog of problems
one can study, not the way an application runs; a continuous application
need not wrap its lifetime in a study.

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

The Studio server's store, and reopening a recorded study there, come with
the engine (step 7).

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

## Interactive execution

The aim, from an outside review: one application where a person acts, a
policy thinks, an outside learner updates that policy, and Studio inspects
both the live state and past ones, natively and entirely in the browser,
through the same operations.

### Rules

- One apply: `apply(point, exchanges)` gives a new point and never changes
  the point it started from. Acting, thinking, branching and trying a
  counterfactual are all this one operation.
- Every point has one history, so a trajectory is the history of a point.
- Live is a ref, not a kind of run. A ref names the point an application is
  living, and the application advances it by compare-and-set: an advance is
  refused, with the ref's current point, when someone advanced it first, so a
  participant who acted on an old state looks again.
- Thinking applies like anyone else and never advances the ref. Only the steps
  a ref passes through are delivered, each with its point, so an imagined
  move cannot reach an executor that follows the ref.

What tells a live run from a thought one is who proposes its exchanges and
whether anything delivers them, never the operations.

### Layers

| Crate | Owns |
| --- | --- |
| `axionomy` | The kernel |
| `axionomy-store` | Runs, points and refs in redb: a file natively; in the browser a file in the origin private file system (OPFS), or memory |
| `axionomy-study` | Problems, the `Registry` and studies |
| `axionomy-engine` (new) | All work on studies, economies and runs, shared by every front end |
| `axionomy-mcp`, `axionomy-studio-server`, `axionomy-cli`, `axionomy-studio-wasm` | Transports |

Today the work on economies and runs lives in MCP
(`crates/axionomy-mcp/src/{store,authoring,studies,read}.rs` and the apply
and search paths of `server.rs`), so only MCP clients can author, apply,
step, find, diff or page through an economy; the Studio server has no store
and the CLI only studies.

### `axionomy-engine`

Synchronous, without tokio or protocol types, and built natively and for
`wasm32-unknown-unknown`. Servers call it from blocking tasks; the browser's
Worker calls it directly.

```rust
pub struct Engine { registry: Registry, store: Store } // Clone

impl Engine {
    // Studies: each document's trajectory is recorded as a run.
    pub fn catalog(&self) -> Vec<ProblemDescriptor>;
    pub fn study(&self, request: &StudyRequest, progress: …) -> Result<Studied, EngineError>; // and each document's first and last point

    // Points.
    pub fn author(&self, economy: paths::Economy) -> Result<Point, EngineError>;
    pub fn extend(&self, at: Point, additions: …) -> Result<Point, EngineError>;
    pub fn apply(&self, at: Point, exchanges: &[paths::Exchange]) -> Result<Point, Refused>; // all or nothing: Refused { index, assessment }
    pub fn assess(&self, at: Point, exchange: &paths::Exchange) -> Result<paths::ExchangeAssessment, EngineError>;
    pub fn actions(&self, at: Point) -> Result<Vec<paths::Exchange>, EngineError>;
    pub fn ancestor(&self, at: Point, step: u64) -> Result<Point, EngineError>;
    pub fn find(&self, at: Point, condition: …) -> Result<Option<Point>, EngineError>;
    pub fn frames(&self, at: Point, after: u64, limit: usize) -> Result<Vec<Frame>, EngineError>;
    pub fn search(&self, at: Point, candidates: …, goal: …, budget: …, progress: …) -> Result<Option<Point>, EngineError>;
    // and paged reads: accounts, balances, rates, invariants, changes, diff

    // Live.
    pub fn refer(&self, name: &RefPath, at: Point) -> Result<(), EngineError>;
    pub fn advance(&self, name: &RefPath, expected: Point, to: Point) -> Result<(), Moved>; // Moved { current }
    pub fn watch(&self, name: &RefPath, on: impl FnMut(Live) + Send + 'static) -> Watch;    // Live: a Frame, or Switched
}
```

A `Frame` is a step's point, its `ExchangeFrame`, the problem's objectives at
that point and whether its goal holds, so one frame is a whole learning
transition, what `RlTransition` carries. `watch` pushes frames; `frames(after)`
pulls them from the last point a consumer handled, so delivery resumes after a
disconnect and works over a stateless transport. `Switched` reports a ref
moved to a point that does not descend from the one before.

Moving out of MCP: economies as points and studies recorded as runs
(`store.rs`), authoring (`authoring.rs`), studies as economies
(`studies.rs`), the page reads of `read.rs`, and the apply and search work of
`server.rs`. MCP keeps its tools, resources, schemas, messages, issues,
completion and HTTP, and its table from handles to points.

### Step 6: the other documents' words

`docs/CONTINUOUS_AGENT_SYSTEMS.md`'s integration contract: `fork(snapshot)`
and `apply(fork_or_live, exchange)` become `apply(point, exchanges)`, with
`advance(ref, expected, point)` and `follow(ref, after)`. PDD's Studio
section and the crate READMEs name the engine layer.

### Step 7: `axionomy-engine`

The engine takes over MCP's work on economies and runs, and MCP becomes a
transport over it. The Studio server and the CLI hold an `Engine`, so the
Studio server has a store: in memory, or a file when one is given.

Done when MCP's Rust tests and its Python interop suite pass unchanged, and
MCP touches the store only for its handle table.

### Step 8: the engine runs in the browser

redb is pure Rust and `axionomy-store` already compiles for
`wasm32-unknown-unknown` with `getrandom`'s `wasm_js` feature. What does not
run there is the store's own writer, a thread with a timer
(`crates/axionomy-store/src/writer.rs`):

- Without threads, the writer runs each job inline on the caller, the Worker,
  still in batches, and every durable commit flushes.
- `getrandom`'s `wasm_js` feature is enabled for wasm targets only.
- `Store::on(backend)` opens a store on any redb `StorageBackend`.
- `axionomy-studio-wasm` gains an OPFS backend over a
  `FileSystemSyncAccessHandle`, whose synchronous `read` and `write` at an
  offset, `truncate`, `flush` and `getSize` map one to one onto
  `StorageBackend`'s `read`, `write`, `set_len`, `sync_data` and `len`. The
  handle exists only in dedicated Workers, where Studio's engine already runs
  (`studio/src/engine.worker.ts`), which opens the file; IndexedDB is
  asynchronous and localStorage is not available to Workers. The handle is
  exclusive, so a second tab gets a store in memory and says so.

Done when a wasm test records a run in a Worker, reloads and reads it back,
and CI builds `axionomy-engine` for `wasm32-unknown-unknown`.

### Step 9: problems show and offer any point

`Problem` gains, for each ontology it owns, two hooks over paths:

- `view`: a history's frames, labels, scene, objectives and goal, from the
  derivation its studies already use, so a study document and a run of the
  same ontology show the same;
- `actions`: the applicable exchanges at a state, from the candidates its
  policies already search.

An ontology no problem owns is shown with its paths as labels and offers no
candidates; an exchange can still be written and assessed.

Done when every study document's frames equal `Engine::frames` of the run
that recorded it.

### Step 10: live refs

The store's refs gain compare-and-set, and the store indexes each run's
branches; the engine gains `refer`, `advance` and `watch`.

Done when an advance from an old point is refused with the current one,
points made while thinking are never delivered, `frames(after)` resumes where
a consumer stopped, and a ref moved to a branch is reported as `Switched`.

### Step 11: Studio works on points and refs

The native server's routes and the Worker's messages expose the same
`EngineClient` operations, so the browser engine does everything the native
one does; the static engine, which plays saved studies, stays read-only.
Studio selects a point, and its controls target it:

| Control | Effect |
| --- | --- |
| Scrub, play | Reads a history; changes nothing |
| Follow | Shows a ref's steps as they are delivered; scrubbing back does not stop it |
| Act | Applies a candidate or a written exchange, assessed first. At a ref's point it advances the ref; anywhere else it makes a branch |
| Continue from here | Advances a ref to the selected point |
| Compare | Two histories side by side |

Done when Studio's end-to-end tests pass on the native and the browser
engine.

### Step 12: one application, a person, a thinking policy, an outside learner

A Python test (uv, httpx) drives the native server on connect-four, standing
in for the application and the Autonomy Harness:

- The application refers `live` to the empty board of a connect-four study.
- A person, stood in for by the test (step 11's end-to-end test does it
  through Studio), applies a move and advances `live`.
- A policy thinks: from `live`'s point it plays out each candidate with its
  value table, then applies its choice there and advances `live` from the
  point it saw, looking again when refused.
- A learner follows `live` through `frames(after)` and updates the policy's
  value table between its decisions.

Done when every delivered frame names its point, no point made while thinking
is delivered, an advance from an old point is refused, and `live`'s history
verifies.

### Step 13: thinking in Studio (separable)

A problem's own policy proposes from the selected point, connect-four's MCTS
first, and the branch it makes shows its thinking.

### Open question

Which transport the Autonomy Harness uses. Step 12 drives the server over
HTTP. If the harness speaks MCP, refs join MCP in step 10: MCP mints a handle
for a ref as it does for a point, and follows it by pulling `frames(after)`,
since its HTTP transport is stateless.

## Not planned

Each of these waits for a problem or a client that needs it:

- A separate behavior crate, or scorer, picker, thinker and action types.
- Floating-point utility scores.
- Action state machines and behavior checkpoints.
- Typed behavior identifiers, versioned definitions, or definitions as data.
- Durable workflow, retry or compensation machinery.
- MCP operations for behaviors.
- MuJoCo or language-model executors; those belong to the Autonomy Harness.
- MCP exposing refs, unless the Autonomy Harness speaks MCP (the open
  question above).
- A runner or scheduler framework: step 12's loop is the first application
  loop, and one is extracted when a second application shares it.
- Bridges to robots or other executors, which belong to the Autonomy
  Harness: a bridge follows a ref, resumes with `frames(after)` and stops on
  `Switched`.
