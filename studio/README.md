# Axionomy Studio

Axionomy Studio is the browser trace player and economic debugger for Axionomy.
Every problem can be discovered and studied. **Source of truth** shows
accounts and assets; **The rules** shows rates, roles, goals, and invariants;
**One step** explains the selected exchange and its exact effects.

Graph, grid, matrix, timeline, and reserves pictures help people read a result, but they
never become a second simulation model. Geometry, semantic entities, paths,
metrics, annotations, and transition cues all come from the same Rust contract.
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

**Thinking** retains search progress, **Step-by-step replay** scrubs
accepted exchanges, and **Tradeoffs** compares non-dominated outcomes. Tabler
icons come from a constrained semantic glyph registry rather than arbitrary SVG
inside studies.

Replay uses one generic cockpit across every problem: the disposable scene,
replay-derived standings, current-step narration, and authoritative account
ledger remain visible together on wide screens and stack without page overflow
on narrow screens. Balances that never change in the selected trace may be
folded for density, but remain explicitly labeled as unchanged assets and are
always inspectable. The viewer does not reinterpret them as configuration or
remove them from the source-of-truth ledger. The mouse wheel scrolls the page
over the picture; its +/− controls, a pinch, or Ctrl with the wheel zoom it. An
outcome without exchanges says so and has nothing to play. While a saved study
downloads, Studio shows how much has arrived and holds Study and Copy link
until it is on screen.

Each problem declares its instances, family and capabilities, each a key with
a label, and Studio shows the labels it receives; the phases and algorithms of
thinking read the same way. The sample problems declare Micro, Showcase, and
Stress instances (`?instance=micro`). A problem's default instance, Showcase
for the samples, is also the pre-built offline study; the others are
live engine choices. The comparison surface puts replayable outcomes, objective
values, trace lengths, and each algorithm's own work (its counters, in the
words it reports them) side by side, while uniform complexity telemetry exposes
accounts, rates, transitions, rejection proofs, and alternatives across every
domain. An outcome is named by its strategy's label, as in the Strategy picker;
outcomes a study computes beside its strategies are listed apart as computed
for comparison. The heading names the problem as the picker does.

The Micro Mission's coordinated replay executes repeated ISMCTS decisions, conditions
beliefs after each public action and required Nature response, and retains the
search evidence for those committed actions. Actor observations are derived
from each replay frame; the viewer shows the Scout's private sighting before
it reaches the Medic. A post-scan constraint probe explains why coordinated
movement requires sharing first. The sampled Pareto view is explicitly labeled
as fixed-policy baseline evaluation, since it does not evaluate the planner.

Outcome comparisons open by default and show initial-to-final scene metrics
alongside objectives and search evidence, making the existing alternatives
readable before opening an individual trace.

The Living Market draws its pool on the reserves surface, in its own words: an
exact reserve curve, replayed price path, direct
per-actor price attribution, three counterfactual realities, and final exact
Shapley attribution across every coalition of active actors. No external price
or oracle enters the study; every displayed price comes from replayed pool
reserves.

Showcase Work League now advances a shared world clock: multiple activities
can overlap, repair and charging bays remain occupied until their due time,
and contracts can expire or transfer to another worker for payment. The scene
shows world time, active work, service occupants, claim deadlines, missed jobs,
spendable credit, material, and wear. Each comparison field uses the same
job-attempt Nature tickets. The small Micro fixture remains available.

The Micro Logistics MCTS alternative executes an entire adaptive journey, retaining
its actual deliveries or explicit failure and the search choices that were
committed. Its fixed-policy Monte Carlo frontier is not attributed to the
planner. Search and environment random streams are separate.

Larger Logistics profiles run two vehicles with different cargo capacities,
three customers, arriving orders, delivery windows, shared road occupancy,
finite fuel and repair parts, paid service bays, towing and hub relays.
Showcase and Stress use responsive fleet policies; the catalog and telemetry
distinguish them from Micro's MCTS. Route outcomes are encoded as compact hidden
integer balances, and each trace applies the same seed/vehicle/trip/road tickets.

