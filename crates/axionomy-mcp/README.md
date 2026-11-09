# axionomy-mcp

`axionomy-mcp` is Axionomy's stateless MCP `2026-07-28` reference server. It
lets a remote client build economies of any size, apply exchanges to them, and
read everything they hold, without putting HTTP, Tokio, task lifecycle or
storage policy into the core engine.

## Semantic contract

The server has no current economy. Every operation names an immutable
snapshot, and every change stores a new one:

```text
entries ─────────────── economy_create ──> economy_id
economy_id + entries ── economy_extend ──> new economy_id
economy_id + exchange ─ exchange_assess ─> explanation, nothing stored
economy_id + exchange ─ exchange_apply ──> new economy_id
economy_id + exchanges  trace_replay ────> new economy_id
economy_id + goal ───── search (task) ───> economy_id the solution reaches
```

A source snapshot never changes. A failed operation stores nothing; the
caller keeps the handle it had. Counterfactual branches and rollback are
therefore just older handles.

Nothing a tool accepts or returns grows with the size of an economy. A call
adds at most 10,000 entries or replays at most 1,000 exchanges, and a result
carries handles, counts and at most ten preview lines. Everything else is read
as a resource, one page at a time.

## Authoring

`axionomy_economy_create` and `axionomy_economy_extend` take the same entries,
keyed by ID, with every collection optional:

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
and `conditions`. Extending adds: a new ID adds an account, rate or invariant,
and an existing one gains balances, amounts or weights. An entry that would
replace something already present is a `conflict`, so an economy too large for
one call is built by chaining calls, each of which stores a valid snapshot.
The kernel's `EconomyBuilder` builds every snapshot, so the kernel alone
decides what a valid economy is.

## Resources

Resources hold the data. Their URI templates (RFC 6570) are listed by
`resources/templates/list`:

| Template | Content |
| --- | --- |
| `axionomy://economy/{economy_id}` | Origin, parent and counts |
| `…/accounts{?cursor}` | Accounts and how many assets each holds |
| `…/accounts/{account}/balances{?prefix,cursor}` | Nonzero balances, optionally by asset-ID prefix |
| `…/rates{?asset,cursor}` | Rates and their roles, optionally only those that move `asset` |
| `…/rates/{rate}` | Roles, distinct pairs, parameters and counts |
| `…/rates/{rate}/amounts{?cursor}` | Fixed, then computed, amounts per unit |
| `…/rates/{rate}/conditions{?cursor}` | Conditions every exchange must satisfy |
| `…/invariants{?cursor}` | Conservation laws and how many assets each weights |
| `…/invariants/{invariant}/weights{?cursor}` | Asset weights |
| `…/changes{?cursor}` | Quantities moved by the exchanges that made the snapshot |
| `…/exchanges{?cursor}` | Those exchanges; replaying them on the parent reproduces it |
| `axionomy://assessment/{assessment_id}` and `…/lines{?cursor}` | An assessment and what explains it |
| `axionomy://artifact/{artifact_id}`, `…/documents/{document_id}` and `…/frames{?cursor}` | A problem run |

A list page is `{"items": [...], "nextCursor": "..."}` and follows the MCP
pagination rules: the server chooses the page size, the cursor is opaque, the
next page is the same URI with that cursor, and a cursor from another listing
is refused with `-32602`. The snapshot behind a page never changes, so every
page is cached with `cacheScope: "private"` and a one-day `ttlMs`, and needs no
subscription. `resources/list` is empty: snapshots are reached through their
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
| `axionomy_search` | MCP task: outcome and the snapshot a solution reaches | additive, idempotent |
| `axionomy_problem_catalog` | Built-in problems, instances and strategies | read-only |
| `axionomy_problem_run` | MCP task: run summary | additive, idempotent |

Every tool declares input and output schemas and `openWorldHint: false`.
Results link to the resources that page through what they summarize. Applying
one exchange and replaying it alone are the same operation and return the same
handle. A failure is a result with `isError` and a stable code:
`invalid_request`, `not_found`, `conflict`, `too_large`, `infeasible`,
`invalid`, `tasks_required` or `internal`. An infeasible or invalid exchange
says which exchange failed, previews what accounts lack, and links to the
stored assessment.

## Handles

A snapshot handle is a BLAKE3 hash, keyed with a secret drawn from the
operating system when the store is created, of the parent handle and the
operation. Retrying an operation therefore returns the same handle without
storing the economy again, and nobody without the server can derive a handle
from an economy's content. Two different histories that reach the same
balances get different handles, so a handle also names how its state was
reached. Assessment handles are keyed the same way.

`AxionomyMcp<S>` is generic over the small `SnapshotStore` contract: store a
snapshot under the handle derived from its parent and operation, and resolve a
handle. Implementations must never replace the snapshot behind a live handle.
`MemorySnapshotStore` is the default. Its clones share one map, and a snapshot
made by exchanges shares its unchanged accounts with its parent through `Arc`.
Handles last as long as that store; a restarted process resolves none of them,
and the tools say so.

```rust
use axionomy_mcp::{AxionomyMcp, MemorySnapshotStore};

let server = AxionomyMcp::new(MemorySnapshotStore::default());
# let _ = server;
```

Assessments and problem-run artifacts are derived data kept in process memory
for as long as the server runs.

## Tasks

`axionomy_search` and `axionomy_problem_run` return a task handle when the
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
RUST_LOG=axionomy_mcp=info,rmcp=info \
  cargo run -p axionomy-mcp --bin axionomy-mcp
```

The Streamable HTTP endpoint is `/mcp` and the health endpoint is `/health`.
The transport accepts only MCP `2026-07-28`, keeps no sessions, prefers JSON
responses, and requires per-request protocol metadata and the `Mcp-Method` and
`Mcp-Name` routing headers. `cargo test -p axionomy-mcp --test http` runs the
whole flow over HTTP: tools, resource templates and pages, completion, and a
search task with cancellation.

## Deployment boundary

This crate is a local reference implementation, not a production control
plane. It keeps snapshots, assessments, artifacts and tasks in one process. It
has no authentication, tenant isolation, quotas, retention limits, persisted
search checkpoints or distributed workers. A deployment that needs durable
handles implements `SnapshotStore`; public deployments must also configure
allowed hosts and origins rather than loosening `rmcp`'s DNS-rebinding
protection.
