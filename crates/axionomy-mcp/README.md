# axionomy-mcp

`axionomy-mcp` is Axionomy's stateless MCP `2026-07-28` reference server. It
lets a remote client build economies of any size, apply exchanges to them, and
read everything they hold, without putting HTTP, Tokio or task lifecycle into
the core engine. Snapshots are kept in an `axionomy-store` store, in memory or
on disk.

## Semantic contract

The server has no current economy. Every operation names an immutable
snapshot, and every change stores a new one:

```text
entries ─────────────── economy_create ──> economy_id
economy_id + entries ── economy_extend ──> new economy_id
economy_id + exchange ─ exchange_assess ─> explanation, nothing stored
economy_id + exchange ─ exchange_apply ──> new economy_id
economy_id + exchanges  trace_replay ────> new economy_id
economy_id + step ───── trace_step ──────> economy_id of an intermediate state
economy_id + goal ───── search (task) ───> economy_id the solution reaches
```

A source snapshot never changes. A failed operation stores nothing; the
caller keeps the handle it had. Counterfactual branches and rollback are
therefore just older handles. Every result and snapshot carries its state's
`root`, a content hash: equal roots mean equal states, however they were made.

Nothing a tool accepts or returns grows with the size of an economy. A call
adds at most 10,000 entries or replays at most 1,000 exchanges, and a result
carries handles, counts and at most ten preview lines. Everything else is read
as a resource, one page at a time.

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
closed, and a closed path may be opened again. Extending adds: a new ID adds an account, rate or invariant,
and an existing one gains balances, amounts or weights. An entry that would
replace something already present is a `conflict`, so an economy too large for
one call is built by chaining calls, each of which stores a valid snapshot.
The kernel's `EconomyBuilder` builds every snapshot, so the kernel alone
decides what a valid economy is.

## Resources

Resources hold the data. Their URI templates (RFC 6570) are listed by
`resources/templates/list`. A path goes last in its URI and appears as is
(`{+account}` expands without escaping), so an account's balances are
`axionomy://economy/{economy_id}/accounts/agents/7/wallet`:

| Template | Content |
| --- | --- |
| `axionomy://economy/{economy_id}` | Origin, parent, state root and counts |
| `…/accounts{?prefix,cursor}` | Accounts in path order and how many assets each holds, optionally by path prefix such as `agents/7/` |
| `…/accounts/{+account}{?prefix,cursor}` | An account's nonzero balances in asset path order, optionally by asset-path prefix |
| `…/rates{?asset,cursor}` | Rates and their roles, optionally only those that move `asset` |
| `…/rates/{+rate}` | Roles, distinct pairs, parameters and counts |
| `…/rate-amounts/{+rate}{?cursor}` | Fixed, then computed, amounts per unit |
| `…/rate-conditions/{+rate}{?cursor}` | Conditions every exchange must satisfy |
| `…/invariants{?cursor}` | Conservation laws and how many assets each weights |
| `…/invariants/{+invariant}{?cursor}` | Asset weights |
| `…/changes{?cursor}` | Quantities moved by the exchanges that made the snapshot |
| `…/exchanges{?cursor}` | Those exchanges; replaying them on the parent reproduces it |
| `…/diff/{other_id}{?cursor}` | What differs from this snapshot to another: accounts opened and closed, balances with both quantities, rates and invariants |
| `axionomy://assessment/{assessment_id}` and `…/lines{?cursor}` | An assessment and what explains it |
| `axionomy://study/{study_id}`, `…/documents/{document}` and `…/frames{?cursor}` | A study of a problem: its key and documents, one document, or its frames |

A list page is `{"items": [...], "nextCursor": "..."}` and follows the MCP
pagination rules: the server chooses the page size, the cursor is opaque, the
next page is the same URI with that cursor, and a cursor from another listing
is refused with `-32602`. The snapshot behind a page never changes, so every
page is cached with `cacheScope: "private"` and a one-day `ttlMs`, and needs no
subscription. Accounts and balances are read from the snapshot's state tree,
so a page and a prefix filter cost the logarithm of the economy's size, and a
diff skips everything the two states share. `resources/list` is empty: snapshots are reached through their
handles and the `resource_link`s tool results carry, never through a listing
shared by every caller. `completion/complete` suggests accounts, assets,
rates, invariants and documents for template arguments, by prefix.

## Tools

| Tool | Result | Hints |
| --- | --- | --- |
| `axionomy_economy_create` | `economy_id`, counts | additive, idempotent |
| `axionomy_economy_extend` | new `economy_id`, counts | additive, idempotent |
| `axionomy_exchange_assess` | status, issues, line count and preview | read-only |
| `axionomy_exchange_apply` | new `economy_id`, change count | additive, idempotent |
| `axionomy_trace_replay` | new `economy_id`, change count | additive, idempotent |
| `axionomy_trace_step` | `economy_id` of the state after some of the exchanges that made a snapshot | additive, idempotent |
| `axionomy_trace_find` | The first step of those exchanges after which a balance is equal to, at least or at most a quantity, and that state | additive, idempotent |
| `axionomy_search` | MCP task: outcome and the snapshot a solution reaches | additive, idempotent |
| `axionomy_problem_catalog` | Built-in problems, instances and strategies | read-only |
| `axionomy_problem_study` | MCP task: study summary | additive, idempotent |

