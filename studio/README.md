# Axionomy Studio

Axionomy Studio is the browser trace player and economic debugger for Axionomy.
All twelve canonical problems are discoverable and runnable. Universal panels
inspect balances, exchange bindings, assessments, projected and committed
deltas, encoded rates and invariants, objectives, alternatives, telemetry, and
actor-relative observations. Graph, grid, matrix, and timeline scenes are
derived conveniences; they never become a second simulation model.

`Run` asks the native engine for a new reproducible artifact; the playback
transport only scrubs the selected artifact's already verified exchange trace.
During a live run, Studio shows the current phase, elapsed time, deterministic
request parameters, and bounded Monte Carlo/MCTS/ISMCTS work counters. Pause
and cancellation are observed between those chunks. Completion leaves a
persistent receipt and marks the newly loaded artifact so a fast computation
still has an obvious outcome.

The constraint-probe panel contains deliberately invalid or infeasible
proposals. Their expected rejection is evidence that the model's constraints
are active, not a failed run. Structured issue kinds and subjects preserve the
specific missing role, account, asset, or rate instead of collapsing distinct
causes into duplicate messages. Actual execution and transport failures appear
separately as error banners.

## Run locally

From the repository root, start the native server:

```sh
cargo run -p axionomy-studio-server --bin axionomy-studio
```

In another terminal:

```sh
cd studio
pnpm install
pnpm dev
```

Open `http://127.0.0.1:5173`. Vite proxies `/api` to the server at
`127.0.0.1:3000`. The Rust-generated catalog and artifacts in
`public/artifacts` provide every problem when the server is unavailable.

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
```

The Playwright tests launch both servers, start a real run, follow its SSE
stream, observe fine-grained logistics progress and pause/resume behavior,
fetch and scrub its replay-verified artifact, verify structured rejection
proofs, and inspect a structurally different static Exact Cover matrix. Rust
adapter tests build all twelve problem artifacts.