Larger Mission profiles give Scout and Medic independent travel and work
timelines. Private discoveries, finite radio charges, in-person briefings,
joint extraction, archive recovery, evolving hazards and clinical deterioration
shape their decisions. Rescue independently models several hidden casualties,
fallible hints, stabilization, finite resupply and one evacuation seat. Both
derive actor observations at each replay frame and retain failed objectives.

Bridge traffic holds the lane for the full crossing duration and compares
deadline-responsive auctions and concurrent paid detours against fixed
priority. Marketplace reserves real funds, inventory, and carrier capacity
before shipping; settlement, price concessions, late rebates, and refunds
remain visible as separate exchanges. Scheduling distinguishes queuing from
executing, with alternative machines, setup costs, shared crews and resumable
work after a timed machine repair. Their larger comparisons use the same
initial scenario for every alternative and state search limits explicitly.

Perishables now follows cohorts through scarce cold storage, warm exposure,
splitting, processing, delivery and spoilage. A child cohort retains its
parent's heat history. Backup generation and transport spend the same fuel;
refrigeration and preservation spend the same electricity. Four policies
share the seeded outage schedule, with unfulfilled food obligations visible at
the end of the horizon.

The Living Market's Showcase and Stress profiles run eight and twelve market
days. Actors revisit trade, production, capacity investment, and liquidity
under shared seeded sunlight. Daily needs, obligations, material output,
finite Treasury funding, storage limits, and delayed investment interact.
The scene shows unmet needs and missed obligations alongside price and depth;
a closed season is not a claim that every actor succeeded.

The Work League adds six replay-derived leaderboards. Contract value,
throughput, resource efficiency, residual waste, reliability, and Pareto
standing are recomputed from each verified snapshot, so the lead can change as
the scrubber moves. A compact ranking-dimension selector exposes every board
without hiding choices in a horizontal scroller. Rank evidence links back to
the participant account; it is never a second score store.

Problem, instance, strategy, document, Thinking or replay view, replay step, selected
leaderboard, seed, and budget are encoded in the query string. **Copy link**
therefore shares the exact inspection state on the native server, browser Wasm,
or GitHub Pages. Major selections participate in Back/Forward history, while
autoplay and scrubbing replace the current history entry. A link never starts
compute automatically. A parameter a link omits means the service's default
(seed 17, budget 128). The heading names the key of the study on screen; when
the controls ask for a study that is not loaded (another instance, seed or
budget, or an outcome the study lacks), a notice says what was asked for, what
is shown instead, and how to compute it. A link's outcome, step, or number that
Studio cannot open is reported, never replaced silently.

`Study` asks the selected engine for a new reproducible study; the playback
transport only scrubs the selected study's already verified exchange trace.
The separate Thinking view follows typed phase, rollout, tree, and frontier
observations, and the completed study retains a bounded copy so a fast study
has the same inspectable evidence as a slow live stream. A saved review starts
on the last search checkpoint: preparing the study and recording its documents
are bookkeeping around the search, and a strategy that records no search says
so instead of listing that bookkeeping. Any graph scene whose search reports
the nodes it expanded becomes a map of that search, under the scene's own
title; legend entries for the path statuses the map does not draw are left
out. Phases, algorithms and every other word the Thinking view shows come with
the observations, except the names of the generic checkpoint kinds (frontier,
rollouts, search tree, beliefs, candidates, solutions, pruning) that are
Studio's own vocabulary. A link that names no problem opens the catalog's
first. Completion leaves a persistent receipt
and marks the newly loaded study.

The client chooses capabilities in order: a currently healthy native HTTP/SSE
engine, the Rust/Wasm service in an isolated Web Worker, then pre-built static
Showcase playback. The badge reflects current health or Worker initialization;
it does not infer connection from a catalog loaded earlier. Native studies expose
pause, resume, and cancel. Browser cancellation terminates the disposable
Worker immediately; resumable browser pause is not advertised.