Every tool declares input and output schemas and `openWorldHint: false`.
Results link to the resources that page through what they summarize. Applying
one exchange and replaying it alone are the same operation and return the same
handle. A failure is a result with `isError` and a stable code:
`invalid_request`, `not_found`, `conflict`, `too_large`, `infeasible`,
`invalid`, `tasks_required` or `internal`. An infeasible or invalid exchange
says which exchange failed, previews what accounts lack, and links to the
stored assessment.

## Handles and storage

A snapshot handle, `EconomyId`, is written `eco-` and 64 hexadecimal digits: a
BLAKE3 hash of the parent handle and the operation, keyed with a secret the
store keeps. A handle is not a path: the server mints it, it means nothing to
read, and anything else is refused. Retrying an operation therefore returns
the same handle without storing the economy again, and nobody without the
server can derive a handle from an economy's content. Two different histories
that reach the same balances get different handles, so a handle also names how
its state was reached; their equal `root`s say they reached the same state.
Assessment handles, `AssessmentId`, are written `asm-` and keyed the same way.
Study handles, `StudyId`, are written `stu-` and keyed by what the study was
computed from, so the same problem, instance, strategy, seed and budget always
get the same handle. A study takes the problem and strategy as kebab-case paths
(`work-league`, `a-star`) and the instance as `micro`, `showcase` or `stress`;
its summary carries the study handle, that key, the selected document and
every document's frame count.

Every snapshot is a point of a run in the store. Authoring records a run that
starts from the authored state; applying exchanges records a run forked from
the source snapshot's point, one step per exchange, storing every 64th state
and the last. A handle, its run, its steps and its states commit in one
transaction, so a handle exists exactly when everything it names does. An
intermediate state that `axionomy_trace_step` names is rebuilt from the
nearest stored state by replay, with every receipt checked against the one
recorded. Assessments are stored as their inputs and computed again when read;
studies are stored whole. Recently used snapshots stay in memory.

```rust
use axionomy_mcp::AxionomyMcp;
use axionomy_store::Store;

let server = AxionomyMcp::new(Store::memory()?)?;
# let _ = server;
# Ok::<(), Box<dyn std::error::Error>>(())
```

A store in a file (`Store::open`) keeps snapshots, handles and the secret
across restarts; an in-memory store, the default, keeps them until the server
stops.

## Tasks

`axionomy_search` and `axionomy_problem_study` return a task handle when the
caller declares the `io.modelcontextprotocol/tasks` extension, and a
`tasks_required` error otherwise. `rmcp::TaskManager` registers the task
before returning its handle. `tasks/get` reports progress and the terminal
result, and `tasks/cancel` records cancellation intent, which the search
observes between deterministic chunks. The default task TTL is `rmcp`'s five
minutes with a 100 ms poll interval; `AxionomyMcp::with_task_options` replaces
that policy. Tasks do not survive a restart.

The search is breadth-first over the caller's explicit candidate exchanges,
reconsidered at every state and filtered through core application, so a
solution is a trace the kernel replays.

## Running

```console
AXIONOMY_MCP_BIND=127.0.0.1:8000 \
AXIONOMY_MCP_STORE=snapshots.redb \
RUST_LOG=axionomy_mcp=info,rmcp=info \
  cargo run -p axionomy-mcp --bin axionomy-mcp
```

Without `AXIONOMY_MCP_STORE` the server keeps snapshots in memory.

The Streamable HTTP endpoint is `/mcp` and the health endpoint is `/health`.
The transport accepts only MCP `2026-07-28`, keeps no sessions, prefers JSON
responses, and requires per-request protocol metadata and the `Mcp-Method` and
`Mcp-Name` routing headers.

## Testing

The Rust unit tests hold the economic behavior: authoring conflicts and limits,
paging a 100,000-asset economy, failures, tasks, stepping and diffs, and a
store on disk across restarts. `interop/` checks that a standard client reaches
all of it: the official MCP Python SDK drives the server binary over
Streamable HTTP through discovery, tools, resource links and pages,
completion, stepping and diffs, task-backed search and cancellation, a
problem study, error codes, the routing-header rule, and snapshots that outlive
a restart. The SDK does not ship the tasks extension,
so the suite declares it, as SEP-2663 describes. Python is managed with uv:

```console
cargo build -p axionomy-mcp --bin axionomy-mcp
cd crates/axionomy-mcp/interop && uv run --locked pytest
```

## Deployment boundary

This crate is a local reference implementation, not a production control
plane. Snapshots, assessments and studies persist in its store; tasks live in
the process. It has no authentication, tenant isolation, quotas, retention
limits, persisted search checkpoints or distributed workers. Public
deployments must also configure allowed hosts and origins rather than
loosening `rmcp`'s DNS-rebinding protection.
