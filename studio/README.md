# Axionomy Studio

Axionomy Studio is the browser trace player and economic debugger for Axionomy. Its universal panels inspect account balances, exchange bindings, assessments, receipts, and objectives. Optional graph or grid scenes are derived conveniences; they never become a second simulation model. The included Maze scene and exact Pareto plot are the first reference projection.

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

Open `http://127.0.0.1:5173`. Vite proxies `/api` to the server at `127.0.0.1:3000`. The committed document in `public/examples` also loads when the server is unavailable.

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

The Playwright test launches both servers, starts a real Maze run, follows its SSE stream, fetches the completed document, and scrubs an assessed exchange.
