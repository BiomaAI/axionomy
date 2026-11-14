# axionomy-mcp

`axionomy-mcp` is Axionomy's stateless MCP `2026-07-28` reference server. It
lets a remote client build economies of any size, apply exchanges to them, and
read everything they hold, without putting HTTP, Tokio or task lifecycle into
the core engine. Economies are kept in an `axionomy-store` store, in memory or
on disk.

Its economies are points the engine (`axionomy-engine`) records: authoring,
applying exchanges, stepping to a point of a history and assessing are the
engine's, and this server keeps its protocol and its tables from handles to
points. Finding a balance condition and search move to the engine as it
gains strategies. Packages loaded when the server starts carry economies,
strategies and experiments, and the package and experiment tools run them
([plan](../../docs/SIMULATION.md#plan-axionomy-on-primitives), D-045,
D-048, D-050). The server knows no problem: the samples reach it as
packages like any other.

## Semantic contract

The server has no current economy. Every operation names an economy by its
handle, an economy never changes, and every change stores a new one:

```text
entries ─────────────── economy_create ──> economy_id
economy_id + entries ── economy_extend ──> new economy_id
economy_id + exchange ─ exchange_assess ─> explanation, nothing stored
economy_id + exchange ─ exchange_apply ──> new economy_id
economy_id + exchanges  trace_replay ────> new economy_id
economy_id + step ───── trace_step ──────> economy_id of a step of its trace
economy_id + goal ───── search (task) ───> economy_id the solution reaches
economy_id ──────────── ref_create ──────> ref_id, a name for where an
                                           application is
ref_id + economy_ids ── ref_advance ─────> the ref moved forward from one to
                                           the other, or `moved` and where
                                           it is
ref_id + economy_id ─── ref_frames ──────> the steps it was moved over after
                                           that economy
ref_id + economy_id ─── ref_delete ──────> the ref gone from there, or
                                           `moved` and where it is
(nothing) ───────────── package_list ────> each loaded package, its refs'
                                           economy_ids, experiments, strategies
experiment ──────────── experiment_run ──> its goal, every trial's seed,
                                           outcome, objective values and
                                           economy_id
experiment ──────────── experiment_results  the same, as last recorded
```

The economy an operation reads never changes. A failed operation stores
nothing; the caller keeps the handle it had. Counterfactual branches and
rollback are therefore just older handles. Every result and economy carries its
`root`, a content hash: economies with equal roots have equal contents,
however they were made.

Nothing a tool accepts or returns grows with the size of an economy. A call
adds at most 10,000 entries or replays at most 1,000 exchanges, and a result
carries handles, counts and at most ten preview lines. Everything else is read
as a resource, one page at a time.

## Start here

The server's instructions walk a first-time client through an experiment:

1. `axionomy_package_list` lists the packages the server loaded when it
   started: each one's refs, with the `economy_id` of the point each names,
   and its experiments and strategies.
2. `axionomy_experiment_run` runs an experiment, such as `maze/micro`, and
   names each trial's end by an `economy_id`.
3. `axionomy_trace_step` with a step on that `economy_id` names the economy
   at that step of the trial's run.
4. That economy's `rates` resource lists the exchanges it can take, and its
   `accounts` resource the accounts to bind.
5. `axionomy_exchange_assess` says whether an exchange would apply, and why
   not; `axionomy_exchange_apply` applies it.
6. The `diff` resource compares the result with the run's next step.

See [Packages and experiments](#packages-and-experiments).

An exchange names a rate, an account for each of its roles, and how many units
to apply:

```json
{ "rate": "transfer", "bindings": { "giver": "source", "receiver": "sink" }, "units": "2" }
```

A role the rate opens binds the path of the account the exchange creates,
which must not exist yet. `axionomy_search` runs only as an MCP task;
every other tool works without the tasks extension.
An application that moves through economies, such as a game, keeps a ref on
the economy it is at; see [Refs](#refs).

## Authoring

`axionomy_economy_create` and `axionomy_economy_extend` take the same entries,
keyed by ID, with every collection optional. Accounts, assets, rates, roles and
invariants are identified by paths: lowercase segments of `a`–`z`, `0`–`9` and
`-`, separated by `/`, such as `loans/42` or `agents/7/wallet`. Each kind is
its own Rust type (`AccountPath`, `AssetPath`, `RatePath`, `RolePath`,
`InvariantPath`), and a malformed path is refused before anything runs. The
hierarchy is for people and tools, such as prefix queries; it never decides
whether an exchange is valid.

```json
{
  "accounts": { "source": { "token": "5" }, "sink": {} },
  "rates": {
    "transfer": {
      "consume": { "giver": { "token": "1" } },
      "produce": { "receiver": { "token": "1" } },
      "distinct": [["giver", "receiver"]]
    }
  },
  "invariants": { "supply": { "token": 1 } }
}
```

Quantities are decimal strings, exact up to `2²⁵⁶ − 1`. A rate may also list
`preserve` amounts, `computed` amounts evaluated from balances and parameters,
`conditions`, and the roles whose account it `opens` or `closes`. An exchange
names each account it opens, which must not exist, and may close an account
only once its own effects leave it empty; results list the accounts opened and
closed, and a closed path may be opened again. Extending adds: a new ID adds an
account, rate or invariant, and an existing one gains balances, amounts or
weights. An entry that would replace something already present is a
`conflict`, so an economy too large for one call is built by chaining calls,
each of which stores a valid economy. The kernel's `EconomyBuilder` builds
every economy, so the kernel alone decides what a valid economy is.

## Resources

Resources hold the data. Their URI templates (RFC 6570) are listed by
`resources/templates/list`. A path goes last in its URI and appears as is
(`{+account}` expands without escaping), so an account's balances are
`axionomy://economy/{economy_id}/accounts/agents/7/wallet`:

| Template | Content |
| --- | --- |
| `axionomy://economy/{economy_id}` | Origin, parent, root and counts |
| `…/accounts{?prefix,cursor}` | Accounts in path order and how many assets each holds, optionally by path prefix such as `agents/7/` |
| `…/accounts/{+account}{?prefix,cursor}` | An account's nonzero balances in asset path order, optionally by asset-path prefix |
| `…/rates{?asset,cursor}` | Rates and their roles, optionally only those that move `asset` |
| `…/rates/{+rate}` | Roles, distinct pairs, parameters and counts |
| `…/rate-amounts/{+rate}{?cursor}` | Fixed, then computed, amounts per unit |
| `…/rate-conditions/{+rate}{?cursor}` | Conditions every exchange must satisfy |
| `…/invariants{?cursor}` | Conservation laws and how many assets each weights |
| `…/invariants/{+invariant}{?cursor}` | Asset weights |
| `…/changes{?cursor}` | Quantities moved by the exchanges that made the economy, by `exchange_index` |
| `…/exchanges{?cursor}` | Those exchanges; replaying them on the parent reproduces it |
| `…/diff/{other_id}{?cursor}` | What differs from this economy to another: accounts opened and closed, balances with both quantities, rates and invariants |
| `axionomy://assessment/{assessment_id}` and `…/lines{?cursor}` | An assessment and what explains it |

A list page is `{"items": [...], "nextCursor": "..."}` and follows the MCP
pagination rules: the server chooses the page size, the cursor is opaque, the
next page is the same URI with that cursor, and a cursor from another listing
is refused with `-32602`. The economy behind a page never changes, so every
page is cached with `cacheScope: "private"` and a one-day `ttlMs`, and needs no
subscription. Accounts and balances are read from the economy's state tree, so
a page and a prefix filter cost the logarithm of the economy's size, and a
diff skips everything the two economies share. `resources/list` is empty:
economies are reached through their handles and the `resource_link`s tool
results carry, never through a listing shared by every caller.
`completion/complete` suggests accounts, assets, rates and invariants for
template arguments, by prefix.

A resource that names something missing is refused with `-32602` and a
message saying what: an economy the store does not hold, or an account, rate,
or invariant the economy does not have, with the resource
that lists what it has. A malformed path or handle in a URI is refused with
the reason it is malformed.

## Tools

| Tool | Result | Hints |
| --- | --- | --- |
| `axionomy_economy_create` | `economy_id`, counts | additive, idempotent |
| `axionomy_economy_extend` | new `economy_id`, counts | additive, idempotent |
| `axionomy_exchange_assess` | status, issues, line count and preview | read-only |
| `axionomy_exchange_apply` | new `economy_id`, change count | additive, idempotent |
| `axionomy_trace_replay` | new `economy_id`, change count | additive, idempotent |
| `axionomy_trace_step` | `economy_id` of the economy at one step of the trace that made another | additive, idempotent |
| `axionomy_trace_find` | The first step of that trace where a balance is equal to, at least or at most a quantity, and its economy; `null` if none | additive, idempotent |
| `axionomy_trace_diff` | The economy two traces share last, and step by step after it each side's exchange and the balances they then differ in, where they are equal again, and objectives at both | additive, idempotent |
| `axionomy_search` | MCP task: outcome and the economy a solution reaches | additive, idempotent |
| `axionomy_package_list` | Every loaded package's manifest, its refs with the `economy_id` of each point, its experiments and strategies | additive, idempotent |
| `axionomy_experiment_run` | The goal every arm works toward, every arm's strategy and start, and every trial's seed, outcome, objective values and `economy_id` | additive |
| `axionomy_experiment_results` | The same, as the experiment last recorded them | additive, idempotent |
| `axionomy_ref_create` | A new `ref_id` and the `economy_id` it names | additive |
| `axionomy_ref_read` | The `economy_id` a ref is at | read-only |
| `axionomy_ref_advance` | The `economy_id` a ref moved to, or `moved` with where it is | additive, idempotent |
| `axionomy_ref_frames` | The steps a ref was moved over after an economy, waiting up to 30 s for the next, and where it is | additive, idempotent |
| `axionomy_ref_delete` | The `economy_id` a deleted ref was at, or `moved` with where it is | destructive, idempotent |

Every tool declares input and output schemas and `openWorldHint: false`.
Results link to the resources that page through what they summarize. Applying
one exchange and replaying it alone are the same operation and return the same
handle.

A failure is a result with `isError` and structured content
`{error, message, details}`, whose `error` is a stable code: `invalid_request`,
`not_found`, `conflict`, `moved`, `too_large`, `infeasible`, `invalid`,
`tasks_required` or `internal`. Arguments are read before anything runs, so a
malformed path, quantity or handle, or a missing field, is an
`invalid_request` whose `details.pointer` is the JSON pointer (RFC 6901) of the
argument at fault, such as `/exchange/units` or
`/entries/accounts/agents~17~1Wallet`. An infeasible or invalid exchange says
which exchange failed (`details.exchange_index`), names what each account
lacks or lists its `issues`, each with a snake_case `kind`, the paths
involved and a `message` that points to the resource listing what it could
name instead, and links to the stored assessment, whose `status` is
`applicable`, `infeasible` or `invalid`. A handle the store does not hold is
`not_found`, with a message saying how to make it again; a server whose store
is in memory forgets every handle when it restarts.

### Steps and exchange indices

One convention holds everywhere:

- Step `N` of a trace is the economy after its first `N` exchanges; step `0`
  is the economy they were applied to. `axionomy_trace_step`,
  and `axionomy_trace_find` count steps; `axionomy_trace_diff` counts them
  from the economy two traces share.
- A position of an exchange counts from zero: the exchange at index `i` takes
  step `i` to step `i + 1`. Change lines and failures name it
  `exchange_index`.
- Messages count from one: "exchange 2 of 3 is infeasible" is the exchange at
  index `1`.

`axionomy_trace_step` returns the economy at that step with its `step`, the
economy passed in as `traced_economy_id`, and the trace's source, step `0`, as
`source_economy_id`. `axionomy_trace_find` returns the first step where a
balance condition holds, or `{"step": null, "message": …}` when none does; an
account no economy of the trace has is `not_found`.

`axionomy_trace_diff` compares the traces that made `left_economy_id` and
`right_economy_id`. They meet when one was made by exchanges from the other,
or both from a third, as two trials of an experiment are from their start
and a branch is from the step it was made at; economies authored apart, by
`axionomy_economy_create` or `axionomy_economy_extend`, start traces of their
own and are refused, `invalid_request` at `/right_economy_id`. It returns
`shared_economy_id`, the last economy on both traces, how many exchanges each
side made after it, and for each step after it, counted from it, the exchange
each side made (absent once its trace has ended, when it stays the economy it
ended at) and every balance `{account, asset, left, right}` the two sides then
differ in, exactly, with any account only one side has. `converged_step` is
the first step after which they are equal again, listed or not. At most
`limit` steps, up to 100, come per call, after `after_step`;
`next_after_step` asks for the rest. `objectives`, paths a loaded package
carries such as an experiment's, are measured at both economies; one no
package carries is `not_found` at its pointer.

## Packages and experiments

A package is a store file, or a zip holding one, with a manifest: its name,
version, store format and script host API, the refs it names and the
definitions it carries (D-048). The server loads packages only when it
starts (`axionomy mcp --package maze.zip`, below), installing each in its
store after the engine's checks: merged, or in place of the version of it
a store on disk holds; no tool reads a file or fetches a URL.

`axionomy_package_list` lists every loaded package: its manifest, each of
its refs that names a point with the `economy_id` of that point (such as
`maze/micro`, where an instance starts), and the experiments and strategies
it carries, by path. A ref here is the package's name for a point, not a
`ref_id`, and the package's to keep: it never moves, so the package means
what it shipped. To play or follow an application from that economy, make a
ref of your own there with `axionomy_ref_create`.

`axionomy_experiment_run` takes an experiment's path, such as `maze/micro`,
and runs it as the engine does: every arm's strategy from the arm's start
toward the goal the experiment names, which every arm shares, once per
trial, trial `i` seeded from the experiment's seed and `i` so every arm
meets the same chances, each trial measured on the experiment's
objectives. It records the runs and the results, and returns them:

```json
{
  "experiment": "maze/micro",
  "goal": "maze/solved",
  "objectives": [
    { "objective": "maze/energy", "direction": "minimize" },
    { "objective": "maze/time", "direction": "minimize" }
  ],
  "arms": [
    {
      "strategy": "maze/a-star",
      "start": "maze/micro",
      "start_economy_id": "eco-…",
      "trials": [
        {
          "seed": 17,
          "ended": { "kind": "reached" },
          "values": { "maze/energy": "6", "maze/time": "6" },
          "economy_id": "eco-…",
          "exchanges": 6
        }
      ]
    }
  ]
}
```

`ended` is `reached`, `stopped`, `horizon`, `refused` (with the `turn`),
`exhausted`, `spent` or `interrupted`; objective values are exact
quantities, written as every quantity is. A trial's `economy_id` is the
economy it reached, made by its `exchanges` exchanges from the arm's start:
its `exchanges` resource is the trial's trace, `axionomy_trace_step` names
every step from the start (step zero) to it, and every other tool takes it,
so a client assesses, applies, diffs and branches from any step of any
trial. To try something else from step `N`, assess or apply another exchange
to that step's economy, or replay another trace from it, and compare the
result with the trial's own step `N + 1` through the `diff` resource:

```text
economy_id + N ──────────── trace_step ──────> the trial's economy at step N
that economy + exchange ─── exchange_apply ──> your branch
your branch + trial's N + 1  …/diff/{other} ──> what your choice changed
your branch + trial ──────── trace_diff ─────> where they part, and every step since
```

 `axionomy_experiment_results` gives
the results the experiment last recorded, by a run or in its package, in
the same shape, and is `not_found` until there are some. The engine runs the
experiment on a thread where blocking is allowed. One call runs or reports
at most 1,000 trials over all the arms; a larger experiment is `too_large`,
and an experiment no loaded package carries is `not_found`, both at
`/experiment`. A run records runs of its own, so running an experiment again
gives the same trials and values under new handles, whose equal roots say
their economies are equal.

## Refs

An economy never changes, so trying out an exchange, a counterfactual, a
branch and the step that counts are one operation: applying it. A ref says
which steps count. It names the economy an application, such as a game or a
simulation, is at, and moving it forward is what makes a step count
(D-046). The engine does not know who proposes: a person, an agent, a policy
or a loop applying responses are alike.

- `axionomy_ref_create` makes a new ref naming an economy and returns its
  `ref_id`; every call makes a ref of its own.
- `axionomy_ref_read` names the economy the ref is at.
- `axionomy_ref_advance` moves it from `expected_economy_id`, where the mover
  saw it, forward to `economy_id`, an economy made by exchanges from there,
  such as `axionomy_exchange_apply`'s result. A ref moves only from where its
  mover expects it, so of two movers from one economy the second fails with
  `moved` and `details.current_economy_id`, where the ref is, and looks again
  from there. It never moves back, or to an economy not made from where it is:
  that is an `invalid_request` at `/economy_id`. So a ref at `economy_id` was
  moved there over `expected_economy_id`, and asking again for a move already
  made succeeds: a mover that never heard whether its move counted asks
  again, and fails with `moved` only once the ref is somewhere else.
- `axionomy_ref_frames` gives a follower the steps the ref was moved over
  after `after_economy_id`, the last economy it handled, earliest first: each
  frame is the step's `exchange`, as `axionomy_exchange_apply` takes it, its
  `receipt` (the quantities it moved, as `changes` lines, and the accounts it
  opened and closed), and the `economy_id` it made, with `holds` when the call
  names a `goal` as `axionomy_search` takes it. Only steps a ref is moved
  over are delivered, never an exchange that was only tried out. A call
  returns at most `limit` frames, up to 100, and the ref's `economy_id`: while
  it is past the last frame, follow on from that frame. With `wait_seconds`,
  up to 30, a call made while the ref is still at `after_economy_id` waits for
  it to move, on a thread where blocking is allowed, and returns as soon as it
  does.
- `axionomy_ref_delete` deletes the ref from `expected_economy_id`, where the
  deleter saw it, failing with `moved` and where it is as an advance does,
  and returns the economy it was at. Only the ref goes: the economies it
  named stay, and a ref read, advanced or followed after is `not_found`.

```text
ref_create(start) ────────────────────────> ref_id
exchange_apply(start, x) ─────────────────> mine
ref_advance(ref_id, start, mine) ─────────> economy_id: mine
ref_advance(ref_id, start, theirs) ───────> moved, current_economy_id: mine
ref_frames(ref_id, start, wait_seconds) ──> [{economy_id: mine, exchange: x, receipt}]
```

Every economy a ref or a frame names is an `economy_id` every other tool
takes: assess, apply, step through, find in, diff. The engine names a point
once, in the run whose step reached it, and the server names a point by the
first handle that named it, so a ref reads back, and its steps are
delivered, as the handles their movers made; a step inside a replay that no
handle names yet gets one minted from its point and stored like any economy,
made by the frame's exchange from the frame before it.
Refs are store-wide names, so the server exposes only refs it made: each is
an engine ref named `mcp/` and its handle, and a store on disk keeps it
across restarts.

## Handles and storage

An economy handle, `EconomyId`, is written `eco-` and 64 hexadecimal digits: a
BLAKE3 hash of the parent handle and the operation, keyed with a secret the
store keeps. A handle is not a path: the server mints it, it means nothing to
read, and anything else is refused. Retrying an operation therefore returns
the same handle without storing the economy again, and nobody without the
server can derive a handle from an economy's content. Two different histories
that reach the same balances get different handles, so a handle also names how
its economy was reached; their equal `root`s say they reached equal contents.
Assessment handles, `AssessmentId`, are written `asm-` and keyed the same way.
Ref handles, `RefId`, are written `ref-` and keyed from fresh randomness,
since a ref is a name to move rather than a result.

Every economy is a point of a run in the store. Authoring records a run that
starts from the authored state; applying exchanges records a run forked from
the source economy's point, one step per exchange, storing every 64th state
and the last. A handle, its run, its steps and its states commit in one
transaction, so a handle exists exactly when everything it names does. The
economy at a step that `axionomy_trace_step` names is rebuilt from the nearest
stored state by replay, with every receipt checked against the one recorded.
Assessments are stored as their inputs and computed again when read.
Recently used economies and assessments stay in memory.

A point the server did not make, such as one a package's ref names or one an
experiment reached, is named as a ref's steps are: by the first handle that
named it, or by a handle minted from the point and recorded with it. A
minted point is the economy its run's exchanges made from the point the run
starts from, itself named the same way, or an authored economy when its run
starts from an economy of its own; so an experiment's trial ends as an
economy whose parent is its arm's start, and listing a package or reading
results again names every point as before.

`AxionomyMcp::new` takes the `Engine` it serves (`axionomy-engine`), which
holds the store it keeps economies in, with any packages already loaded into
it.

```rust
use axionomy_engine::Engine;
use axionomy_mcp::AxionomyMcp;
use axionomy_store::Store;

let engine = Engine::new(Store::memory()?)?;
let server = AxionomyMcp::new(engine)?;
# let _ = server;
# Ok::<(), Box<dyn std::error::Error>>(())
```

A store in a file (`Store::open`) keeps economies, handles and the secret
across restarts; an in-memory store, the default, keeps them until the server
stops, and a handle it issued is `not_found` after a restart.

## Tasks

`axionomy_search` returns a task handle when the caller declares the `io.modelcontextprotocol/tasks` extension, and a
`tasks_required` error otherwise; a client without the extension can still
build, step through and branch economies with every other tool.
`rmcp::TaskManager` registers the task before returning its handle.
`tasks/get` reports progress and the terminal result, and `tasks/cancel`
records cancellation intent, which the search observes between deterministic
chunks. The default task TTL is `rmcp`'s five minutes with a 100 ms poll
interval; `AxionomyMcp::with_task_options` replaces that policy. Tasks do not
survive a restart.

The search is breadth-first over the caller's explicit candidate exchanges,
reconsidered at every economy it reaches and filtered through core
application, so a solution is a trace the kernel replays.

## Running

This crate is a library and knows no package and reads no file; the
`axionomy` application (`axionomy-cli`) serves with it the packages it is
given:

```console
cargo run -p axionomy-cli -- package samples packages
AXIONOMY_MCP_BIND=127.0.0.1:8000 \
AXIONOMY_MCP_STORE=economies.redb \
RUST_LOG=axionomy_mcp=info,rmcp=info \
  cargo run -p axionomy-cli -- mcp --package packages/maze.zip --package packages/sokoban.zip
```

Without `AXIONOMY_MCP_STORE` (or `--store`) the server keeps economies in
memory. Each `--package` names a package file, a redb file or a zip holding
one, which the server installs in its store before it serves (another
version of a package the store holds replaces it); without
`--package`, `AXIONOMY_MCP_PACKAGES` lists the files, separated as `PATH`
separates them. A package that fails the engine's checks (a file that is no
package, no manifest, scripts needing a newer host API) stops the server
before it serves, naming the file and why; a name the store already gives
something else keeps what it is, with a warning. A store on disk keeps the
packages loaded into it, which `axionomy_package_list` lists after a restart
too.

The Streamable HTTP endpoint is `/mcp` and the health endpoint is `/health`.
The transport accepts only MCP `2026-07-28`, keeps no sessions, prefers JSON
responses, and requires per-request protocol metadata and the `Mcp-Method` and
`Mcp-Name` routing headers.

## Testing

The Rust unit tests hold the economic behavior: authoring conflicts and limits,
paging a 100,000-asset economy, failures and the arguments they name, tasks,
stepping and diffs, a store on disk across restarts, and refs that move
only forward from where their movers expect them, deliver only the steps
they move over, under the same handles every call, and wake a waiting
follower; and the maze's package loaded into a server: its refs' economies,
an experiment whose every trial matches the engine's run of it (seeds,
outcomes, objective values and traces) and ends at an economy whose steps
apply from the arm's start, results that read back under the same handles,
and experiments no package carries or too large for a call. `interop/`
checks that a standard client reaches all of it: the official MCP Python SDK
drives `axionomy mcp` over Streamable HTTP through discovery, tools,
resource links and pages, completion, stepping and diffs, task-backed search
and cancellation, structured errors with argument pointers, the
routing-header rule, economies and an experiment's results that outlive a
restart, and refs: two clients advancing one ref from the same
economy, the second told where it is; a step only tried out never delivered;
a follower pulling frames after the last economy it handled, and a long poll
that returns when another client advances. It also starts servers with the
maze's and the logistics packages, which the session builds with `axionomy
package samples` and never commits: listing one, running `maze/micro` to the
energy and time the engine gives each strategy, stepping through a trial's
trace from its start, stepping into one logistics trial and branching from it
with another's exchange, and a package that fails its checks stopping the
server. The SDK does
not ship the tasks extension, so the suite declares it, as SEP-2663
describes. Python is managed with uv:

```console
cargo build -p axionomy-cli --bin axionomy
cd crates/axionomy-mcp/interop && uv run --locked pytest
```

## Deployment boundary

This crate is a local reference implementation, not a production control
plane. Economies, assessments, refs, loaded packages and experiment
results persist in its store; tasks and followers waiting on a ref live in
the process. Anyone holding a ref's handle may move it, and anyone may run a
loaded package's experiments. It has no authentication, tenant isolation,
quotas, retention limits, persisted search checkpoints or distributed
workers. Public
deployments must also configure allowed hosts and origins rather than
loosening `rmcp`'s DNS-rebinding protection.
