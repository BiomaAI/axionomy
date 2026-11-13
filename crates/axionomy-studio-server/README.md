# axionomy-studio-server

This crate is the native reference backend for Axionomy Studio. It adapts the
same `axionomy-study` runtime used by CLI and MCP into one problem catalog,
pausable/resumable/cancellable in-memory studies, replay-derived documents,
paginated frames, Server-Sent Events, and an OpenAPI 3.1 contract, over the
problems of the `Registry` its `StudioState` is given; it knows no problem of
its own. Each study gets a minted ID such as
`study-7`; once published, the study, its documents and their frames are
served under `/api/studies/{study_id}/study`. A study in progress is
operational UI state, while a published, portable `Study` can be exported to
JSON whenever durable or static playback is wanted.

`StudioState::new` takes the `axionomy-engine` `Engine` whose store's refs
the API serves, beside the `Registry`. Planned: the server records each
study's documents as runs, and serves points: frames, candidate exchanges,
assess and apply at a point
([plan](../../docs/SIMULATION.md#plan-axionomy-on-primitives), D-045, D-046).

## Refs

A ref names the point an application is at, and moving it forward is what
makes a step count (D-046). A point is written `PointId`: its run, `:` and
its step, such as `run-000000000000feed0000000000000007:3`. A ref is named by
its store-wide path; one such as `live/game` is a single URL segment, its `/`
written `%2F`.

| Route | Body or query | Result |
| --- | --- | --- |
| `POST /api/refs` | `{name, point}` | `RefView {name, point}`; 409 `RefMoved` when a ref has the name |
| `GET /api/refs/{name}` | | `RefView`: the point it names |
| `POST /api/refs/{name}/advance` | `{expected, to}` | `RefView` at `to`; 409 `RefMoved {error, name, current}` when it is not at `expected`; 400 when `to`'s history does not pass through `expected` |
| `GET /api/refs/{name}/frames` | `after`, `limit` (1 to 1000, 100 by default), `wait_seconds` (up to 30), `goal`, `objectives` | `RefFrames {name, point, frames}` |

A ref moves only from where its mover expects it, so of two movers from one
point the second gets 409 with where the ref is, and looks again from there.
It moves only forward, to a point whose history passes through `expected`.
`frames` delivers the steps the ref was moved over after `after`, the last
point a follower handled, earliest first: each `RefFrame` is the point the
step led to, its exchange and receipt, the `values` of the `objectives` there
(paths of objectives the store keeps, separated by commas) and, with a
`goal` the store keeps, whether it `holds`. Only steps a ref is moved over are
delivered, never an exchange that was only tried out. While the ref's `point`
is past the last frame, more follow from it; with `wait_seconds`, a request
made while the ref is still at `after` waits for it to move and returns as
soon as it does. A ref, point, goal or objective the store lacks is 404, and
`after` off the ref's history is 400. The engine's waits and replays run
where blocking is allowed, never on the async runtime.

The `axionomy` application (`axionomy-cli`) serves the sample problems with
it, the API and the optional built Studio frontend:

```sh
cargo run -p axionomy-cli -- studio
```

The default address is `127.0.0.1:3000`; override it with `--bind` or `AXIONOMY_STUDIO_BIND`. The store whose points and refs it serves is in memory unless `--store` or `AXIONOMY_STUDIO_STORE` names a store file, such as one a package was loaded into. If `studio/dist` exists, the server hosts it with an SPA fallback; `--frontend` or `AXIONOMY_STUDIO_DIR` names another built frontend. The OpenAPI contract is available at `/api/openapi.json`.
On startup the command prints clickable Studio, health, and OpenAPI URLs using
the address and port the operating system actually bound. It also reports when
the frontend has not been built and prints the command needed to build it.

Studio generates its browser contract and complete static catalog with
`pnpm generate`, which runs:

```sh
cargo run -p axionomy-studio-server --bin export-openapi -- studio/openapi.json
cargo run -p axionomy-cli -- export-studies studio/public/studies
```

The contract does not depend on which problems are registered; the studies
are the sample problems'.