Planned: Studio selects a point, the economy after a given step of a recorded
run, and its controls act on it: scrub and play read a history and change
nothing; Follow shows a ref's steps as they are delivered; Act applies a
candidate or a written exchange, assessed first, and on the point a ref is on
also moves the ref, while anywhere else it makes a branch; Continue from here
moves a ref to the selected point; Compare shows two histories. The
native and browser engines both offer them; static playback stays read-only
([plan](../docs/SIMULATION.md#plan-axionomy-on-primitives)).

The constraint-probe panel contains deliberately invalid or infeasible
proposals. Their expected rejection is evidence that the model's constraints
are active, not a failed study. Structured issue kinds and subjects preserve the
specific missing role, account, asset, or rate instead of collapsing distinct
causes into duplicate messages. Actual execution and transport failures appear
separately as error banners.


The larger Maze profiles now expose shared vault/canal power, protective gear,
persistent injury delays, and medical recovery. Sokoban uses crates to hold
pressure plates, remote latches to preserve door access, and a far-side exit;
Stress composes two circuits. Exact Cover uses irregular globally overlapping
sets with legal dead ends and different-size solutions. Connect Four exposes
forced wins, blocks and double threats, and compares tactical MCTS with two
independent opponents. These remain replayable economic exchanges, with the
new state visible in scene metrics and account inspection.

## Run locally

Install `wasm-pack`, then install dependencies and start Studio:

```sh
cd studio
pnpm install
pnpm dev
```

Open `http://127.0.0.1:5173`. `predev` builds the Worker engine, so every
problem can be studied without a server. To use native HTTP/SSE and resumable pause,
start this optional command from the repository root:

```sh
cargo run -p axionomy-cli -- studio
```

Vite proxies `/api` to `127.0.0.1:3000`. Turn the server off and the badge will
fall back to `Running in your browser` after the next health check; Study
remains available. The Rust-generated catalog and studies in `public/studies`
remain the final fallback if Worker initialization fails.

The production build is deployed automatically from `main` to
`https://biomaai.github.io/axionomy/`. It sets the Vite base to `/axionomy/`,
forces browser-engine mode so Pages does not make pointless API probes, and
ships the Worker, Wasm binary, every pre-built study, and `.nojekyll` as one static
site. It requires no application server, database, WebSocket, or
`SharedArrayBuffer`.

## One source of browser types

Rust DTOs in `axionomy-view` derive Serde and Schemars. The server turns them into OpenAPI 3.1 with Aide, then `openapi-typescript` generates `src/generated/api.d.ts`. `pnpm generate` writes that contract, its TypeScript declarations, the pre-built studies in `public/studies` and every sample's package, zipped, in `public/packages` from the Rust source; `pnpm dev`, `pnpm test` and `pnpm build` run it first. None of it is committed, so it can never be stale.

```sh
pnpm generate
```

Quantities are exact decimal strings. `ViewId` joins user-defined ontology values by an item path such as `maze/account/agent` while keeping labels and optional diagnostic JSON separate from semantic authority. Problems, strategies, documents and every other key are kebab-case paths, so Studio links read `?problem=work-league&strategy=mixed-field`; a study started from Studio gets a minted ID such as `study-7`, and once published it is fetched through that ID.

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

The Playwright tests launch both servers, start a real study, follow its SSE
stream, observe tactical search progress and pause/resume behavior,
fetch and scrub its replay-verified study, verify structured rejection
proofs, and inspect a structurally different static Exact Cover matrix. Rust
tests in `axionomy-problems` study every problem.

The Wasm suite launches Studio with native probes disabled, studies stochastic
logistics in the Worker, verifies live and retained thinking,
loads the replay-derived study, and proves responsive cancellation by
terminating and recreating the isolated Worker. It also drives the engine
itself in the Worker (`src/browserPackages.ts`, which Studio's UI moves onto
in P12): a Worker loads `packages/maze.zip` from its URL into a store on the
origin private file system, runs its experiment and, after a reload, finds
the package and the results again, the URL asked only whether it changed
and answering 304; a second Worker, while the first holds that store, keeps
one in memory and says so.
