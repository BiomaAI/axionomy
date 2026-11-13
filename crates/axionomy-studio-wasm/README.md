# axionomy-studio-wasm

The Web Worker adapter for Axionomy Studio. It exports the engine itself
(P11, D-047), offering the operations the native Studio server offers (P12,
D-045). JavaScript schedules and transports the computation; it does not
reimplement any economic semantics.

`Engine` is an `axionomy-engine` over the Worker's store.

- `Engine.open(handle)` opens the store on a `FileSystemSyncAccessHandle` to
  a file of the origin private file system, through a redb backend over the
  handle, so it stays across reloads; `Engine.open()` opens one in memory.
  `persistent` says which.
- `load(bytes, url, etag, last_modified)` loads a package file, a store file
  or a zip of one, as the engine loads any package (its manifests checked,
  then merged, or in place of the version of it the store holds), and gives
  a `LoadedView`, the packages it held, the names the store kept and the
  versions it replaced. Given the URL it came from, the store keeps the response's
  validators and the packages it held, which `source(url)` gives back while
  the store still holds every one of them: the Worker sends the validators
  with its next request for the URL, which answers 304 when it serves the
  same.
- The operations the server serves, each calling the same
  `axionomy-studio-api` function and giving the same JSON: `packages`,
  `draw`, `ancestor`, `balances`, `candidates`, `assess`, `apply`,
  `runStrategy` and `runExperiment` (calling back with their progress),
  `results`, `compare`, `createRef`, `readRef`, `advanceRef`, `deleteRef`
  and `frames`. A request is the JSON of the server's body or query, read as
  its extractors read it; a point is a `PointId` and a path its text. The
  engine's wait blocks a thread, which a Worker has only one of, so
  `frames` answers at once; `moves` counts the moves of the engine's refs,
  and the Worker keeps a follower whose ref has not moved waiting until it
  changes and the ref has moved, or the wait is over.

A refused operation throws the JSON of `{status, body}`: the status the
server answers it with (404, 400, 403 for a ref a package ships, 409 with
`RefMoved`, 500) and its body, so Studio's client refuses alike whichever
engine refused. A synchronous run reads no message until it returns, so
cancelling one terminates the Worker while it runs it and starts another on
the same store, asked again for every other request waiting; a terminated
Worker leaves everything it committed, and loses only an experiment's trial
records it had not made
durable. The Worker (`studio/src/engine.worker.ts`) opens the handle,
fetches packages and posts replies; `studio/src/browserPackages.ts` is its
client, one of the two implementations of Studio's `PackageEngine`.

Build it through `pnpm build:wasm` in `studio/`. The generated bindings are
consumed by Vite and intentionally remain build output.
