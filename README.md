<picture>
  <source media="(prefers-color-scheme: dark)" srcset="assets/axionomy-logo-dark.webp">
  <source media="(prefers-color-scheme: light)" srcset="assets/axionomy-logo-light.webp">
  <img alt="Axionomy" src="assets/axionomy-logo-light.webp">
</picture>

> A closed economic state machine for verifiable problem solving.

**Everything is an asset. Every change is an exchange.**

Axionomy treats every problem as an economy. Assets represent anything that can
exist or matter, including resources, facts, permissions, beliefs, goals, and
state. Accounts define where those assets belong, rates define the laws by
which they may change, and exchanges are the only events that make those
changes real. Search algorithms, optimizers, simulations, and policies may
explore this economy and propose exchanges, but they never own its truth; the
economy remains the single authoritative model, making every accepted decision
inspectable, verifiable, and replayable.

Because every transition exposes explicit preconditions, shortfalls, consumed
resources, preserved facts, produced outcomes, and invariant violations,
Axionomy provides much denser feedback than a sparse success-or-failure
signal—especially for reinforcement learning, where it can supply valid-action
masks, intermediate progress, resource costs, and structured failure reasons.
The same encoding can also serve graph search, optimization, Monte Carlo
simulation, and learned policies without duplicating domain logic; exact forks
enable counterfactual exploration, while replayable traces make decisions
auditable, comparable, and independently verifiable.

Axionomy is a Rust engine for encoding bounded problem spaces through four
primitives:

| Primitive | Meaning |
| --- | --- |
| Asset | A resource, fact, proposition, capability, condition, memory item, or state token |
| Account | An owner, actor, location, scope, or namespace |
| Rate | A law describing what may be consumed, produced, and preserved |
| Exchange | One concrete, role-bound firing of a rate |

The non-negotiable rule is:

> Nothing semantically authoritative may live outside assets, accounts, rates,
> and exchanges.

There is no parallel `WorldState`. Position, topology, time, goals, machine
capacity, uncertainty, beliefs, random seeds, bids, and rewards are modeled in
the same closed state. Search and optimization algorithms may fork that state
and propose exchanges, but only the core can accept a transition.

```text
State       = Account × Asset → Quantity<N>
Problem     = Initial accounts + available rates + declared invariants
Computation = A sequence of valid exchanges
Goal        = A required asset configuration
Solution    = A replayable exchange trace reaching that configuration
```

Fungibility follows asset identity rather than a privileged token class. A
balance of `10_000 × Claim(CohortA)` is one fungible quantity, while
`Condition(CohortA, Fresh) = 1` is a unique state fact whose lifecycle is
protected by rates and invariants. This also supports non-fungible goods by
giving each item a unique asset identity and conserving its unit supply. When
many holders share one fate, their balances can remain stable claims while a
single cohort or pool account carries the authoritative condition or epoch.

See [PDD.md](PDD.md) for the product and technical contract and
[PROBLEMS.md](PROBLEMS.md) for the conformance problems that drive the API.

## Workspace

The repository separates execution semantics, solver strategies, and domain
models into one-way workspace dependencies:

| Crate | Responsibility |
| --- | --- |
| `axionomy` | Pure closed-state validation and exchange execution kernel |
| `axionomy-search` | Non-authoritative graph and Pareto search, rollout, Monte Carlo, perfect-information and information-set MCTS, and learning projections |
| `axionomy-problems` | Canonical problem encodings, specialized proposers, and conformance tests |
| `axionomy-units` | Self-describing asset denominations, schema coherence, and dimension-safe `uom` authoring |
| `axionomy-time` | Calendar-aware Jiff authoring over the same canonical timeline denominations |
| `axionomy-mcp` | Strict stateless MCP 2026-07-28 reference server with caller-owned snapshot storage and interruptible search tasks |
| `axionomy-view` | Runtime-neutral, replay-derived browser presentation contracts |
| `axionomy-service` | Interface-neutral catalog, run control, progress, and replay-derived artifacts for every canonical problem |
| `axionomy-cli` | Human- and script-friendly access to the shared problem service |
| `axionomy-studio-server` | Native OpenAPI/SSE reference backend and in-memory run orchestration for Axionomy Studio |

