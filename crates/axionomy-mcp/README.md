# axionomy-mcp

`axionomy-mcp` is Axionomy's strict stateless MCP 2026-07-28 reference
boundary. It demonstrates remote, interruptible use without putting HTTP,
SQLite, Tokio, task lifecycle, or client policy into the core engine.

## Semantic contract

The server has no hidden “current economy.” Every operation identifies an
immutable snapshot explicitly:

```text
economy JSON ── put ──> economy_id
economy_id + exchange ── assess ──> explanation (no new state)
economy_id + exchange ── apply ──> receipt + new economy_id
economy_id + trace ── replay ──> receipts + new economy_id
economy_id + goal + candidates ── search ──> task ── poll ──> replayable trace
```

Snapshot IDs are BLAKE3 hashes of the exact current Serde representation.
Putting identical bytes deduplicates them. A source snapshot is never changed;
failed apply or replay operations create nothing, and a successful operation
stores a new snapshot. This supplies counterfactual branches and rollback by
retaining the prior handle, without inventing a second transaction model.

Operational data may live outside the economy only when it is derived and
disposable. SQLite task rows, progress counters, work budgets, cancellation
intent, candidate lists, and HTTP request metadata cannot change exchange
validity or install a successor. Every searched edge is still a concrete
exchange accepted by `axionomy`, and a solution is still a trace the kernel can
replay.

## Tools

| Tool | Input | Result |
| --- | --- | --- |
| `axionomy_economy_put` | Directly serializable string-ID, `u64` economy | Content-addressed `economy_id` and deduplication flag |
| `axionomy_exchange_assess` | `economy_id`, exchange | Applicable/infeasible/invalid status, full shortfalls or projected deltas |
| `axionomy_exchange_apply` | `economy_id`, exchange | Source ID, new snapshot ID, and receipt |
| `axionomy_trace_replay` | `economy_id`, trace | Source ID, new snapshot ID, and all receipts |
| `axionomy_search` | `economy_id`, goal, concrete candidate exchanges, bounds, optional idempotency key | MCP task handle; poll `tasks/get` for progress and the structured search result |

Tool inputs and outputs carry generated JSON Schemas. The reference wire
ontology uses `String` for asset, account, rate, and role IDs and `u64` for
quantities. This is a deliberately usable reference profile, not a privileged
core ontology or a versioned historical compatibility promise.

The search tool currently runs breadth-first search over the caller's explicit
finite candidate exchange universe. Candidates are reconsidered at every
state and filtered through core application. This makes the remote action
contract serializable and honest: the server does not hide domain-specific
binding enumeration in a callback. Richer declarative proposal schemas should
be added only when concrete candidate pressure justifies them.

## Tasks and control

`axionomy_search` returns `CreateTaskResult` only when the caller advertises
the `io.modelcontextprotocol/tasks` extension. Otherwise it returns a visible
tool error. Before returning a handle, the server durably records the task.

- `tasks/get` returns persisted status, a human-readable progress message, and
  the terminal `CallToolResult` or JSON-RPC error.
- `tasks/cancel` durably records cancellation intent. The worker observes it
  between deterministic BFS chunks and settles the task as cancelled.
- `tasks/update` acknowledges known tasks, but this implementation never asks
  for in-task input.
- Reusing an idempotency key with identical search JSON returns the original
  task. Reusing it with different parameters is rejected.
- If the process restarts during work, startup converts the abandoned task to
  an explicit failed state. Search checkpoints are not yet persisted for
  continuation across process failure.

The core search crate supplies the runtime-neutral `BfsSession` and progress
contract. Tokio spawning and SQLite polling are adapter choices made here.
Monte Carlo, MCTS, and ISMCTS now expose the same bounded session shape and can
be added as task kinds without changing core authority.

## Running

```console
AXIONOMY_MCP_BIND=127.0.0.1:8000 \
AXIONOMY_MCP_DATABASE=axionomy-mcp.sqlite3 \
RUST_LOG=axionomy_mcp=info,rmcp=info \
  cargo run -p axionomy-mcp --bin axionomy-mcp
```

The Streamable HTTP endpoint is `/mcp`; the health endpoint is `/health`.
Defaults are `127.0.0.1:8000` and `./axionomy-mcp.sqlite3`.

The transport:

- advertises and accepts only MCP `2026-07-28`;
- disables legacy MCP sessions and uses `NeverSessionManager`;
- prefers JSON responses and does not create `Mcp-Session-Id` state;
- requires `MCP-Protocol-Version`, per-request protocol/client metadata, and
  the revision's `Mcp-Method`/`Mcp-Name` routing headers.

Run the complete raw HTTP reference flow—including strict-header rejection,
snapshot storage, task creation, polling, and solution decoding—with:

```console
cargo test -p axionomy-mcp --test http
```

## Deployment boundary

This crate is a local reference implementation, not a production control
plane. It currently provides one SQLite store and task workers in one process.
It does not provide authentication, tenant isolation, authorization, quotas,
TTL garbage collection, distributed worker leasing, cross-instance task
notifications, persisted search checkpoints, or schema migration. Public
deployments must also configure explicit allowed hosts/origins rather than
loosening rmcp's DNS-rebinding protections indiscriminately.
