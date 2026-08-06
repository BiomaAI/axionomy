# axionomy-studio-server

This crate is the native reference backend for Axionomy Studio. It exposes the example catalog, cancellable in-memory runs, replay-derived documents, paginated frames, Server-Sent Events, and an OpenAPI 3.1 contract. It intentionally has no database: a run is operational UI state, while a portable `ViewDocument` can be exported to JSON whenever durable or static playback is wanted.

Run the API and optional built Studio frontend:

```sh
cargo run -p axionomy-studio-server --bin axionomy-studio
```

The default address is `127.0.0.1:3000`; override it with `AXIONOMY_STUDIO_BIND`. If `studio/dist` exists, the server hosts it with an SPA fallback. The OpenAPI contract is available at `/api/openapi.json`.

Regenerate the committed browser contract and a standalone trace with:

```sh
cargo run -p axionomy-studio-server --bin export-openapi -- studio/openapi.json
cargo run -p axionomy-studio-server --bin export-view -- maze_pareto_energy studio/public/examples/maze-pareto-energy.json
```
