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
gains strategies, and the problem tools become package and experiment tools
([plan](../../docs/SIMULATION.md#plan-axionomy-on-primitives), D-045,
D-050).

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
problem + strategy ──── problem_study ───> study_id, and two economy_ids for
                        (task)             each document: where it starts and
                                           where its exchanges reach
economy_id ──────────── ref_create ──────> ref_id, a name for where an
                                           application is
ref_id + economy_ids ── ref_advance ─────> the ref moved forward from one to
                                           the other, or `moved` and where
                                           it is
ref_id + economy_id ─── ref_frames ──────> the steps it was moved over after
                                           that economy
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

The server's instructions walk a first-time client through a study:

1. `axionomy_problem_catalog` lists the problems the server studies, with
   the keys of their instances and strategies and the labels each problem
   gives its family, capabilities, instances and strategies.
2. `axionomy_problem_study` studies one and returns its documents, each with
   `initial_economy_id` and `final_economy_id`.
3. `axionomy_trace_step` with a step on `final_economy_id` names the economy
   at that step of the study.
4. That economy's `rates` resource lists the exchanges it can take, and its
   `accounts` resource the accounts to bind.
5. `axionomy_exchange_assess` says whether an exchange would apply, and why
   not; `axionomy_exchange_apply` applies it.
6. The `diff` resource compares the result with the study's next step.

An exchange names a rate, an account for each of its roles, and how many units
to apply:

```json
{ "rate": "transfer", "bindings": { "giver": "source", "receiver": "sink" }, "units": "2" }
```

A role the rate opens binds the path of the account the exchange creates,
which must not exist yet. `axionomy_problem_study` and `axionomy_search` run
only as MCP tasks; every other tool works without the tasks extension.
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
| `axionomy://study/{study_id}` | A study: its key, selected document, and each document with its frame count and two economies |
| `…/documents/{document}` | One document without its frames, with its two economies |
| `…/documents/{document}/frames{?cursor}` | Its frames, each with its `step`, its `exchange` as the economies take it, and its `view` |

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
`completion/complete` suggests accounts, assets, rates, invariants and
documents for template arguments, by prefix.

A resource that names something missing is refused with `-32602` and a
message saying what: an economy the store does not hold, or an account, rate,
invariant or document the economy or study does not have, with the resource
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
| `axionomy_search` | MCP task: outcome and the economy a solution reaches | additive, idempotent |
| `axionomy_problem_catalog` | Built-in problems, instances and strategies | read-only |
| `axionomy_problem_study` | MCP task: study summary | additive, idempotent |
| `axionomy_ref_create` | A new `ref_id` and the `economy_id` it names | additive |
| `axionomy_ref_read` | The `economy_id` a ref is at | read-only |
| `axionomy_ref_advance` | The `economy_id` a ref moved to, or `moved` with where it is | additive, idempotent |
| `axionomy_ref_frames` | The steps a ref was moved over after an economy, waiting up to 30 s for the next, and where it is | additive, idempotent |

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
  `axionomy_trace_find` and a study frame's `step` count steps.
- A position of an exchange counts from zero: the exchange at index `i` takes
  step `i` to step `i + 1`. Change lines and failures name it
  `exchange_index`; a frame's `view.index` is one too.
- Messages count from one: "exchange 2 of 3 is infeasible" is the exchange at
  index `1`.

`axionomy_trace_step` returns the economy at that step with its `step`, the
economy passed in as `traced_economy_id`, and the trace's source, step `0`, as
`source_economy_id`. `axionomy_trace_find` returns the first step where a
balance condition holds, or `{"step": null, "message": …}` when none does; an
account no economy of the trace has is `not_found`.

## Studies

`axionomy_problem_study` studies one instance of a listed problem with a
seed and a budget. Its documents are the problem's outcomes: the run of the
requested strategy, which is the `selected_document`, beside the others,
usually the runs of its other strategies and cases such as a deadlock. Each
document's frames are resources, and each document is also two ordinary
economies, listed in the study's summary and in the document resource:

- `initial_economy_id`: the economy the document starts from, stored as an
  authored economy.
- `final_economy_id`: the economy its exchanges reach, made by applying them
  to the initial economy, one step per exchange; the initial economy itself
  when the document has none.

Everything a client does with an economy, it does with a study. The final
economy's `exchanges` resource is the document's trace; `axionomy_trace_step`
with step `N` names the economy after its first `N` exchanges, the one frame
`N − 1` shows; `axionomy_trace_find` finds where a balance condition starts.

A frame is the exchange that makes its step and the view Studio draws of it:

```json
{
  "step": 1,
  "exchange": {
    "rate": "move/start/key-room/2/false",
    "bindings": { "actor": "agent", "environment": "world" },
    "units": "1"
  },
  "view": { "index": 0, "exchange": { "rate": { "key": "maze/rate/move/start/key-room/2/false", "label": "…" }, "…": "…" }, "after": "…" }
}
```

Its `exchange` is written in the economy's own paths, exactly as
`axionomy_exchange_apply` takes it: applied to step `step − 1` of the final
economy's trace, it makes step `step`. The `view` keys accounts, assets and
rates for display, under the problem's name; act with `exchange` and the
economies' resources instead. To try something else from step `N`, assess or
apply another exchange to that economy, or replay another trace from it, and
compare the result with the study's own step `N + 1` through the `diff`
resource:

```text
final_economy_id + N ─────── trace_step ──────> the study's economy at step N
that economy + exchange ──── exchange_apply ──> your branch
your branch + study's N + 1  …/diff/{other} ──> what your choice changed
```

Replaying a document's trace on its initial economy is the operation that made
its final economy, so `axionomy_trace_replay` returns the same handle. A
study's documents usually start from equal economies under different handles;
their equal roots say so.

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
  that is an `invalid_request` at `/economy_id`.
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

```text
ref_create(start) ────────────────────────> ref_id
exchange_apply(start, x) ─────────────────> mine
ref_advance(ref_id, start, mine) ─────────> economy_id: mine
ref_advance(ref_id, start, theirs) ───────> moved, current_economy_id: mine
ref_frames(ref_id, start, wait_seconds) ──> [{economy_id: mine, exchange: x, receipt}]
```

Every economy a ref or a frame names is an `economy_id` every other tool
takes: assess, apply, step through, find in, diff. The server names a point
by the first handle that named it, whatever run holds it, so a ref reads back,
and its steps are delivered, as the handles their movers made; a step inside
a replay that no handle names yet gets one minted from its point and stored
like any economy, made by the frame's exchange from the frame before it.
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
since a ref is a name to move rather than a result. Study handles, `StudyId`,
are written `stu-` and keyed by what the study was computed from, so the same problem, instance, strategy, seed and budget always
get the same handle. A study takes the problem, instance and strategy as kebab-case paths
(`work-league`, `micro`, `a-star`), the instance one its problem declares;
its summary carries the study handle, that key, the selected document and, for
every document, its frame count and its two economies.

Every economy is a point of a run in the store. Authoring records a run that
starts from the authored state; applying exchanges records a run forked from
the source economy's point, one step per exchange, storing every 64th state
and the last. A handle, its run, its steps and its states commit in one
transaction, so a handle exists exactly when everything it names does. The
economy at a step that `axionomy_trace_step` names is rebuilt from the nearest
stored state by replay, with every receipt checked against the one recorded.
Assessments are stored as their inputs and computed again when read. A study
is stored as its key and its documents' handles, never as its views: each
document's initial economy is a run of its own, and its exchanges a run forked
from it. Its documents and frames are computed again from the key when they
are not in memory, and served only if they replay the very trajectories the
study recorded; a study recorded by another version of the server is refused
with `-32602` (a resource not found, which rmcp reports as invalid params under
`2026-07-28`) and must be studied again, while its economies remain. Recently
used economies and studies stay in memory.

`AxionomyMcp::new` takes the `Engine` it serves (`axionomy-engine`), which
holds the store it keeps economies in, and the `Registry` of problems its
study tools study until packages replace them;
`axionomy_problems::registry()` holds every sample problem.

```rust
use axionomy_engine::Engine;
use axionomy_mcp::AxionomyMcp;
use axionomy_store::Store;
use axionomy_study::Registry;

let engine = Engine::new(Store::memory()?)?;
let server = AxionomyMcp::new(engine, Registry::new())?;
# let _ = server;
# Ok::<(), Box<dyn std::error::Error>>(())
```

A store in a file (`Store::open`) keeps economies, handles and the secret
across restarts; an in-memory store, the default, keeps them until the server
stops, and a handle it issued is `not_found` after a restart.

## Tasks

`axionomy_search` and `axionomy_problem_study` return a task handle when the
caller declares the `io.modelcontextprotocol/tasks` extension, and a
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

This crate is a library and knows no problem; the `axionomy` application
(`axionomy-cli`) serves the sample problems with it:

```console
AXIONOMY_MCP_BIND=127.0.0.1:8000 \
AXIONOMY_MCP_STORE=economies.redb \
RUST_LOG=axionomy_mcp=info,rmcp=info \
  cargo run -p axionomy-cli -- mcp
```

Without `AXIONOMY_MCP_STORE` (or `--store`) the server keeps economies in
memory.

The Streamable HTTP endpoint is `/mcp` and the health endpoint is `/health`.
The transport accepts only MCP `2026-07-28`, keeps no sessions, prefers JSON
responses, and requires per-request protocol metadata and the `Mcp-Method` and
`Mcp-Name` routing headers.

## Testing

The Rust unit tests hold the economic behavior: authoring conflicts and limits,
paging a 100,000-asset economy, failures and the arguments they name, tasks,
stepping and diffs, a store on disk across restarts, a study's documents as
economies that replay its trajectories, frames whose exchanges apply to the
step before them, studies computed again from their keys, and refs that move
only forward from where their movers expect them, deliver only the steps
they move over, under the same handles every call, and wake a waiting
follower. `interop/`
checks that a standard client reaches all of it: the official MCP Python SDK
drives `axionomy mcp` over Streamable HTTP through discovery, tools,
resource links and pages, completion, stepping and diffs, task-backed search
and cancellation, a problem study, stepping into a study and branching from
it, applying a frame's exchange to its study's economy, structured errors with
argument pointers, the routing-header rule, and economies and studies that
outlive a restart, and refs: two clients advancing one ref from the same
economy, the second told where it is; a step only tried out never delivered;
a follower pulling frames after the last economy it handled, and a long poll
that returns when another client advances. The SDK does not ship the tasks
extension, so the suite declares it, as SEP-2663 describes. Python is managed with uv:

```console
cargo build -p axionomy-cli --bin axionomy
cd crates/axionomy-mcp/interop && uv run --locked pytest
```

## Deployment boundary

This crate is a local reference implementation, not a production control
plane. Economies, assessments, studies and refs persist in its store; tasks
and followers waiting on a ref live in the process. Anyone holding a ref's
handle may move it. It has no authentication, tenant isolation, quotas, retention
limits, persisted search checkpoints or distributed workers. Public
deployments must also configure allowed hosts and origins rather than
loosening `rmcp`'s DNS-rebinding protection.