```text
axionomy-search ──────→ axionomy
axionomy-problems ────→ axionomy
                    └─→ axionomy-search
axionomy-units ───────→ axionomy
axionomy-time ────────→ axionomy-units ──→ axionomy
axionomy-view ────────→ axionomy
axionomy-service ─────→ axionomy-problems + axionomy-view
axionomy-cli ─────────→ axionomy-service
axionomy-mcp ─────────→ axionomy-service + axionomy-search + axionomy
axionomy-studio-server → axionomy-service
```

The kernel does not depend on a solver, problem, unit, or time crate. The
packages are independently publishable: release `axionomy` first; search and
units may follow; time follows units; problems and view follow the reusable
layers; the service follows those; CLI, MCP, HTTP, and Studio remain outer
adapters.

## What is implemented

- User-defined asset, account, rate-ID, and role types.
- Atomic multi-account rewrite rates.
- Separate consume, produce, and preserved/read-only baskets per role.
- Explicit exchange role bindings and multiplicity.
- Generic checked `Quantity<N = u64>` arithmetic with `u64` and optional
  non-`Copy` `BigUint` support.
- Stable insertion-ordered model collections and direct Serde support for
  economies, rates, accounts, baskets, exchanges, receipts, traces, and goals.
- Structured construction, validation, arithmetic, and invariant errors.
- Asset-qualified `AssetAmount` values, self-describing unit-aware asset keys,
  model-wide denomination validation, and exact `uom` and Jiff adapters.
- Explanatory exchange assessments with complete multi-account shortfalls and
  projected receipt deltas.
- Boolean `is_applicable` checks and bulk `applicable` candidate filtering.
- Global declared linear invariants checked on every firing.
- Successful application prepares and commits only touched account contents;
  linear conservation is checked from exchange deltas without scanning
  unrelated balances.
- Asset-configured goals.
- Isolated forks, speculative execution, and deterministic trace replay.
- Account-restricted economic views with canonical observation identities.
- Generic BFS, Dijkstra, A*, best-first search, replayable rollouts, weighted
  sampling, exact replayable Pareto fronts, approximate sampled policy fronts,
  Monte Carlo statistics, vector-valued MCTS,
  observation-scoped ISMCTS, and RL trajectory projections.
- Reproducible ChaCha sampling and established statistical estimators for
  means, variance, quantiles, tail risk, and Bernoulli credible intervals.
- Lazy action sources that derive concrete proposals from a full economy or an
  actor observation before core applicability filtering.
- Runtime-neutral resumable BFS, exhaustive Pareto, Monte Carlo, MCTS, and
  ISMCTS sessions with deterministic work budgets, progress snapshots, and
  cooperative observer interruption; a running Pareto front remains explicitly
  approximate until the reachable state space is exhausted.
- Forks share immutable laws and untouched account contents, with compact
  model-scoped state fingerprints for search caches.
- A strict MCP 2026-07-28 Streamable HTTP reference server with
  content-addressed immutable economy snapshots, a caller-provided storage
  boundary, an in-memory default, schema-backed tools, polling, progress, and
  cooperative cancellation through `rmcp::TaskManager`.
- Twelve closed benchmark encodings in `axionomy-problems`, with distinct
  solver strategies, independent reference oracles, and encoded stochastic
  priors.
- One interface-neutral problem service used by CLI, HTTP/Studio, and MCP,
  with twelve discoverable canonical problems, reproducible strategy requests,
  cooperative run control, progress, alternatives, and portable artifacts.
- Axionomy Studio's Rust-owned `ViewDocument` contract, replay-derived account
  snapshots, assessments and receipts, exact string quantities, graph, grid,
  matrix, and timeline scenes, alternative traces, Pareto fronts, stochastic
  telemetry, partial observations, model inspection, live resumable SSE runs,
  cancellation, OpenAPI-generated TypeScript, and static JSON for all twelve
  problems.

The core contains no application ontology or search algorithm. Problem assets
and specialized solvers compile against the same public kernel API available
to users.

## Multi-objective decisions without a second truth

Axionomy can expose every non-dominated outcome instead of forcing a caller to
hide tradeoffs inside one weighted score. A result dominates another only when
it is no worse on every declared objective and strictly better on at least one.
Objective values are read from terminal account balances—energy spent, process
time, participant utility, retained credit, completion time, preserved
inventory—not invented by the search. The search-layer schema supplies names
and minimize/maximize direction as disposable decision policy; it cannot make
an invalid exchange valid or alter an outcome.

