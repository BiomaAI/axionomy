# Axionomy Studio

Axionomy Studio is the browser trace player and economic debugger for Axionomy.
It opens on **packages**
([plan](../docs/SIMULATION.md#plan-axionomy-on-primitives), P12): the
economies, refs, strategies, experiments and views a package carries, served
by an engine, native or in the browser, that Studio asks the same way.

- **One Package picker** lists the packages in the store and, below them,
  the samples not loaded yet (`packages/<name>.zip`, which
  `packages/index.json` lists; with nothing loaded, Studio lists them on its
  first page). Picking either opens it; **Load a file** opens a store file or
  a zip of one. A link such as `?package=packages/maze.zip` loads one when it
  opens. The link keeps the URL, the package, the ref or point and the step
  (`name`, `ref`, `point`, `step`), so a reload or a copied link opens the
  same. A package is a release: loading another version of one the store
  holds replaces what the held version shipped, and Studio says so; loading
  the same version again changes nothing. Refs and runs of your own stay.
  What an action did is said once, in place of what the last one said: a
  confirmation goes by itself, and a caution stays until it is dismissed.
- **The history tree** is the page's spine: every history the package's
  refs are on, and every branch grown from them, drawn as a timeline (steps
  left to right, each branch on a row below the step it split off at, long
  stretches drawn short with their steps counted). It is live: the engine
  tells the page of every run recorded, ref moved and definition kept as it
  happens, natively over a server-sent stream and in the browser from the
  Worker, so a strategy's branch, an experiment's trials or another tab's
  moves appear as they are made. Refs are labels on the steps they name
  (a package's own, which are fixed, in steel; yours in brass), experiment
  trials and branches made in this tab are labelled at their tips, and the
  history on screen is drawn in brass with the play cursor on it. Hovering a
  step says what its branch is; clicking it opens that branch at that step,
  and clicking a step of the history on screen moves the cursor there.
  `HistoryTree` knows only branches, steps and labels, so anything with a
  branching history can draw one.
- **Play** walks the history on screen in the picture and the inspectors.
  Where nothing has happened yet, such as a package's first ref when it
  opens, Play lets the strategy the package's first experiment runs find
  the moves and plays them; pressed again while it searches, it stops. Space
  plays and pauses, the arrow keys step.
- **From here**, beside the picture, is everything one can do at the step
  on screen, and says first what doing it does: at a ref of your own it
  moves the ref forward; anywhere else, a package's ref among them, it makes
  a branch.
  - **Moves here**: the moves the package's rules offer, each checked
    (applies, or what it lacks), each with **Apply**; a move opens its full
    check when clicked.
  - **Let a strategy play** picks a strategy and plays it from here toward
    the goal and with the effort its experiment gives it (or the package's
    first goal, a budget of 1000000 and 1000 turns); its branch opens at the
    step it started from and plays.
  - **Experiments from here**, for each experiment that starts at this
    point: **Play each arm** plays every arm's trials, each a new branch of
    the tree as it is recorded, and lists how each trial ended, in how many
    steps, and its objective values, with **Watch** to open and play it.
    Played again, it says whether the same seeds gave the same results.
  - **Name this point** names it with a ref of yours (`<package>/mine`
    unless you name it otherwise); at a ref of yours, **Follow it** delivers
    each step it is moved over, by this page or any other proposer, and
    **Delete it** deletes it, its history staying in the tree.
  - **Write a move by hand**, folded away: an exchange checked before it can
    be applied.
- **Words explain themselves**: Studio's own words (package, history,
  branch, ref, fixed, move, strategy, goal, experiment, arm, trial, seed,
  budget, turns, follow) are dotted where they are met, and say what they
  mean on hover or focus.

Natively the store is the server's, so whatever another client does to a
ref there shows here; in the browser it is the Worker's, on the origin
private file system where it stays across reloads, so a sample loaded again
after Pages publishes a new version is that version, not a mix of both.

**Source of truth** shows accounts and assets; **The rules** shows rates,
roles, goals, and invariants; **Proof of this step** explains the selected
exchange: what was checked before it applied and its exact effects.

Graph, grid, matrix, timeline, and reserves pictures help people read a result, but they
never become a second simulation model. Geometry, semantic entities, paths,
metrics, annotations, and transition cues all come from the package's view,
drawn by the engine into the same Rust contract every view is drawn into.
Graph scenes use one semantic grammar across domains: topology is the stable
structure, account state is folded into its owner, agents and vehicles dock to
locations or move on routes, jobs and orders form tethered collections, and
scenario-wide uncertainty occupies a separate context lane. Shapes, grouping,
and relationship lines therefore communicate what belongs where without
inventing presentation-only mechanics.

[`VISUAL_LANGUAGE.md`](VISUAL_LANGUAGE.md) is the normative
design reference. It names Work League as the canonical graph scene, Sokoban
as the canonical grid, scheduling as the canonical timeline, and Exact Cover
as the canonical matrix, and defines the geometric checks expected before a
new visualization is accepted.

Grid scenes use the same identity discipline. Cells describe terrain and link
to their authoritative accounts; players, crates, and pieces are explicit
entities layered above them. A stable entity ID lets the generic renderer move
the same object between anchors, emphasize receipt-derived effects, and settle
it onto goals. Studio always uses the full replay choreography so movement and
state changes remain visually explicit.

Playback uses one generic cockpit for every package: the scene, current-step
narration, and authoritative account ledger remain visible together on wide
screens and stack without page overflow on narrow screens. Balances that never
change in the history on screen may be folded for density, but remain
explicitly labeled as unchanged assets and are always inspectable. The viewer
does not reinterpret them as configuration or remove them from the
source-of-truth ledger. The mouse wheel scrolls the page over the picture; its
+/− controls, a pinch, or Ctrl with the wheel zoom it. Space plays or
pauses and the arrow keys step. Play on a point where nothing has happened
yet, such as a package's first ref when it opens, runs the strategy the
package's first experiment runs toward its goal and plays the moves it
finds; pressed again while it searches, it stops.

## Run locally

Install `wasm-pack`, then install dependencies and start Studio:

```sh
cd studio
pnpm install
pnpm dev
```

Open `http://127.0.0.1:5173`. `predev` builds the Worker engine and the
sample packages, so Studio runs every package without a server. To use the
native engine, start this optional command from the repository root:

```sh
cargo run -p axionomy-cli -- studio
# with packages loaded before it serves, and its store kept in a file:
cargo run -p axionomy-cli -- studio --store work.redb --package studio/public/packages/maze.zip
```

Vite proxies `/api` to `127.0.0.1:3000`. Studio chooses its engine once,
when the page opens, since the server's store and the browser's are not the
same: a page opened while the server answers works on the server's store
until it is reloaded, and one opened while it does not runs the engine in the
browser's Worker.

The production build is deployed automatically from `main` to
`https://biomaai.github.io/axionomy/`. It sets the Vite base to `/axionomy/`,
forces browser-engine mode so Pages does not make pointless API probes, and
ships the Worker, Wasm binary, every sample package, and `.nojekyll` as one
static site. It requires no application server, database, WebSocket, or
`SharedArrayBuffer`.

## One source of browser types

Rust DTOs in `axionomy-view` and `axionomy-studio-api` derive Serde and Schemars. The server turns them into OpenAPI 3.1, then `openapi-typescript` generates `src/generated/api.d.ts`. `pnpm generate` writes that contract, its TypeScript declarations and every sample's package, zipped, in `public/packages` from the Rust source; `pnpm dev`, `pnpm test` and `pnpm build` run it first. None of it is committed, so it can never be stale.

```sh
pnpm generate
```

The engine's operations are typed once, in Rust (`axionomy-studio-api`), and
Studio reaches them through one interface, `PackageEngine`
(`src/packageEngine.ts`), with two implementations: `nativePackages`, over
the Studio server's HTTP API, and `BrowserPackages` (`src/browserPackages.ts`),
over the Worker's engine, which runs the same Rust functions and posts the
same JSON. Both use the generated OpenAPI types; a refusal is an
`EngineRefusal` with the server's status, and a ref found elsewhere carries
where it is. A strategy's or an experiment's run is a `Running`: its
progress as it comes, its end, and `cancel`, which closes the native stream
or terminates the Worker while it runs the run, asking the next Worker every
other request still waiting, so nothing else notices. A follower in the
Worker waits for its ref to move, asking nothing meanwhile. The engine is
chosen once, when Studio opens: the native server when it answers, else the
Worker, since each keeps its own store. Points are `PointId`s (`run-…:3`);
seeds and budgets are decimal text, since JSON's numbers cannot carry every
64-bit one.

Quantities are exact decimal strings. `ViewId` joins user-defined ontology values by an item path such as `maze/account/agent` while keeping labels and optional diagnostic JSON separate from semantic authority.

## Verify

```sh
pnpm build
pnpm test
pnpm exec playwright install chromium
pnpm test:e2e
pnpm test:e2e:wasm
```

Run `pnpm build` before either browser suite. Playwright starts Vite directly
and reuses that build's Wasm artifacts, keeping Wasm optimization outside the
server startup timeout. CI uses the same sequence.

`tests/packages.spec.ts` runs in both suites, against the native server and
the Worker alike: it loads a sample package and scrubs a ref's history,
applies a candidate at a ref's point (the ref moves) and at an earlier step
(a branch), refuses to apply a written exchange before it is assessed
applicable, continues from a ref's point with a strategy and moves the ref
to where it ended, follows a ref while the page moves it, runs an experiment
and compares its trials, and opens a `?package=` link and reloads it. The
native server's store outlives a test, so each test that moves a ref has
one of its own and counts from where it is. Natively, the suite also
follows a ref that another proposer moves over the HTTP API.

The Wasm suite also drives the engine itself in the Worker
(`src/browserPackages.ts`), from a page that runs no Studio
(`tests/worker.html`) so no Worker of Studio's holds the store: a Worker
loads `packages/maze.zip` from its URL into a store on the origin private
file system, runs its experiment and, after a reload, finds the package and
the results again, the URL asked only whether it changed and answering 304;
a follower waits in the Worker for its ref to move, asking nothing
meanwhile; a second Worker, while the first holds that store, keeps one in
memory and says so; and cancelling a game mid-run terminates its Worker,
the next one opening the store with all committed before.
