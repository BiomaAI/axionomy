# Axionomy Studio

Axionomy Studio is the browser trace player and economic debugger for Axionomy.
It opens on **packages**
([plan](../docs/SIMULATION.md#plan-axionomy-on-primitives), P12): the
economies, refs, strategies, experiments and views a package carries, served
by an engine, native or in the browser, that Studio asks the same way.

- **Load a package** from a file (a store file or a zip of one), or open a
  sample by choosing it (`packages/<name>.zip`, which `packages/index.json`
  lists; with nothing loaded, Studio lists them on its first page); a link
  such as `?package=packages/maze.zip` loads one when it opens, and a sample
  is linked by that path. The link keeps the URL, the package, the ref or point, the step and
  the view (`name`, `ref`, `point`, `step`, `view`), so a reload or a copied
  link opens the same. A package is a release: loading another version of
  one the store holds replaces what the held version shipped, and Studio
  says so; loading the same version again changes nothing. Refs and runs of
  your own stay. What an action did is said once, in place of what the last
  one said: a confirmation goes by itself, and a caution (a package that
  could not be reached, a version replaced, a ref or definition kept) stays
  until it is dismissed.
- **A package's refs are fixed**, so the package means what it shipped:
  at one, **Name a ref here** names its point with a ref of yours
  (`<package>/mine` unless you name it otherwise), which moves as you play. A ref of your own can be deleted; the
  points it named stay.
- **A ref or a point** draws its history with the package's view, in the
  playback, the picture and the inspectors; a ref opens where it is, at the
  end of its history. A point with no history yet says so in place of
  playback, and how to make one.
- **What to do here** sits beside the picture, so what an action does shows
  at once: **Act**, **Continue from here** and **Follow**.
- **Act** works at the step on screen. A candidate set's exchanges there are
  listed, each assessed (applicable, infeasible or invalid, with what it
  lacks), and an exchange can be written as JSON and assessed before it can
  be applied. At the point a ref of your own is on, applying also moves the
  ref over the new step, and is told where the ref went if another mover
  came first; anywhere else, a package's ref's point among them, it makes a
  branch, a new point no ref names, which **Name a ref here** names.
- **Continue from here** runs a strategy from the step on screen toward a
  goal picked from the package's goals, with a seed, a budget of expansions
  for a search and the most turns a policy or a game plays (the goal and the
  effort an experiment gives that strategy, when one runs it, and otherwise
  the package's first goal, a budget of 1000000 and 1000 turns), reports its
  progress (a policy's or a game's turns counted as a search counts
  expansions), can be cancelled, whatever its kind, and shows how it ended,
  where, and its thinking: what the search expanded, generated, left on its
  frontier and visited, and the solution's depth. From the point of a ref of
  your own, the ref can move to where it ended.
- **Follow** delivers the steps a ref is moved over as they come, by this
  page or any other proposer, each with the package's objectives there.
- **Experiments** lists the package's experiments; running one reports its
  progress and can be cancelled (a cancelled experiment keeps no results).
  Each names its arms before it runs; each arm's trials are listed with
  their seeds, how they ended, the step they ended at and their exact
  objective values; **Pick by** orders the objectives, and the trial
  ends no other dominates are marked, the pick highlighted. A trial's end
  opens like any point.

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
+/− controls, a pinch, or Ctrl with the wheel zoom it. A point without
exchanges says so and has nothing to play.

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
