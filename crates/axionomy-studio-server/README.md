# axionomy-studio-server

This crate is the native reference backend for Axionomy Studio. It serves the
engine's operations Studio uses over HTTP, as `axionomy-studio-api` defines
them, with an OpenAPI 3.1 contract generated from the same route table
([plan](../../docs/SIMULATION.md#plan-axionomy-on-primitives), P12, D-045):
the packages its store holds, loading one, a point's history, the
candidates at a point, assessing and applying exchanges, strategies and
experiments run while their progress streams, comparison and refs. The
browser Worker (`axionomy-studio-wasm`) calls the same functions and gives
the same JSON, so Studio reaches both through one client.

`StudioState::new` takes the `axionomy-engine` `Engine` whose store the API
serves, beside the `Registry` of the problems it studies until studies
retire. Each handler runs the engine's synchronous work where blocking is
allowed, never on the async runtime.

## Packages, points and experiments

A point is written `PointId`: its run, `:` and its step, such as
`run-000000000000feed0000000000000007:3`. A path such as an experiment's
`maze/micro` or a ref's `live/game` is a single URL segment, its `/` written
`%2F`. A seed or a budget is `Natural`, a 64-bit number written in decimal,
since JSON's numbers cannot carry every one exactly; quantities are exact
decimal text.

| Route | Body or query | Result |
| --- | --- | --- |
| `GET /api/packages` | | `PackageList`: each package's manifest, its refs with their points (those it names and any made under its name since), experiments with their arms, objectives, seed, budget, trials and whether results are recorded, strategies, objectives with their directions, goals and candidate sets |
| `POST /api/packages` | the file's bytes, `application/octet-stream` | `LoadedView {packages, kept}`: the packages it held and the names the store kept as it gives them; 400 for a file that is no package or one this engine cannot load |
| `GET /api/points/{point}/document` | | the point's history as a `ViewDocument`, drawn with its view |
| `GET /api/points/{point}/ancestor` | `step` | `Ancestor {point}`: the point at that step of the history |
| `GET /api/points/{point}/candidates` | `set` | `Candidates`: each exchange the candidate set offers there, with its `ProposalView`, read with the point's view and assessed there |
| `POST /api/points/{point}/assess` | `{exchange}` | `Candidate`: the exchange assessed there; nothing is recorded |
| `POST /api/points/{point}/apply` | `{exchanges}` | `Applied {from, point}`: the new point, all of them applied or none; 400 when one does not apply |
| `POST /api/points/{point}/run` | `{strategy, seed, budget}` | server-sent `StrategyEvent`s: `progress` with the search's counts, then `ran` (how it ended, where, its thinking and cost) or `failed` |
| `GET /api/experiments/{experiment}/results` | | `ResultsView`: every arm's trials with their seeds, ends, how they ended and exact objective values; 404 before it has run |
| `POST /api/experiments/{experiment}/run` | | server-sent `ExperimentEvent`s: `progress`, then `results` or `failed` |
| `POST /api/compare` | `{points, objectives}` | `ComparisonView`: each point's values, the front no point dominates, and the pick, best by the objectives in order |

A strategy or an experiment is checked before it runs, so one the store
lacks is 404 at once; then its run streams, and a client that closes the
stream stops its search at its next look at its progress. An experiment
stopped that way keeps no results, so the results it kept before stay.
What the store lacks is 404, a request that cannot be served as asked 400,
and an engine failure 500, each `ErrorResponse {error}`. The contract also
carries `SamplePackages`, the `packages/index.json` a static site lists its
sample packages by, which no endpoint serves.

## Refs

A ref names the point an application is at, and moving it forward is what
makes a step count (D-046).

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
`after` off the ref's history is 400.

## Studies

Until they retire, it also adapts the `axionomy-study` runtime into one
problem catalog, pausable, resumable and cancellable in-memory studies,
replay-derived documents, paginated frames and Server-Sent Events, over the
problems of its `Registry`; it knows no problem of its own. Each study gets
a minted ID such as `study-7`; once published, the study, its documents and
their frames are served under `/api/studies/{study_id}/study`.

## Serving

The `axionomy` application (`axionomy-cli`) serves the sample problems with
it, the API and the optional built Studio frontend:

```sh
cargo run -p axionomy-cli -- studio
cargo run -p axionomy-cli -- studio --store work.redb --package maze.zip --package sokoban.zip
```

The default address is `127.0.0.1:3000`; override it with `--bind` or
`AXIONOMY_STUDIO_BIND`. The store whose packages, points and refs it serves
is in memory unless `--store` or `AXIONOMY_STUDIO_STORE` names a store file.
Each `--package` (repeatable), or else each file `AXIONOMY_STUDIO_PACKAGES`
lists, separated as `PATH` separates them, is merged into the store before
it serves; a package that fails the checks loading makes stops the server.
If `studio/dist` exists, the server hosts it with an SPA fallback;
`--frontend` or `AXIONOMY_STUDIO_DIR` names another built frontend. The
OpenAPI contract is available at `/api/openapi.json`. On startup the command
prints clickable Studio, health, and OpenAPI URLs using the address and port
the operating system actually bound. It also reports when the frontend has
not been built and prints the command needed to build it.

Studio generates its browser contract, its static catalog and the sample
packages with `pnpm generate`, which runs:

```sh
cargo run -p axionomy-studio-server --bin export-openapi -- studio/openapi.json
cargo run -p axionomy-cli -- export-studies studio/public/studies
cargo run -p axionomy-cli -- package samples studio/public/packages
```

The contract does not depend on which problems are registered or which
packages are loaded.
