# axionomy-studio-wasm

The Web Worker adapter for Axionomy Studio. It exports the same Rust-owned
problem catalog, study request, solver observations, and study used by the
native HTTP service: `study()` computes one and emits the same Studio events.
JavaScript schedules and transports the computation; it does not reimplement
any economic semantics.

Beside the studies, which Studio on packages retires
([plan](../../docs/SIMULATION.md#plan-axionomy-on-primitives), P12), it
exports the engine itself (P11, D-047): `Engine`, an `axionomy-engine` over
the Worker's store.

- `Engine.open(handle)` opens the store on a `FileSystemSyncAccessHandle` to
  a file of the origin private file system, through a redb backend over the
  handle, so it stays across reloads; `Engine.open()` opens one in memory.
  `persistent` says which.
- `load(bytes, url, etag, last_modified)` loads a package file, a store file
  or a zip of one, as the engine loads any package (its manifests checked,
  then merged), and gives the manifests and the names the store kept. Given
  the URL it came from, the store keeps the response's validators, which
  `source(url)` gives back while the store still holds every package it
  loaded from there: the Worker sends them with its next request for the
  URL, which answers 304 when it serves the same.
- `packages()`, `experiments()`, `runExperiment(path, progress)`,
  `results(path)` and `draw(point)` list the store's manifests and
  experiments, run one, read its last results and draw a point's history
  with its view.

Paths and points are parsed where they cross from JavaScript, and results
come back as the engine's JSON. The Worker (`studio/src/engine.worker.ts`)
opens the handle, fetches packages and posts replies;
`studio/src/browserPackages.ts` is its client.

Build it through `pnpm build:wasm` in `studio/`. The generated bindings are
consumed by Vite and intentionally remain build output.