This produces a more useful decision surface for allocation, planning, and
learning. Callers can inspect who benefits, defer the final tradeoff until
preferences are known, preserve discrete or non-convex alternatives that a
weighted sum can miss, and use the retained set as richer supervision for a
policy or reinforcement learner. Every exact entry carries a trace that is
replayed from the source economy. Exhaustive finite search is labeled `Exact`
only after completion; interrupted search and Monte Carlo policy fronts remain
visibly `Approximate`.

## Inspect with Axionomy Studio

Axionomy Studio is a universal economic debugger and trace player, not a
second simulator. It scrubs a trace by replaying accepted exchanges through
the Rust core, then shows the resulting accounts, assets, assessments,
receipt deltas, objectives, and optional read-only domain projection. The Maze
graph, Sokoban and Connect Four grids, Exact Cover matrix, scheduling and
perishables timelines, market and logistics networks, stochastic telemetry,
Pareto alternatives, rejected proposals, and actor-relative observations are
all projections over that same truth; none can make an exchange valid.

Studio, the CLI, the HTTP API, and MCP deliberately call the same
interface-neutral service. Their different usability demands pressure one
shared catalog, request, progress, control, and artifact API. A capability is
promoted toward the core only when it is independent of transport and
presentation and belongs to economic validation or replay; interface-specific
state stays outside. Cross-interface tests compare complete artifacts so the
four surfaces cannot quietly acquire different semantics.

```console
cargo run -p axionomy-studio-server --bin axionomy-studio
cd studio
pnpm install
pnpm dev
```

Open `http://127.0.0.1:5173`. The committed static catalog and all twelve
artifacts work without the server; connecting the native server enables
reproducible runs, Server-Sent Event progress, pause/resume/cancel control, and
freshly derived documents. Every canonical problem is present in both modes. See
[`studio/README.md`](studio/README.md) for contract generation and verification.

The same service is available without a browser:

```console
cargo run -p axionomy-cli -- catalog
cargo run -p axionomy-cli -- describe logistics
cargo run -p axionomy-cli -- run logistics --strategy reliable
```

## Core example

```rust
use axionomy::{
    Account, EconomyBuilder, Exchange, Goal, LinearInvariant, Quantity, Rate,
    basket,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum Asset {
    Raw,
    Finished,
    Tool,
    Solved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum AccountId {
    Workshop,
    Success,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum Role {
    Shop,
    Goal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum RateId {
    Build,
    Finish,
}

let mut economy = EconomyBuilder::new()
    .account(
        AccountId::Workshop,
        Account::from(basket([(Asset::Raw, 2), (Asset::Tool, 1)])),
    )
    .account(AccountId::Success, Account::default())
    .rate(
        RateId::Build,
        Rate::new()
            .consume(Role::Shop, basket([(Asset::Raw, 2)]))
            .preserve(Role::Shop, basket([(Asset::Tool, 1)]))
            .produce(Role::Shop, basket([(Asset::Finished, 1)])),
    )
    .rate(
        RateId::Finish,
        Rate::new()
            .preserve(Role::Shop, basket([(Asset::Finished, 1)]))
            .produce(Role::Goal, basket([(Asset::Solved, 1)]))
            .distinct(Role::Shop, Role::Goal),
    )
    .invariant(
        LinearInvariant::new("material")
            .weight(Asset::Raw, 1)
            .weight(Asset::Finished, 2),
    )
    .build()?;

let build = Exchange::new(RateId::Build, Quantity::new(1))
    .bind(Role::Shop, AccountId::Workshop);
let assessment = economy.assess(&build);
assert!(assessment.is_applicable());
assert_eq!(
    assessment
        .projected_deltas()
        .expect("applicable exchanges project their receipt deltas")
        .len(),
    1,
);
economy.apply(build)?;
economy.apply(
    Exchange::new(RateId::Finish, Quantity::new(1))
        .bind(Role::Shop, AccountId::Workshop)
        .bind(Role::Goal, AccountId::Success),
)?;

let goal = Goal::new().require(
    AccountId::Success,
    basket([(Asset::Solved, 1)]),
);
assert!(economy.matches(&goal));
# Ok::<(), Box<dyn std::error::Error>>(())
```

The `Build` exchange is valid only if every role binding, balance,
preservation requirement, arithmetic operation, and invariant succeeds.
`assess` explains validity without mutation: feasible exchanges expose the
same per-account deltas later confirmed by their receipt, while infeasible
exchanges expose every account-and-asset shortfall. The core prepares all
affected accounts first and commits all of them together.

## Exact values and typed authoring

