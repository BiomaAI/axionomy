# axionomy-studio-server

This crate is the native reference backend for Axionomy Studio. It adapts the
same `axionomy-service` used by CLI and MCP into one problem catalog,
pausable/resumable/cancellable in-memory studies, replay-derived documents,
paginated frames, Server-Sent Events, and an OpenAPI 3.1 contract. Each study
gets a minted ID such as `study-7`; once published, the study, its documents
and their frames are served under `/api/studies/{study_id}/study`. It
intentionally has no database: a study in progress is operational UI state,
while a published, portable `Study` can be exported to JSON whenever durable or
static playback is wanted.

Run the API and optional built Studio frontend:

```sh
cargo run -p axionomy-studio-server --bin axionomy-studio
```

The default address is `127.0.0.1:3000`; override it with `AXIONOMY_STUDIO_BIND`. If `studio/dist` exists, the server hosts it with an SPA fallback; `AXIONOMY_STUDIO_DIR` names another built frontend. The OpenAPI contract is available at `/api/openapi.json`.
On startup the command prints clickable Studio, health, and OpenAPI URLs using
the address and port the operating system actually bound. It also reports when
the frontend has not been built and prints the command needed to build it.

Studio generates its browser contract and complete static catalog with
`pnpm generate`, which runs:

```sh
cargo run -p axionomy-studio-server --bin export-openapi -- studio/openapi.json
cargo run -p axionomy-studio-server --bin export-studies -- all studio/public/studies
```
