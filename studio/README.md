# Axionomy Studio

Axionomy Studio is the browser trace player and economic debugger for Axionomy.
All twelve canonical problems are discoverable and runnable. Universal panels
inspect balances, exchange bindings, assessments, projected and committed
deltas, encoded rates and invariants, objectives, alternatives, telemetry, and
actor-relative observations. Graph, grid, matrix, and timeline scenes are
derived conveniences; they never become a second simulation model. Their
shared Rust contract separates geometry from semantic entities, paths, exact
metrics, annotations, and transition cues. Tabler icons render a constrained
semantic glyph registry instead of embedding arbitrary SVG in artifacts.

Each problem advertises Micro, Showcase, and Stress instances. Showcase is the
default and is also the committed offline artifact; Micro and Stress are live
engine choices. The comparison surface puts replayable outcomes, objective
values, trace lengths, and algorithm evidence side by side, while uniform
complexity telemetry exposes accounts, rates, transitions, rejection proofs,
and alternatives across every domain.

`Run` asks the selected engine for a new reproducible artifact; the playback
transport only scrubs the selected artifact's already verified exchange trace.
The separate Solve view follows typed phase, rollout, tree, and frontier
observations, and the completed artifact retains a bounded copy so a fast run
has the same inspectable evidence as a slow live stream. Completion leaves a
persistent receipt and marks the newly loaded artifact.

The client chooses capabilities in order: a currently healthy native HTTP/SSE
engine, the Rust/Wasm service in an isolated Web Worker, then committed static
Showcase playback. The badge reflects current health or Worker initialization;
it does not infer connection from a catalog loaded earlier. Native runs expose
pause, resume, and cancel. Browser cancellation terminates the disposable
Worker immediately; resumable browser pause is not advertised.

The constraint-probe panel contains deliberately invalid or infeasible
proposals. Their expected rejection is evidence that the model's constraints
are active, not a failed run. Structured issue kinds and subjects preserve the
specific missing role, account, asset, or rate instead of collapsing distinct
causes into duplicate messages. Actual execution and transport failures appear
separately as error banners.

## Run locally

Install `wasm-pack`, then install dependencies and start Studio:

```sh
cd studio
pnpm install
pnpm dev
```

Open `http://127.0.0.1:5173`. `predev` builds the Worker engine, so all twelve
problems can run without a server. To use native HTTP/SSE and resumable pause,
start this optional command from the repository root:

```sh
cargo run -p axionomy-studio-server --bin axionomy-studio
```

Vite proxies `/api` to `127.0.0.1:3000`. Turn the server off and the badge will
fall back to `browser engine ready` after the next health check; Run remains
available. The Rust-generated catalog and artifacts in `public/artifacts`
remain the final fallback if Worker initialization fails.

The production build is deployed automatically from `main` to
`https://biomaai.github.io/axionomy/`. It sets the Vite base to `/axionomy/`,
forces browser-engine mode so Pages does not make pointless API probes, and
ships the Worker, Wasm binary, all artifacts, and `.nojekyll` as one static
site. It requires no application server, database, WebSocket, or
`SharedArrayBuffer`.

## One source of browser types

Rust DTOs in `axionomy-view` derive Serde and Schemars. The server turns them into OpenAPI 3.1 with Aide, then `openapi-typescript` generates `src/generated/api.d.ts`. Do not edit the JSON contract or TypeScript declaration manually.

```sh
pnpm generate:contracts
pnpm check:contracts
```

Quantities are exact decimal strings. `ViewId` joins user-defined ontology values by a presentation key while keeping labels and optional diagnostic JSON separate from semantic authority.

## Verify

```sh
pnpm build
pnpm test
pnpm exec playwright install chromium
pnpm test:e2e
pnpm test:e2e:wasm
```

The Playwright tests launch both servers, start a real run, follow its SSE
stream, observe fine-grained logistics progress and pause/resume behavior,
fetch and scrub its replay-verified artifact, verify structured rejection
proofs, and inspect a structurally different static Exact Cover matrix. Rust
adapter tests build all twelve problem artifacts.

The Wasm suite launches Studio with native probes disabled, runs stochastic
logistics through the Worker, verifies live and retained solver evidence,
loads the replay-derived artifact, and proves responsive cancellation by
terminating and recreating the isolated Worker.