An asset defines what a value means and names its atomic economic basis;
`Quantity<N>` defines how its exact non-negative coefficient is represented.
The default backend is `u64`, while the `bigint` feature supports exact
non-`Copy` `BigUint` economies without changing model semantics.

Unit-aware models use `UnitAsset<Id>` as their actual asset key. An
`AssetSchema` declares each logical ID once as discrete or measured, and the
key carries the physical dimension, stable `uom` quantity kind, and exact
atomic basis through equality, hashing, ordering, and Serde. Inputs may be
expressed in any compatible unit, but every account, basket, rate, invariant,
and goal stores the same canonical atoms. Conflicting definitions cannot
silently alias and the schema-backed model build rejects them. Jiff uses this
same representation, so calendar and physical-time authoring cannot create
competing clocks outside the economy.

The typed `uom` and Jiff adapters currently require the resulting atom count to
fit `u64` before converting it into the economy's selected `Quantity<N>`
backend. `BigUint` removes the core economy's arithmetic ceiling, but it does
not expand this authoring-adapter conversion boundary.

```console
cargo run --features bigint --example bigint
cargo run -p axionomy-units --example cargo_mass
cargo run -p axionomy-time --example calendar_window
```

Model types serialize directly through Serde and preserve stable insertion
order for readable, repeatable output. Deserialization passes through the same
canonical construction rules: zero balances are removed and duplicate
identifiers are rejected. This ordinary serialization is intentionally not
claimed as a versioned compatibility protocol. The MCP reference server hashes
the exact current serialized bytes to identify immutable snapshots inside its
store; that deployment handle is not a promise that future schema versions
will produce the same ID.

## Stateless MCP reference server

`axionomy-mcp` demonstrates how a remote, interruptible Axionomy integration
can remain faithful to the closed model. It keeps no hidden “current economy.”
`axionomy_economy_put` returns an immutable `economy_id`; assessment names that
snapshot, while apply and replay return a new ID and leave the source
available. `axionomy_search` accepts an explicit candidate exchange universe
and returns an MCP task handle whose progress, terminal result, and cooperative
cancellation are observable for the lifetime configured by the running server.

Snapshot lifetime is caller policy. `AxionomyMcp<S>` accepts any
`SnapshotStore`; the reference binary uses `MemorySnapshotStore`, whose clones
share immutable content-addressed snapshots for one process. A restarted
reference server intentionally starts empty. Applications that genuinely need
longer-lived handles can provide database or object-store implementations
without changing the MCP tools or the core engine.

The server accepts only MCP `2026-07-28`, disables legacy sessions, and
requires the revision's request metadata and standard HTTP routing headers.
Task state, queues, progress counters, and snapshot handles are operational
artifacts—not a parallel world model. Search still reaches successors only by
applying concrete exchanges through the kernel, and its completed result is a
replayable trace.

```console
cargo run -p axionomy-mcp --bin axionomy-mcp

# Full stateless HTTP lifecycle, including task polling:
cargo test -p axionomy-mcp --test http
```

See [crates/axionomy-mcp/README.md](crates/axionomy-mcp/README.md) for the tool
contract, deployment limits, and integration details.

## Conformance problems

| Problem | Encoded concepts | Compared strategies |
| --- | --- | --- |
| Key-door maze | Topology, position, lock, key, energy, time, heuristic, goal | BFS, Dijkstra, A*, exact energy/time Pareto front |
| Sokoban | Cell occupancy, push legality, deadlock | BFS and infeasibility |
| Exact cover | Universe, subsets, coverage, selection | BFS and Algorithm X |
| Workshop | Recipes, catalysts, material, labor, waste, process time | BFS, waste minimization, exact waste/time Pareto front |
| Job shop | Precedence, identified machine slots, makespan, per-job completion | Best-first, branch search, direct oracle, exact allocation Pareto front |
| Rescue | Hidden truth, seed, observation, belief, chance, sensor use | Exact scenario evaluation, seeded Monte Carlo, approximate policy front |
| Bridge | Capacity, bids, escrow, priority, joint resolution | BFS, first-come, auction, exact priority/credit Pareto front |
| Marketplace | Buyers, sellers, carriers, tax, commission, competing orders, participant utility | Assessment matching, replayable clearing, exact allocation Pareto front |
| Logistics | Orders, routes, fuel, time, weather, breakdown, repair | Long rollouts, risk-aware Monte Carlo, route MCTS, approximate policy front |
| Connect Four | Identified cells, gravity, turns, line counts, wins, draw | Vector-valued MCTS with plain-board/minimax oracles |
| Mission | Private views, caller-owned beliefs, causal shared intelligence, hazard, treatment | Repeated observation-scoped ISMCTS, scenario/MC evaluation, approximate policy front, RL trajectories |
| Perishables | Fungible cohort claims, unique condition facts, deadlines, refrigeration, power loss, cooling energy | Receipt-maintained index, event agenda, exact storage Pareto front, independent oracle |

Every accepted result is an exchange trace replayed by the same core. The
specialized algorithms are proposers, not alternate execution engines.

These are concrete examples and conformance fixtures, not reusable maze,
scheduler, marketplace, or game frameworks. Users create their own economies.
The suite's purpose is to prove that the common assets/accounts/rates/exchange
axioms generalize and to expose friction that belongs in domain-independent
engine APIs. Rate names never authorize bindings: where identity matters, the
rates require preserved identity or capability assets, and adversarial tests
construct incorrect bindings directly to prove the core rejects them.

Every problem has a small consumer-facing example. The example binaries contain
no domain rules: they instantiate their concrete economy, invoke strategies,
replay the proposed trace, assert the encoded goal, and print a summary.

```console
cargo run -p axionomy-problems --example maze
cargo run -p axionomy-problems --example sokoban
cargo run -p axionomy-problems --example exact_cover
cargo run -p axionomy-problems --example workshop
cargo run -p axionomy-problems --example scheduling
cargo run -p axionomy-problems --example rescue
cargo run -p axionomy-problems --example bridge
cargo run -p axionomy-problems --example marketplace
cargo run -p axionomy-problems --example logistics
cargo run -p axionomy-problems --example connect_four
cargo run -p axionomy-problems --example mission
cargo run -p axionomy-problems --example perishables
```

All runnable examples use structured console logging. `INFO` presents model
construction, strategy decisions, encoded outcomes, and verification;
`RUST_LOG=debug` adds detailed traces, assessments, or canonical denomination
definitions as appropriate. Each example binary installs its own subscriber.
The reusable libraries never install a global subscriber or treat logs as
authoritative state.

A repeatable long-horizon workload is also available:

```console
cargo bench -p axionomy-problems --bench rollout_throughput
```

For example, the maze's default `INFO` view is:

```text
INFO Compare shortest-depth BFS with energy-aware A* over one encoded graph. example="Maze"
INFO encoded economy ready accounts=2 rates=8
INFO proposal replayed strategy="BFS" exchanges=3 expanded=5 goal_verified=true
INFO proposal replayed strategy="A*" exchanges=6 expanded=6 energy=6 goal_verified=true
INFO exact search retained every non-dominated route completeness=Exact outcomes=2 terminal_outcomes=2 expanded=10
INFO non-dominated route energy=9 time=3 exchanges=3 replay_verified=true
INFO non-dominated route energy=6 time=6 exchanges=6 replay_verified=true
```

## Semantics in brief

For exchange units `n`, consume and produce baskets scale by `n`. A preserved
basket is an unscaled read threshold: it must exist, but it is not consumed.
This makes a batched exchange observationally consistent with sequential
firings that reuse the same catalyst or fact.

For every affected account:

```text
required = consume × n + preserve
next     = current - consume × n + produce × n
```

All quantities are checked. Every declared linear invariant must have the same
measure before and after. A failure leaves the economy unchanged.

## Development

Axionomy uses Rust Edition 2024 and supports Rust 1.89 or newer. It relies on
mature ecosystem crates for ordered maps, serialization, exact numeric traits,
typed units, civil time, randomness, statistics, pathfinding, errors, property
testing, benchmarking, and structured example diagnostics while keeping
semantic authority in the four core primitives.

```console
cargo test --workspace --all-targets --all-features
cargo test --workspace --doc --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps
cargo bench --workspace --no-run
cargo package -p axionomy
cargo package -p axionomy-search --list
cargo package -p axionomy-units --list
cargo package -p axionomy-time --list
cargo package -p axionomy-problems --list
cargo package -p axionomy-mcp --list
```

Until `axionomy` is available from a registry, Cargo cannot perform an
ordinary registry-resolution dry run for the dependent packages. CI can
verify their package archives using local `[patch.crates-io]` overrides; their
source manifests retain both path and version requirements for normal
workspace development and future ordered publication.
