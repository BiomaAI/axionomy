# Axionomy

## Closed Economic State Machine — Product Design Document

| Field | Value |
| --- | --- |
| Status | Implemented foundation; living design |
| Product | Axionomy Cargo workspace |
| Version | `0.1.0` |
| Rust edition | 2024 |
| Minimum Rust | 1.89 |
| Last updated | 2026-08-06 |

## 1. Product thesis

Axionomy is a closed computational engine in which every authoritative part of
a problem is represented by assets, accounts, rates, and exchanges.

The fundamental axiom is:

> Nothing semantically authoritative may live outside assets, accounts, rates,
> and exchanges.

This is stronger than saying an application has a ledger. The economic state
is not one subsystem beside a graph, simulator, scheduler, lifecycle object,
or hidden world. It is the world.

Position, connectivity, time, energy, permissions, machine capacity, goals,
agent memory, hidden truth, random state, bids, rewards, and terminal
conditions all use the same representation. After initialization, every real
state change is an exchange.

Algorithms remain welcome:

- BFS, Dijkstra, and A* may search.
- Algorithm X, CP-SAT, MILP, or another OR tool may optimize.
- Monte Carlo and MCTS may simulate.
- Policies, reinforcement learning, and LLMs may decide.

But they have one role:

> Solvers propose. The closed economy defines and validates reality.

The accepted answer is never a solver's private assignment. It is a trace of
exchanges that the core can replay from the initial economy.

## 2. Why closure matters

Conventional systems split one problem across representations: a graph owns
connectivity, an object owns position, a scheduler owns capacity, a callback
owns the goal, and a simulator owns time and chance. Semantic drift appears at
the boundaries. A solver can optimize a constraint execution does not enforce,
or a callback can mutate state that an event log cannot reconstruct.

Closure creates one validation boundary and one audit language.

A closed engine should also explain rejection in the same language that
defines validity. Given a proposed exchange, Axionomy should derive every
affected account requirement, every missing asset, and every projected effect
without mutating state. This turns validation from a binary gate into an
economic explanation while keeping the explanation grounded entirely in the
current accounts, assets, rate, and exchange bindings.

A closed encoding must also be structured. Serializing an entire external
world into an opaque `WorldBlob` asset and transforming it with an arbitrary
callback would satisfy the vocabulary but violate the purpose. Useful
encodings are:

- Local: a rate touches a comprehensible part of state.
- Composable: subsystems interact through explicit shared assets.
- Inspectable: preconditions and effects are visible in the rate.
- Replayable: the exchange trace reconstructs state.
- Solver-neutral: one encoding serves multiple strategies.
- Verifiable: the core can reject an invalid proposal independently.

## 3. The four primitives

| Primitive | General meaning |
| --- | --- |
| Asset | A resource, fact, proposition, capability, condition, permission, observation, memory item, or state token |
| Account | An owner, actor, place, scope, or namespace holding assets |
| Rate | A law specifying which local multisets may become which others |
| Exchange | A concrete binding and firing of one rate |

A useful mnemonic is:

- Assets are the nouns.
- Accounts answer “where?” and “whose?”
- Rates are the laws or verbs.
- Exchanges are the events.

`Quantity`, `Basket`, `Goal`, `ExchangeAssessment`, `Receipt`, and `Trace`
support these primitives; they do not introduce parallel semantic worlds.

Users define ordinary Rust types for assets, account IDs, rate IDs, and roles.
That is an open vocabulary, not an authorization to keep authoritative
instances elsewhere. Reference ontologies live in the separate
`axionomy-problems` crate. No one of them is built into the kernel.

### 3.1 Values have three independent axes

Axionomy separates three questions that are often collapsed into one number:

```text
Asset A / UnitAsset<Id> = what the value means and its atomic economic basis
Quantity<N = u64>       = how its exact non-negative coefficient is represented
Typed binding           = how a physical or calendar value is validated and lowered
```

For example, a `uom` mass value may be converted exactly into a canonical
`Cargo` unit asset and a `Quantity<u64>`. A unit-aware asset key carries its
logical ID, physical dimension, stable quantity kind, and exact atomic basis.
The physical dimension is checked at the authoring boundary, the asset
preserves the denomination and economic identity, and the quantity supplies
the coefficient. A heterogeneous basket therefore does not erase meaning
merely because it stores one numeric backend.

Units are never properties of accounts or baskets. One account may author a
cargo balance in kilograms and another in grams, but both inputs lower through
the same schema-issued handle into the same canonical asset atoms. If two
denominations are economically distinct—such as loose grams and sealed
25-kilogram bags—they are different logical assets connected by an explicit
rate rather than alternate representations of one balance.

One economy uses one numeric backend `N`. This gives every balance, rate,
exchange, shortfall, receipt, and invariant one coherent arithmetic contract.
Using a different numeric representation for every asset is deliberately out
of scope: it would introduce dynamic value dispatch into the authoritative
state and make conservation and generic algorithms substantially less clear.

### 3.2 Fungibility is an identity relation

Axionomy does not need separate fungible-token and non-fungible-token storage
engines. Units with the same asset identity are interchangeable and aggregate
into one quantity. A genuinely unique item receives a unique asset identity,
starts with quantity one, and has its supply or lifecycle protected by the
closed rate book and declared invariants. The kernel deliberately does not
assume that every fact-like asset must have quantity one; capacities,
permissions, and observations may legitimately have greater multiplicity.

Shared fate can be encoded without rewriting every holder. Accounts may own
fungible claims or shares while a cohort, pool, or protocol account owns one
condition, epoch, conversion, or supply fact. Rates that use a claim preserve
the shared fact. Changing that fact once therefore changes which exchanges are
available to every claim holder. This is semantic indirection represented in
accounts and assets, not an external cache.

Secondary holdings, aggregate-supply, rate-dependency, and event indexes are a
different kind of index. They are disposable projections rebuilt from the
economy and updated from receipts. They may decide where a simulator looks
next, but every candidate must still pass core assessment; losing or rebuilding
an index cannot change transition validity.

## 4. Formal state and transition model

### 4.1 State

The complete current state is a finite sparse mapping:

```text
S : Account × Asset → Quantity<N>
```

`N` is an exact, totally ordered scalar representation and defaults to `u64`.
Quantities are non-negative even when a future scalar backend can also
represent negative values. Signed values exist only as derived invariant
measurements, never as account balances. Missing entries have quantity zero,
and zero entries are canonicalized away.

The complete problem definition is:

```text
Problem = Initial state + rate book + declared invariants
```

The rate book is immutable after construction in the current design. It is
part of the closed problem definition, not an external solver table.

### 4.2 Rate

For every account role, a rate can declare three baskets:

```text
C(role) = assets consumed
P(role) = assets produced
R(role) = assets required and preserved
```

It can also declare pairs of roles that must bind to different accounts.
Repeated builder calls for one role merge their baskets.

`preserve` represents a read arc, catalyst, fact, permission, or condition. It
does not require the awkward consume-then-reproduce convention.

### 4.3 Exchange

An exchange contains:

```text
Exchange = (rate_id, account-role bindings, units)
```

All roles mentioned by the rate must be bound. Unknown bindings are rejected.
Every bound account must exist, and distinct-role declarations must hold.

For units `n > 0`, the effect accumulated for one concrete account is:

```text
required = C × n + R
next     = current - C × n + P × n
```

Consume and produce scale because they are per-unit transformations.
Preserved baskets do not scale: they are read thresholds. This means a
catalyst or immutable fact can support a batch just as it can support
equivalent sequential firings.

When multiple roles bind to one account, their effects are merged before any
balance is checked. This prevents role aliasing from creating order-dependent
behavior. Rates can prohibit aliases with `distinct`.

### 4.4 Atomic application

Application has a prepare-and-commit boundary:

1. Validate units, rate ID, bindings, distinctness, and account existence.
2. Scale and merge all effects with checked arithmetic.
3. Check complete consume-plus-preserve requirements.
4. Apply withdrawals and deposits to cloned accounts.
5. Measure every declared invariant before and after.
6. Commit all cloned accounts together.
7. Return a receipt containing the bound exchange and every account delta.

Any error leaves the stored economy unchanged.

### 4.5 Assessment and distance to feasibility

Assessment is a read-only projection of one exchange against one economy
snapshot. It does not introduce a fifth primitive or an alternative execution
path. It derives information from the same rate resolution, role binding,
effect merging, checked arithmetic, balance requirements, and invariants used
by application.

For every affected account:

```text
required(account)  = consumed × units + preserved
shortfall(account) = max(required - current, 0), per asset
projected(account) = current - consumed × units + produced × units
```

The complete distance to feasibility is the sparse vector:

```text
Distance = { (account, asset, missing_quantity) }
```

Affected accounts and shortfalls use deterministic account ordering.
`Basket::iter_sorted` provides deterministic asset ordering when a consumer
materializes the vector for learning, serialization, or comparison.

An assessment must accumulate shortfalls across every affected account rather
than stop at the first deficient account. It must distinguish a structurally
invalid proposal—such as a missing rate, account, or role binding—from a
well-formed exchange that is currently infeasible because assets are missing.
Overflow and invariant failures remain separate structured issues because
adding assets does not necessarily repair them.

A successful assessment exposes the projected per-account deltas that
application would produce. If the economy has not changed, those projected
deltas must exactly match the deltas in the eventual `Receipt`. An assessment
is evidence about a snapshot, not permission to bypass revalidation; `apply`
remains the sole commit boundary.

The core reports structured economic facts, not a universal scalar cost.
Solvers, user interfaces, planners, and reinforcement-learning policies may
weight the shortfall vector to build heuristics, rewards, or action rankings.
Those weights are disposable policy. If a cost changes validity, payment, or
state, it must instead be encoded through assets and rates.

This contract enables:

- Exact explanations of what an exchange lacks and by how much.
- Valid-action masks without speculative mutation.
- Dense reinforcement-learning observations and reward shaping.
- Search heuristics based on distance to feasibility.
- Procurement, prerequisite planning, and exchange repair.
- Preview of all account effects before atomic commitment.

### 4.6 Declared invariants

Literal conservation is correct for trade but too narrow for transformations
such as:

```text
At(A) → At(B)
Wood → Chair + Waste
Uncovered(A) → Covered(A)
AvailableSlot → ReservedSlot
```

`LinearInvariant` assigns signed integer weights to asset identities and sums
the weighted quantity across all accounts. Every exchange must preserve the
measure.

Examples:

```text
Σ position-token weights = 1
Energy + SpentEnergy = constant
Wood + Chair + Waste = constant material mass
AvailableSlot + ReservedSlot = constant capacity
```

An invariant preserves whatever measure exists in the initial state. It does
not by itself assert that the initial value is correct; problem construction
and tests remain responsible for initial validity.

### 4.7 Goals

A `Goal` is a set of required baskets at concrete accounts. Matching is
monotone: an account may own additional assets. Problems commonly use a final
rate that preserves domain completion evidence and produces `Solved` in a
success account.

This avoids arbitrary termination callbacks. Terminal state is part of the
economy.

### 4.8 Multi-objective outcome analysis

Axionomy does not require a universal scalar utility function. A caller may
project several objective values from a terminal economy and compare complete
outcomes by Pareto dominance:

```text
x dominates y
  iff x is no worse than y on every declared objective
  and x is strictly better on at least one objective
```

The values must remain economic facts: spent energy, elapsed time, waste,
completion evidence, retained credit, participant utility, preserved
inventory, or another balance produced by accepted exchanges. The ordered
objective keys and minimize/maximize directions are derived decision policy.
They decide how valid worlds are compared, but they neither authorize a
transition nor manufacture an outcome.

`axionomy-search::pareto` provides `Objective`, `ObjectiveVector`, four-way
`Dominance`, and `ParetoFront`. Objective vectors fail on duplicate keys,
schema drift, or unordered values. A front is explicitly `Exact` or
`Approximate`; equal vectors retain one representative artifact.

`ParetoSearchSession` exhausts a finite reachable state space and stores a
replayable trace with every retained terminal outcome. It deliberately does
not prune an intermediate state merely because its current objective balances
look worse: later exchange availability and resources may reverse that
comparison. The front becomes exact only after all reachable states are
exhausted. Interruption leaves a resumable session and an approximate
best-known front.

Monte Carlo policy evaluation can use the same dominance relation over means,
probabilities, or other summaries derived from encoded rollout outcomes. Such
a front remains approximate because the estimates are sampled, even when the
sampling work itself completes. Exact finite-support scenario enumeration and
sampled estimation must keep these different epistemic claims visible.

The product gain is deferred scalarization. Callers can inspect allocation and
resource tradeoffs, retain discrete or non-convex alternatives that a weighted
sum can miss, and provide a learner with a denser set of viable outcome targets
without moving reward truth into a solver callback. Choosing one point from
the front remains caller policy.

## 5. Core API and ownership boundary

| Type or operation | Responsibility |
| --- | --- |
| `Quantity<N = u64>` | Checked non-negative coefficient with an exact numeric backend |
| `AssetAmount<A, N = u64>` | One asset-qualified quantity at an API boundary |
| `Basket<A, N = u64>` | Sparse heterogeneous asset multiset with one numeric backend |
| `Account<A, N = u64>` | Balance container used during initialization and internal commits |
| `Rate<Role, A, N = u64>` | Multi-role consume/produce/preserve law |
| `Exchange<RateId, Role, AccountId, N = u64>` | Concrete proposal |
| `EconomyBuilder` | Initial problem construction |
| `Economy` | Private account/rate ownership and sole execution authority |
| `ExchangeAssessment` | Applicable, infeasible, or invalid explanatory projection |
| `AccountAssessment` | Required, available, consumed, preserved, and produced assets for one affected account |
| `AccountShortfall` | Exact missing-asset basket for one affected account |
| `ApplyError` | Structured rejection reason |
| `Receipt` | Accepted exchange plus per-account deltas |
| `Trace` | Ordered replayable exchange sequence |
| `Goal` | Required terminal asset configuration |
| `EconomicView` | Account-restricted, immutable projection |

Search-owned `ObjectiveVector`, `ParetoFront`, and `ParetoSearchSession` are
not core primitives. They compare and explore economies through the public API
and return replayable evidence; the `axionomy` kernel does not depend on them.

`Economy` exposes immutable account and rate inspection. It does not expose a
mutable account reference. Semantic mutation occurs only through `apply` or
`replay`.

Search and simulation use:

- `fork` to create an isolated branch.
- `simulate` to apply one exchange on a branch.
- `replayed` to validate a full trace without touching its source.
- `state_key` for a canonicalized logical-state key.
- `assess` to explain one proposal and preview feasible deltas.
- `is_applicable` to derive one boolean decision from assessment.
- `applicable` to filter concrete proposals through core validation.

`assess` returns a structured `ExchangeAssessment` with all account
requirements, complete shortfalls, projected deltas, and non-balance issues
without mutation. `is_applicable`, `applicable`, assessment, and application
share one internal analysis path so their semantics cannot drift.

`replay` mutates its target one exchange at a time. If a caller needs
all-or-nothing validation of an entire trace relative to an existing economy,
it uses `replayed`; the source remains unchanged.

### 5.1 Construction, ordering, and serialization

Model construction and deserialization share one canonical boundary.
`EconomyBuilder::build` returns structured issues rather than silently
replacing duplicate account or rate identifiers. Baskets likewise reject
duplicate asset identifiers, remove zero quantities, and validate the selected
numeric backend. Invalid serialized input cannot construct a looser economy
than ordinary Rust input.

Semantic maps use `IndexMap`: iteration, diagnostics, and direct Serde output
preserve stable insertion order without requiring every user ontology to
implement serialization-specific ordering. When semantic identity must ignore
construction order, `state_key` explicitly sorts account, asset, and quantity
tuples; `state_fingerprint` hashes that canonical logical key for compact
model-scoped caches. Neither stable presentation nor a 64-bit fingerprint is a
substitute for validation, trace replay, collision-safe durable identity, or a
future compatibility protocol.

The MCP reference adapter's default `MemorySnapshotStore` adds one scoped
identity policy: it hashes the exact serialized snapshot with BLAKE3 and stores
those bytes immutably. This creates a collision-resistant process-local handle
without changing `Economy` semantics. Semantically equal economies with
different declared insertion order may have different handles, and a future
serialization schema may change the hash. `AxionomyMcp<S>` accepts a
caller-provided `SnapshotStore` so retention and physical storage remain
integration policy; every implementation must preserve immutable-handle
semantics.

The current public types serialize directly. Axionomy does not maintain
parallel `ModelDefinitionV1`, snapshot, or trace-envelope types before a real
deployed compatibility requirement exists. Serde solves encoding and
decoding; it does not by itself promise schema migration. Explicit wire types
should be introduced only when their compatibility contract is known.
The MCP string-ID/`u64` profile is therefore a current reference boundary, not
an invented historical `V1` model hierarchy.

### 5.2 Numeric and typed authoring boundaries

`QuantityScalar` is intentionally narrower than a general numeric trait. It
defines the exact operations the authoritative economy needs:
non-negativity, total ordering, checked addition/subtraction/multiplication,
conversion from an atomic count, and an associated signed invariant-measure
type. The default `u64` backend is compact and ergonomic. The optional
`BigUint` backend proves that the complete model and search surfaces support
exact non-`Copy` quantities beyond `u64`; its associated invariant measure is
`BigInt`.

Physical and calendar types belong at authoring boundaries:

- `axionomy-units` uses `uom` with exact rational values to declare a measured
  `UnitAsset<Id>`, validate dimensions and unit conversion, and lower an exact
  multiple of its atomic basis into `AssetAmount<UnitAsset<Id>, N>`.
- `axionomy-time` uses Jiff to resolve timestamps, zones, daylight-saving
  transitions, and calendar spans, then lowers exact elapsed duration into the
  same schema-defined time asset representation.

`AssetSchema` is the one declaration point for unit-aware logical IDs. It
rejects repeated IDs, conflicting discrete/measured definitions, incompatible
atomic bases, unknown keys, and definitions foreign to the model. The
schema-backed economy build inspects initial accounts, every rate basket, and
invariant weights; goal validation applies the same rule. Schema-issued typed
handles accept only the `uom` dimension used at declaration.

The asset key itself remains self-describing: its definition participates in
equality, hashing, ordering, and Serde. Even keys created by independent
schemas cannot silently alias when their dimensions, quantity kinds, or
atomic bases differ. A schema can be reconstructed from the keys embedded in
a deserialized economy and will reject one logical ID carrying conflicting
definitions. The schema is therefore a constructor and validator, not a
second authoritative world that must survive for the balances to remain
meaningful.

The lowered value is still an ordinary asset plus quantity. The adapters do
not put heterogeneous physical values inside `Quantity` or create an external
clock. Raw atomic counts remain available explicitly through `atoms` or
`amount`; they cannot change the denomination carried by the asset key.

The current typed adapters convert a validated atom count through `u64` before
calling `QuantityScalar::from_u64`. A larger backend such as `BigUint` therefore
extends authoritative economy arithmetic but does not enlarge the adapters'
input range.

### 5.3 Observability boundary

Logs are derived presentation, not a fifth semantic channel. Reusable Axionomy
crates never install a global tracing subscriber and no validation, search, or
replay result depends on whether logging is enabled. Runnable binaries own
subscriber configuration and emit structured summaries of model construction,
decisions, encoded outcomes, assessments, receipts, and replay checks. Changing
`RUST_LOG` changes visibility only; it cannot change economic state or accepted
behavior.

### 5.4 Long-running computation boundary

Search control is operational state, not economic state. A queue position,
expanded-node count, Monte Carlo sample count, cancellation flag, wall-clock
deadline, or task handle may live outside the economy only because removing it
cannot change which exchanges are valid, what they do, or whether an encoded
goal has been reached.

The runtime-neutral contract is a resumable session:

```text
advance(session, WorkBudget) → status + work completed + progress snapshot
observer(progress)           → continue or interrupt at a safe boundary
```

Work budgets use deterministic algorithm units: BFS counts expanded states,
Monte Carlo counts samples, and MCTS/ISMCTS count iterations. They are not
economic quantities and do not impersonate encoded time or cost. An
interrupted session remains valid and can be advanced again. For fixed inputs
and random seeds, splitting the same total work across chunks must not change
the result.

The search crate owns this contract but does not own Tokio, threads, HTTP,
logging, persistence, or a cancellation-token implementation. A caller may
therefore integrate progress and interruption into a GUI, game loop, worker,
MCP task, or another runtime without granting the adapter transition
authority.

## 6. Workspace and package boundaries

The repository makes authority boundaries visible as dependency boundaries:

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

- `axionomy` owns universal state and transition semantics.
- `axionomy-search` owns disposable action generation, search, rollout,
  sampling, Monte Carlo, perfect-information MCTS, information-set MCTS, and
  learning projections.
- `axionomy-problems` owns domain ontologies, problem constructors,
  specialized proposers, and conformance tests.
- `axionomy-units` owns optional dimension-safe physical authoring.
- `axionomy-time` owns optional calendar-aware timeline authoring.
- `axionomy-mcp` owns the strict stateless MCP reference boundary, immutable
  snapshot-store contract, and process-local task orchestration.
- `axionomy-view` owns runtime-neutral monomorphic presentation contracts and
  replay-derived frames, without owning a domain ontology or runtime.
- `axionomy-service` owns the interface-neutral problem catalog, reproducible
  run requests, cooperative controls, progress, and complete artifacts.
- `axionomy-cli` owns human- and script-oriented command parsing and output.
- `axionomy-studio-server` owns native HTTP, OpenAPI, SSE, in-memory run
  lifecycle, static export, and hosting for the browser application.

The kernel must never depend on a solver or problem crate. Moving BFS and A*
out of the kernel is philosophically significant: algorithms explore the
machine but do not define it. Moving reference problems out prevents their
asset types from becoming privileged engine concepts.

The problem crate depends only on public APIs. It therefore tests not only
correctness but whether downstream model authors can express the intended
domains without kernel access.

The implementation prefers focused, mature ecosystem crates over local
reinvention: `indexmap` for stable maps, Serde for model encoding, `num-traits`
and `num-bigint` for exact numeric backends, `thiserror` for structured errors,
`uom` and Jiff for authoring, `rand`/`rand_chacha` for reproducible sampling,
`statrs` for statistical estimators, `pathfinding` for standard implicit graph
search, `tracing`/`tracing-subscriber` for example observability, `proptest` for
laws, and Criterion for benchmarks. `petgraph` is not a kernel dependency
because Axionomy does not own an authoritative graph and its successors are
generated implicitly from applicable exchanges. It remains a reasonable future
adapter when a user needs explicit graph import, export, or analysis.

The MCP adapter depends inward on the shared service, search, and kernel.
Neither reusable crate depends on rmcp, Tokio, Axum, Clap, a browser, or a
database. This direction is essential: interface requirements may improve the
generic progress, explanation, and interruption contracts, but they cannot
make a protocol task, UI selection, or storage record part of economic truth.

### 6.1 Interfaces pressure one application boundary

CLI, HTTP/Studio, and MCP use one interface-neutral service rather than
reimplementing problem dispatch. Its public vocabulary is intentionally small:

```text
ProblemDescriptor = identity + family + instance profiles + strategies + capabilities
RunRequest         = problem + instance + strategy + deterministic seed + work budget
RunControl         = cooperative pause + resume + cancel
ServiceProgress    = ordered phase + completed/total work + message
RunObserver        = progress + solver observation + replay-verified frame
RunArtifact        = request + resolved instance + selected document + replayable alternatives
                     + assessed constraint probes
```

The service contains no HTTP, async runtime, CLI parser, MCP types, database,
or browser state. Every canonical problem has one adapter here that translates
its user-defined ontology and solver evidence into `axionomy-view`; no problem
gets a privileged viewer module. A synchronous caller may use `run`, while a
runtime calls `run_with` and owns the thread, task, scheduling policy, and
`RunControl` lifetime.

Different interfaces create useful pressure. CLI requires discoverability,
stable machine-readable output, and useful defaults. HTTP requires resumable
event streams, lifecycle control, pagination, and generated schemas. MCP
requires explicit task semantics and cancellation. Studio requires
alternatives, explanations, scrubbing, partial observations, and domain-aware
projections. These demands may change the service and view contracts. They
move into the kernel only when they are transport-neutral parts of economic
validation, assessment, application, or replay.

Semantic parity is executable: CLI JSON, HTTP artifacts, and MCP task results
are compared with the complete `ReferenceService` artifact, not merely a
shared identifier. Transport events and run records are allowed to differ;
economic documents, assessments, receipts, objectives, and traces are not.

### 6.2 Instance profiles keep examples honest

The reference service separates correctness scale from product-demonstration
scale with three explicit problem profiles:

```text
Micro     = compact exact fixture, independent oracle, fast integration proof
Showcase  = decision-dense default for Studio and portable artifacts
Stress    = larger domain-relevant workload for scale and interruption checks
```

This is not a global size knob. Each problem chooses the dimension that
actually pressures its semantics: topology and route tradeoffs for Maze,
spatial maneuvering for Sokoban, incidence alternatives for Exact Cover,
precedence and capacity for scheduling, repeated allocations for Bridge,
coupled settlements for Marketplace, hidden scenario support for Rescue,
standard board geometry for Connect Four, sample work for stochastic search,
and fungible inventory magnitude for Perishables. Showcase growth must add
branching, constraints, roles, uncertainty, or tradeoffs—not merely repeat a
forced exchange.

Every advertised Stress profile must differ observably from Showcase. For a
deterministic problem that means a larger closed economy, horizon, or coupled
state space; for a stochastic problem it may mean a larger encoded scenario
set or a higher enforced work budget. A differently named request that builds
the same economy and performs the same work is a product defect. Service-level
regressions compare deterministic model structure, while problem-level tests
verify each intended structural, scenario, or work-budget dimension directly.
Maze expands topology, Exact Cover expands its incidence matrix, Bridge adds a
bounded allocation round, Marketplace couples six orders, and Rescue expands
to 72 hidden scenarios; the other seven profiles retain their board, horizon,
target, inventory, or sampling pressure.

Profiles remain closed problem definitions. Their accounts, assets, rates,
invariants, goals, and initial quantities are ordinary model data; the
`InstanceDescriptor` is interface identity, not semantic authority. Every
artifact resolves and records that identity. Micro continues to support exact
oracles even when exhaustive search over Showcase would be inappropriate, and
an approximate or candidate-bounded Showcase frontier must be labeled as such.

The conformance service enforces minimum Showcase pressure for all thirteen
problems. Every result also receives transport-neutral **Model size** evidence:
accounts, rules, steps in the trace, rule-check probes, and alternatives. These
measurements are explanatory, not objectives, and cannot affect validity.
Studio uses them with a generic
outcome-comparison table so API awkwardness and accidental toy regressions are
visible early across unrelated domains.

### 6.3 Presentation is a replay-derived boundary

Axionomy Studio is designed as an economic debugger and decision observatory,
not as a second environment. Its universal view shows accounts and balances,
exchange bindings, non-mutating assessments, projected effects, successful
receipt deltas, trace position, search lifecycle, and decision analyses. A
domain may additionally supply a graph, grid, matrix, or timeline scene, but
that scene is a read-only projection of the same economy snapshot. Removing
the projection must make the display less convenient without changing which
exchanges are valid, what they do, or whether a trace reaches its goal.

The browser boundary is deliberately monomorphic. Arbitrary user types such
as `Economy<MyAccount, MyAsset, MyRate, MyRole>` cannot be known statically by
a TypeScript build, so the Rust adapter lowers them into:

```text
ViewId        = stable presentation key + human label + optional JSON context
ExactQuantity = decimal text
ViewSnapshot  = ordered accounts and balances + optional derived scene
                + replay-derived leaderboards
Scene         = terrain/topology surface + stable role-bearing semantic entities
                + typed economic evidence links
                + paths + annotations + metrics
Leaderboard   = direction + ordered participants + exact score + rank evidence
ExchangeFrame = before + assessment + exchange + receipt + after + cues
ViewDocument  = metadata + model + initial snapshot + replay frames
                + proposals + objectives + Pareto fronts + telemetry
                + actor-relative observations + retained solver observations
```

`ViewId` does not replace the ontology value and its optional JSON is not an
authority channel. Exact quantities cross JSON as strings because JSON and
JavaScript numbers cannot preserve every `u64` or wider exact integer. Charts
may convert a value for screen coordinates, but labels and tooltips retain the
exact text and no plotted approximation is fed back into the economy.

Leaderboards obey the same boundary as scenes. Score-bearing assets such as
earned value, elapsed time, waste, attempts, and completed work participate in
the mechanics and remain authoritative. Rank, dominance, eligibility, trend,
and the choice of which objective to display are disposable projections
derived again at every replay snapshot. Removing a leaderboard may make
comparison harder, but cannot change a balance, validate an exchange, or
declare a task complete. Ratios are reduced and transported as exact text; an
agent with no meaningful work remains visible but ineligible instead of
receiving a misleading first-place zero.

Stable keys and human labels are deliberately separate. Reference adapters
implement typed labels for their account, asset, rate, and role enums while
retaining the same Debug-derived keys used by artifacts and scene links.
User-defined economies keep the generic Debug-label fallback unless their
adapter supplies a presentation ontology. Labels stay outside the problem and
kernel crates, wording never becomes semantic identity, and adapters do not
parse formatted Debug strings to recover domain meaning.

There is one generated contract pipeline:

```text
Rust DTOs
  └─ Serde + Schemars
       └─ Aide OpenAPI 3.1
            └─ openapi-typescript
                 └─ typed openapi-fetch client
```

Generated OpenAPI and TypeScript declarations are committed for review and
regenerated in CI. They are never edited manually. This repository pins the
Schemars-1-native Aide line because duplicating the entire DTO graph for an old
schema-trait release would create two contract systems.

The native server exposes the canonical catalog, starts runs, reports state,
pauses, resumes, or cancels computation cooperatively, serves completed
artifacts, documents, and paginated frames, and streams tagged `StudioEvent`
values through Server-Sent Events.
The service emits one monotonic event sequence across adapter phases.
`FrameAppended` carries the complete newly verified frame and its document
identity, so native SSE and browser Wasm can show authoritative progress while
the artifact is still being built. The event is produced by replay derivation,
not by a timer or an invented animation, and completed artifacts retain the
same frames for later inspection.
Logistics, Connect Four, and Mission advance their resumable Monte Carlo,
MCTS, and ISMCTS sessions in bounded work chunks, publishing phase-local
samples, iterations, nodes, and moves while observing pause or cancellation at
each chunk boundary. Adapters whose domain algorithms are still indivisible
observe control between their larger phases.
Event sequence numbers order transport observations only; they are not encoded
time. Run records and documents are process-local reference state, just like a
search queue. They do not survive restart and cannot authorize an exchange.
Applications that need persistence may store portable `RunArtifact` JSON or
build their own outer run service without changing the core.

`SearchObservationView` is the presentation lowering of disposable solver
state. It records phase, algorithm, evidence kind, bounded progress, and exact
metrics without turning a queue, rollout tree, frontier, or belief cache into
economic truth. The service sends observations through a bounded observer
channel and retains at most its configured history in every completed
document. Live execution and a saved artifact therefore expose the same
evidence surface; transport speed does not decide whether a run is explainable.

The React application uses one `EngineClient` boundary for native HTTP/SSE,
the browser Worker, and static playback. The native connection badge is backed
by a periodic health check; a catalog loaded before a server stopped cannot
claim that the engine is still connected. The browser adapter runs
`axionomy-service` compiled to Rust/Wasm inside a dedicated Worker, and static
playback advertises that it cannot execute. Local playback position and play/pause state
stay in a component reducer/state boundary rather than becoming server or
economic state. React Flow renders graph pictures; Apache ECharts renders
tradeoff views; specialized grids remain ordinary React/SVG until measured
scale requires Canvas. TanStack Query owns server-cache behavior. Vite,
Vitest, and Playwright provide build, component-contract, and real-server
browser verification. Tabler supplies the curated implementation of semantic
glyph keys, but the portable contract contains neither React component names
nor arbitrary SVG.

Graph and grid motion are composed generically from stable entity identity,
adjacent scene anchors, typed account/balance evidence, and the replayed receipt. The
browser therefore does not contain branches for a maze explorer, vehicle,
robot, package, or workshop material. The same compositor turns an anchor
change into travel, entity appearance/disappearance into entry/exit, exact
receipt deltas into consumed/produced/preserved emphasis, and path status into
route flow. A small presentation-only role vocabulary removes visual
ambiguity without adding domain semantics: `structure` defines topology,
`occupant` docks to or travels through it, `attachment` is grouped and tethered
to its owner, `state` is integrated into the structure it describes, and
`context` is separated as scenario-wide influence. Playback buttons and
autoplay declare adjacent-step intent so
semantic movement remains visible; scrubber jumps declare seek intent and do
not invent long travel across skipped exchanges. Stable canvas geometry and
higher-order composition prevent related records from becoming an unstructured
cloud or jumping between frames. A grid cell describes terrain and optionally
links to its authoritative account; occupants are explicit entities whose IDs
survive anchor changes. The playback control offers System, Full, and Reduced
motion modes, so accessibility defaults are respected while explanatory motion
can still be requested deliberately. Reduced motion removes decorative
repetition without hiding the essential state transition.

Solve evidence and trace playback are deliberately distinct controls. `Run`
creates a new artifact; the transport controls only replay its accepted
exchanges. The Solve surface exposes live or retained phase, rollout, tree,
frontier, and artifact observations. An active run immediately exposes a
spinner, phase message, elapsed time, deterministic request parameters, and
the latest bounded-work counter.
Completion leaves a dismissible receipt containing duration and request
identity and visibly marks the replacement artifact, so even a sub-second run
has an inspectable result.

Every meaningful Studio selection is encoded in a static-host-compatible query
URL: problem, instance, strategy, document, solve/replay view, replay step,
leaderboard, seed, and budget. Major selections create browser history;
scrubbing replaces the current entry; `popstate` restores the corresponding
artifact and frame. Invalid identities visibly fall back to a catalog default,
and shared links never auto-run computation. The same URL therefore works
against the native server, the browser Wasm engine, and GitHub Pages.

The implemented Studio exposes the complete thirteen-problem Showcase
surface: pathfinding and networks use graphs; Sokoban and Connect Four use
grids; Exact Cover uses a constraint matrix; scheduling and perishables use
timelines; markets expose multi-party settlement and rejected shortfalls;
stochastic and partially observed domains expose telemetry, sampled policy
evidence, and actor-relative views. Every artifact includes replayable strategy
alternatives where the domain supplies them. The universal model explorer
exposes rates, roles, goals, and invariants even when a specialized
scene is absent. The shared scene vocabulary renders stable semantic entities,
typed links to their proving accounts or balances, paths, statuses, exact
metrics, annotations, and transition cues across all four geometric surfaces.
Graph adapters for Maze, Bridge, Workshop, Marketplace, Logistics, Mission,
Rescue, and Work League all expose replay-changing linked entities through one
generic motion system. A transition remains visible through its receipt-
derived cue even when before/after geometry is identical. An instance selector
makes Micro and Stress available to either executable engine, while static
fallback deliberately serves the committed Showcase artifacts.

Assessed proposals in conformance documents are rule checks. Some are
deliberately malformed or infeasible, and their rejection proves that the
roles, balances, and invariants are active. The view contract preserves a
structured issue kind plus involved role, account, asset, or rate identities;
Studio labels these as moves that should be refused and keeps them separate from
operational run and transport failures.

Portable artifacts prove offline playback for the full catalog. The native
path proves generated OpenAPI calls, resumable SSE, pause/resume/cancel hooks,
artifact and frame retrieval, and scrubbing. Browser tests prove that the same
thirteen-problem Rust service initializes, runs, streams observations, publishes
artifacts, and cancels inside an isolated Worker. The Pages build uses a
repository-relative Vite base, includes its Wasm binary and `.nojekyll`, and
deploys from `main` without a server. Specialized projections are added
only when they materially improve comprehension; the authoritative account,
assessment, receipt, and model inspectors remain available for every problem.

## 7. Solver contract

A solver may keep priority queues, visited sets, rollout trees, clause
databases, and other acceleration structures outside the economy when those
structures are fully derived and disposable. Removing them must not change
problem validity, transition effects, goals, or replay.

A solver may:

- Inspect immutable core state or a permitted view.
- Construct concrete exchange bindings.
- Ask the core which proposals are applicable.
- Request structured assessments and projected account deltas.
- Fork states and explore.
- Read objective or heuristic assets.
- Compile a bounded economy to another representation.
- Return an exchange trace.

A solver may not:

- Add an unencoded domain constraint.
- Depend on a hidden position, clock, goal, seed, or belief.
- Directly install a successor state.
- Declare an assignment accepted without core replay.

The `axionomy-search` crate contains deliberately inspectable integrations:
BFS, Dijkstra, and A* use `pathfinding` over implicit core-validated
successors; the compatibility best-first strategy remains local because it
ranks complete economic states rather than additive graph edges. Rollout
execution, weighted sampling, Monte Carlo aggregation, vector-valued MCTS,
observation-scoped ISMCTS, and RL trajectory projections build on the same
exchange boundary. The `axionomy` kernel remains an execution and validation
substrate, not an attempt to replace every mature solver.

Candidate generation is also disposable. An `ActionSource` may lazily emit
concrete exchanges from a full state or an authorized information state. The
source does not declare applicability: the receiving search algorithm filters
each proposal through the economy before traversal.

### 7.1 Rollouts are speculative economic histories

The foundational simulation abstraction is a rollout, not Monte Carlo. A
rollout owns an isolated economy branch and repeatedly:

1. Inspects the current economy or an account-restricted view.
2. Asks a controller to propose one concrete exchange.
3. Applies the proposal through the core.
4. Records the accepted exchange and receipt.
5. Stops at an encoded goal, a dead end, controller stop, rejection, or an
   algorithmic step limit.

The controller may decide what to propose, but it cannot install a successor
state. A retained rollout trace must replay from its source economy to the same
logical final state.

Domain termination and algorithm termination are different. A goal, mission
deadline, turn limit, or draw rule that belongs to the world must be encoded as
assets and rates. A rollout horizon is only a resource limit on exploration
and must be reported as `HorizonReached`, never as success or failure.

### 7.2 Chance and sampling

Domain chance is represented by weighted Nature exchanges. An algorithm may
derive a disposable `WeightedExchange` list from encoded assets, then enumerate
or sample that list. Sampling chooses a proposal; it never creates an
unencoded successor.

Domain randomness and exploration randomness have different authority:

- Weather, sensor error, damage, breakdown, or another world outcome must be
  encoded in Nature state and realized by an exchange.
- Random rollout selection, UCT tie-breaking, or another search choice may use
  an external deterministic seed because it only changes exploration.

The latter seed is reproducibility metadata, not world state. Every sampled
domain outcome still appears in the resulting trace.

Reproducible exploration uses `rand` traits and ChaCha8 rather than a
project-local random generator. Callers may provide their own compatible RNG.
Fixed seeds make a solver run repeatable; they do not turn algorithmic
randomness into authoritative state.

### 7.3 Outcomes and Monte Carlo

Monte Carlo evaluates policies by running the same generic rollout mechanism
many times. It does not define a reward model. Success, score, cost, elapsed
time, casualties, delivered orders, and other semantically meaningful
quantities must be encoded assets. An outcome reader projects those assets
into disposable statistics.

The search crate may provide standard aggregators for Bernoulli success,
scalar mean and variance, vector outcomes, quantiles, and tail risk. Changing
an aggregator or scalarization may change ranking, but cannot change exchange
validity, effects, or the observed economic outcome vector.

Standard distribution functions and estimators come from `statrs`, including
means, population variance, quantiles, and Beta-posterior credible intervals.
Axionomy-specific code retains only the projection from encoded outcomes into
those statistical inputs.

### 7.4 MCTS is a derived tree over core states

Monte Carlo tree search reuses rollout execution but adds external selection,
expansion, and backup state. Tree nodes may cache a state key, visits, value
totals, policy priors, and child exchanges. All are derived accelerators.

Every tree edge is an exchange. Expansion uses applicable core proposals,
simulation uses isolated branches, Nature nodes use encoded outcome exchanges,
and the selected live action is revalidated before commitment. Removing the
tree or transposition table may affect speed and choice quality, but cannot
alter the modeled laws.

Perfect-information MCTS and information-set MCTS are separate contracts.
Policies in partially observed problems must key decisions and trees by
authorized observations, not hidden full state.

The implemented information-set contract makes that boundary explicit:

1. An `EconomicView` produces an `ObservationKey` containing the visible
   account boundary and its canonical balances.
2. `InformationState` pairs that observation with the acting player.
3. A caller-supplied belief sampler receives only the root information state
   and returns a possible closed economy derived from encoded priors and
   beliefs.
4. Each sampled economy must reproduce the root information state or search
   rejects it.
5. Decision sources and rollout policies are passed information states and
   already filtered concrete exchanges; the search API does not pass them the
   sampled full economy.
6. Nature, terminal, and outcome projections may inspect the sampled closed
   economy because they represent environment simulation rather than agent
   knowledge.
7. Tree transpositions merge equal information states, and the final exchange
   is revalidated against the live economy.

The belief sampler is not a second world model. Each determinization is an
ordinary economy containing encoded hidden truth, and any meaningful prior or
belief weight must come from economic assets. Sampling strategy and tree
statistics remain derived, disposable policy.

This is an API information-flow contract, not a security sandbox. Rust callers
could deliberately capture a full economy inside a closure; the conformance
suite proves correct use of the public surfaces, while capability-enforced
proposal authority remains separate future work.

### 7.5 Learning interfaces are projections

An RL adapter may expose an observation, applicable-action mask, assessment
features, receipt deltas, encoded outcome vector, and termination reason. A
learned policy may return an action distribution or value estimate. These are
proposal and training surfaces, not a second environment.

Reward shaping may weight encoded progress and shortfall facts. If a fact
changes terminal truth, payment, resource accounting, or transition validity,
it must be represented in the economy rather than supplied only by the
adapter.

### 7.6 Stateless remote execution

`axionomy-mcp` is the reference implementation of a remote boundary. It
accepts only MCP `2026-07-28`, disables legacy sessions, requires per-request
protocol and client metadata, and uses the revision's standardized HTTP
routing headers. Transport statelessness is necessary but not sufficient, so
the tool model also forbids an ambient current economy.

Every operation names an immutable `economy_id`. The default
`MemorySnapshotStore` derives that ID from a BLAKE3 hash of the exact current
Serde bytes. Assessment returns an explanation without a new snapshot;
successful apply and replay return a new content-addressed snapshot while
preserving the source. The ID is a process-local handle inside the current
reference schema, not a versioned compatibility promise or a replacement for
replay validation.

Storage lifetime is supplied by the embedding application. `SnapshotStore`
has only two responsibilities: store a complete immutable `WireEconomy` and
resolve its opaque content-derived handle. `MemorySnapshotStore` serializes
once, retains the bytes for collision checks, shares decoded snapshots through
`Arc`, and deduplicates equal bytes. Its clones share state; a fresh instance
or process restart starts empty. A database, object store, tenant boundary, or
retention policy can implement the same contract when deployment pressure
requires it without changing any tool schema or kernel rule.

The server exposes schema-backed tools for economy storage, assessment,
application, replay, and search. The reference search request contains an
explicit finite list of concrete candidate exchanges. That list is disposable
proposal policy: BFS reconsiders it at every state and the core rejects
inapplicable exchanges. The list cannot declare a successor or override a
rate. A future declarative binding language should replace large concrete
lists only when measured pressure justifies the new semantics.

Long work uses the MCP Tasks extension:

1. Validate the explicit snapshot handle and search bounds.
2. Register a working task in `rmcp::TaskManager` before returning
   `CreateTaskResult`.
3. Advance a `BfsSession` in deterministic chunks.
4. Publish human-readable progress after each chunk.
5. Observe cooperative cancellation intent between chunks.
6. Return the terminal structured `CallToolResult`, including the replayable
   solution trace, to the task manager.

Task handles, progress, terminal results, and cancellation intent live only in
the running manager. The default `rmcp` TTL bounds their retention; callers can
replace task options, including choosing unlimited process-lifetime retention.
Process restart recovery, idempotent submission, persisted checkpoints, worker
leasing, notification fan-out, authentication, and multi-tenant authorization
are deployment policies to add only when a real deployment requires them.

## 8. Partial observation and chance

Ground truth and belief belong in different accounts. An `EconomicView`
restricts which accounts a policy may inspect, and its canonical
`ObservationKey` includes both that visibility boundary and the balances visible
inside it. Two hidden worlds with the same key are one information set for that
actor.

Chance is modeled as a participant:

```text
Nature account:
  Unresolved
  ScenarioWeight(location, seed)
  Truth(location)
  Seed(n)

Instantiation exchange:
  preserve ScenarioWeight(location, n)
  consume Unresolved
  produce Truth(location)
  produce Seed(n)

Observation exchange:
  preserve Truth(location)
  consume Seed(n)
  produce Seed(n')
  produce Belief(report) for the agent
```

The prior weights and unresolved state are assets. Sampling first fires an
instantiation exchange on a fork; observation then advances the chosen seed.
Both Nature choices enter the trace, so the complete rollout replays from the
uncertain model. Systematic and seeded samplers select among weighted encoded
outcome exchanges; they never generate ambient domain state.

Agent intent and hidden resolution must be separate when one event would
otherwise force an agent to name hidden parameters. In the mission, the Scout
proposes a public `BeginScan` exchange. Nature then fires the uniquely
applicable `ResolveScan` exchange containing hidden truth and seed. The same
pattern separates joint movement from hidden encounter resolution. This keeps
agent proposal sets equal across indistinguishable worlds without moving
causal semantics outside the economy.

## 9. Simultaneous and multi-agent decisions

Joint effects are encoded as one multi-account rate. In the bridge benchmark,
auction resolution consumes both submitted bids and bridge capacity and
produces the winner's crossing right and the loser's waiting status atomically.

The marketplace benchmark extends this from joint resolution to six-party
settlement across two competing orders. Each order-specific rate transfers its
item and gross payment while splitting seller proceeds, tax, platform
commission, and shipping fees; it also consumes shipping capacity and advances
buyer, seller, and order lifecycle assets.
There is no privileged bilateral transaction hidden underneath it and no
compensating rollback protocol: either every role requirement and invariant
passes and all six accounts commit, or nothing changes.

Potential matches are ordinary exchanges built by deriving participant sets
from economy accounts. `applicable` finds exact matches, while `assess` exposes
complete shortfalls for near matches. A caller may rank those shortfalls for
procurement, search, or learned policy, but the ranking is not market truth.
Only account assets, the settlement rate, its role bindings, and the applied
exchange determine whether and how settlement occurs.

A disposable clearing search explores sequences of currently applicable
settlements and ranks resulting economies by encoded settled-order count and
gross value. It does not own an order book or commit compensating mutations;
its output is a two-exchange trace that must replay to both order goals. This
is how global compatibility may remain algorithmic while settlement law stays
economic.

Settlement rates also produce participant utility assets declared by the
market model. Exact Pareto search over completed clearings retains allocations
that differ in which buyer and seller receive those benefits. This does not
make a solver-side utility callback market truth: the benefit is an exchange
effect, while dominance only compares already-valid terminal allocations.

This is the current answer to simultaneous proposals: the resolution law must
be represented by a rate, not an external world update. Search may choose a
compatible set or sequence of those resolutions, but it cannot bypass them.

## 10. Executable conformance suite

The project now validates the thesis with thirteen deliberately different
problems. Full formal specifications live in [PROBLEMS.md](PROBLEMS.md).

| Problem | What it demonstrates |
| --- | --- |
| Key-door maze | A 14-node closed graph, explicit key/lock state, four encoded energy/time routes, exact Pareto front, and different BFS versus A* choices |
| Sokoban | A 9×7 walled warehouse, stable crate identities, atomic three-account rewrites, resumable BFS/A*, and a replayable legal deadlock |
| Exact cover | An 8-element/12-subset logical constraint surface plus Algorithm X as an untrusted proposal generator |
| Workshop | Six-chair multi-batch stoichiometric transformation, reusable catalysts, encoded waste/time front, and invariant rejection |
| Job shop | Six operations, three machines, identified capacity slots, precedence tokens, completion front, makespan, and direct-oracle agreement |
| Rescue | Four hidden sites, 32 encoded scenarios, restricted views, sensing and evacuation, seeded Monte Carlo, approximate success/resource front, and replay |
| Bridge | Repeated multi-agent bids, escrow, capacity, atomic round reset, alternative mechanisms, and fairness/credit frontier |
| Marketplace | Account-derived matching, complete shortfalls, four coupled six-party settlements, clearing, and participant-utility front |
| Logistics | Recurrent encoded chance, long rollouts, repair loops, risk projections, approximate policy front, and MCTS route planning |
| Connect Four | Standard 7×6 geometry, compact 69-line win certificates, encoded gravity and terminal truth, adversarial MCTS, and plain-board oracles |
| Mission | Canonical private observations, caller-owned posterior beliefs, repeated ISMCTS, approximate policy front, causal intelligence exchange, and RL trajectory projection |
| Perishables | Fungible cohort claims, shared condition facts, explicit time, refrigeration, exact preservation/energy front, outage effects, stale-event rejection, and an independent oracle |
| Work League | Four autonomous workers contending for 12 finite jobs across shared facilities, seeded disruptions and recovery, exact resource accounting, six per-step standings, and multiple defensible winners |

These are not unrelated demos. Together they test:

- Search, constraint satisfaction, optimization, simulation, negotiation,
  market matching, adversarial play, and learning-data generation.
- Single-, multi-, and hidden-account transitions.
- Resource consumption, fact preservation, and identity transformation.
- Feasible and infeasible instances.
- Specialized algorithms over the same core semantics.
- Replay of every accepted solution.
- Competitive outcomes where value, speed, efficiency, waste, reliability,
  and Pareto standing can select different leaders without one universal score.

They are deliberately concrete examples and conformance fixtures, not reusable
domain frameworks. Users should construct their own economies rather than
depend on an Axionomy maze, scheduler, market, or game ontology. The suite
exists to prove that the four primitives generalize and to identify recurring
domain-independent authoring or search requirements. Descriptive rate IDs are
never treated as authorization: semantic role identity is witnessed by assets,
and adversarial tests bypass trusted action helpers to verify closure.

The benchmark ontologies live in `crates/axionomy-problems`, outside the
generic kernel package. Their presence in the workspace makes architectural
pressure executable and prevents future API changes from silently narrowing
the model. Each ontology also has a minimal example binary that uses only its
public API, replays the proposed trace, and checks the encoded goal. Examples
are teaching and integration surfaces; authoritative problem semantics remain
in each concrete example economy. Those modules are importable for tests and
examples, but they are not promised as reusable domain APIs.

## 11. Current guarantees

- Asset, account, role, and rate-ID types are user-defined.
- Quantities are generic, exact, non-negative, and checked; `u64` is the
  default and optional `BigUint` demonstrates an unbounded non-`Copy` backend.
- Signed invariant measurement is associated with the selected quantity
  backend.
- Missing balances behave as zero.
- Stable maps preserve declared iteration order, while semantic state keys and
  fingerprints are independent of construction order.
- Core model, proposal, receipt, trace, and goal types serialize directly
  through Serde; deserialization rejects duplicates and canonicalizes zeroes.
- Construction and operational failures are structured `thiserror` values.
- Unit-aware keys retain logical ID, dimension, quantity kind, and atomic basis
  through equality, hashing, ordering, and serialization.
- One asset schema rejects repeated logical IDs and conflicting denominations
  across initial accounts, rates, invariants, and goals.
- Physical and calendar authoring lower exactly into the same schema-defined
  asset-qualified quantities through the optional `uom` and Jiff adapters.
- Compile-time dimension checks prevent a typed handle from accepting an
  incompatible `uom` quantity, while schema checks prevent cross-handle alias.
- Reusable crates do not install a global tracing subscriber; example binaries
  own structured presentation without making logs authoritative.
- Exchange application is atomic across all affected accounts.
- Consume, produce, and preserve effects are explicit.
- Required role bindings and role distinctness are checked.
- Failed exchanges preserve complete state.
- Declared linear invariants are checked on every firing.
- Applicable exchanges prepare only touched account contents, and linear
  invariants use weighted receipt deltas on the successful path.
- Assessment distinguishes applicable, infeasible, and invalid proposals.
- Infeasible assessment reports every affected account shortfall.
- Applicable assessment projects the exact deltas later returned by `Receipt`.
- Receipts describe accepted per-account deltas.
- Forks are isolated while sharing immutable rates, invariants, and untouched
  account contents.
- Model-scoped state fingerprints provide compact in-process cache keys.
- Trace replay uses exactly the same validation path as live execution.
- Restricted views cannot inspect omitted accounts.
- Restricted views produce canonical observation keys that include their
  account visibility boundary.
- Lazy action sources emit concrete proposals which search filters through core
  assessment.
- Rollouts distinguish encoded terminal state, controller stops, rejection,
  and algorithmic horizons.
- Weighted samplers select only encoded exchange proposals reproducibly.
- Seeded exploration uses ChaCha8, while caller-owned `rand` generators can be
  adapted without changing search semantics.
- Monte Carlo provides Bernoulli, scalar, vector, quantile, and tail
  statistics through established estimators without defining outcome truth.
- Ordered objective schemas reject duplicates, direction drift, and unordered
  values; incremental Pareto fronts preserve non-dominated outcomes.
- Exhaustive Pareto sessions return replayable traces, expose bounded progress
  and interruption, and claim exactness only after reachable-state exhaustion.
- Sampled logistics, rescue, and mission policy fronts remain explicitly
  approximate even after their configured Monte Carlo work completes.
- BFS, Dijkstra, and A* use established implicit-graph implementations while
  still deriving every successor through core exchange validation.
- MCTS supports vector values, encoded chance nodes, deterministic budgets,
  random rollouts, and canonical transpositions.
- ISMCTS root-samples encoded belief worlds, rejects inconsistent
  determinizations, merges actor-visible information states, and revalidates
  the selected live exchange.
- RL projections expose assessment masks, sparse shortfalls, receipts,
  observations, outcomes, and replay-derived transitions.
- BFS, exhaustive Pareto search, Monte Carlo, MCTS, and ISMCTS expose resumable
  deterministic-budget sessions with serializable progress and safe
  cooperative interruption.
- The MCP reference server uses immutable explicit economy handles rather than
  session-scoped current state and returns new handles for successful changes.
- The MCP adapter accepts caller-provided snapshot storage and defaults to a
  process-local, content-addressed `MemorySnapshotStore`.
- MCP search tasks are registered before their handles are returned and expose
  pollable progress, terminal results, configurable retention, and cooperative
  cancellation through `rmcp::TaskManager`.
- Strict Streamable HTTP conformance is exercised end to end with MCP
  `2026-07-28` metadata and standardized routing headers, without legacy
  sessions.
- Built-in benchmark results replay and specialized solvers agree where an
  oracle is provided.
- Property laws cover assessment/application agreement, atomic failure,
  serialization round trips, checked arithmetic, and exact unit lowering;
  Criterion baselines cover core exchange and long-rollout throughput.

## 12. Explicit limitations

The current foundation is intentionally bounded:

- Every economy uses one exact quantity backend, defaulting to `u64`. Numeric
  representations are not selected independently per asset, and floating
  point is not accepted as authoritative balance storage.
- Unit safety is opt-in through `UnitAsset<Id>` and `AssetSchema`; the generic
  kernel cannot infer physical semantics for an arbitrary user-defined asset
  type. Raw counts remain valid and explicitly mean atoms of their asset key.
- Typed `uom` and Jiff adapters currently require the lowered atom count to fit
  `u64`, even when the target economy uses a wider quantity backend.
- Rates are concrete. There is no variable matching, unification, guard
  language, or parameterized rate schema.
- Problem builders may generate many concrete rates to represent one logical
  schema.
- Candidate binding enumeration is supplied by the model or adapter; the core
  filters and validates proposals but does not enumerate every account
  permutation. Search supports lazy visitor-based generation, but there is no
  typed parameterized rate-schema language or automatic binding enumerator.
- Linear invariants are global weighted sums. There are no local, inequality,
  temporal, or arbitrary logical invariant languages.
- Stable insertion-ordered maps make iteration, diagnostics, and ordinary
  serialization reproducible. This does not make a state fingerprint a
  collision-proof durable identity.
- Serialization represents the current public model directly. Durable schema
  migration and explicit wire-version negotiation are not yet supported.
- The rate book is immutable after construction; dynamic laws are not yet
  modeled.
- `fork` clones the account index while sharing immutable laws and account
  contents. There is no persistent map or incremental state fingerprint yet.
- Views restrict account data and observation identities preserve that
  boundary, but there is not yet a capability system for rate visibility or
  proposal authority.
- MCTS and ISMCTS currently provide UCT, not PUCT priors, progressive widening,
  or deterministic parallel workers. Belief construction and updating remain
  caller-supplied projections from encoded state.
- Monte Carlo consumes finite encoded weighted supports; parameterized or
  continuous distribution schemas remain future work.
- Exact Pareto search currently exhausts finite reachable states without
  intermediate dominance pruning. Large or cyclic models must provide bounded
  state and candidate spaces; epsilon fronts, constrained dominance, and
  domain-proven safe pruning are not implemented.
- Native Rust and single-threaded `wasm32-unknown-unknown` are validated Studio
  engine targets. GitHub Pages cannot supply cross-origin isolation headers, so
  the reference browser engine deliberately avoids Wasm threads and
  `SharedArrayBuffer`; scale comes from a dedicated Worker, not parallelism.
- Studio run control is cooperative. Resumable Monte Carlo, MCTS, and ISMCTS
  adapters check pause/cancel between bounded work chunks; other adapters check
  at phase and document-publication boundaries. A one-shot domain solver is not
  preempted in the middle of an indivisible call. Native runs support resumable
  pause. Browser cancellation is immediate because the Worker is disposable;
  browser pause is intentionally not advertised until service sessions can be
  suspended and resumed across Worker event-loop turns.
- Full model projection currently materializes every concrete rate. Studio
  filters large rate books client-side. Standard 7×6 Connect Four now uses 226
  concrete placement, adjudication, and four-cell certificate rates—smaller
  than the former 1,282-rate 4×4 counter cross-product—but pagination or a
  generic parameterized-rate view still awaits measured pressure from other
  domains.
- The MCP reference binary keeps snapshots and task lifecycle in memory. Its
  handles do not survive restart; callers can implement `SnapshotStore` when
  snapshot persistence is required, while distributed task recovery remains
  outside the reference scope.
- The MCP reference server has no authentication, tenant isolation,
  authorization, quotas, TTL garbage collection, distributed worker leasing,
  cross-instance notifications, signatures, consensus, or production
  financial controls.
- The conformance suite is evidence for useful bounded expressiveness, not a
  proof of computational universality.

## 13. Design findings from the problem suite

### 13.1 Multi-account consume/produce/preserve is the correct minimum

Bilateral transfer cannot naturally represent pushing a crate through three
cells, reserving two machine slots, or resolving two bids with one bridge.
Role-bound atomic rewriting is the smallest core that supports these without
an external state update.

### 13.2 Read-only facts deserve first-class semantics

Edges, tools, goals, truth, and completion evidence are required without being
consumed. Explicit `preserve` makes rates clearer and distinguishes a fact from
a resource.

### 13.3 Invariants belong to the problem, enforcement belongs to the core

The workshop can install a malformed “counterfeit chair” rate. It still cannot
create mass because the economy independently rejects the firing. Solver
discipline is not a trust boundary.

### 13.4 Concrete rate generation works but is the largest scalability pressure

The maze and workshop need few rates. Scheduling creates a rate for each
operation, readiness time, and start time. Rescue creates a rate for each
truth/seed outcome. This is a real scaling pressure, but not yet authority to
add a fifth semantic primitive. Concrete rates remain the minimum until
measurement justifies a constrained, inspectable rate-schema and binding
system; arbitrary hidden callbacks would not be acceptable.

### 13.5 Objectives can remain assets while algorithms remain generic

Energy spent, process time, waste, per-job completion, participant utility,
retained credit, and cooling energy are ordinary balances. Search projections
only read those balances. Objective names and minimize/maximize directions
choose comparison policy; they do not define or alter the outcome state.

The exact maze, workshop, scheduling, bridge, marketplace, and perishables
fronts demonstrate why this matters. They preserve real discrete alternatives
instead of committing early to weights, make distributional allocation visible
alongside resource efficiency, and return a replayable proof for each retained
point. The search remains conservative: it filters completed outcomes and does
not assume that current intermediate objective values safely predict all
future exchanges.

Logistics, rescue, and mission apply the same relation to Monte Carlo summaries
of encoded outcomes. Their fronts are useful decision estimates, not exact
claims about the policy space. Exactness therefore belongs in the result type,
not in prose or caller convention.

### 13.6 Specialized solvers fit when translation and replay are explicit

Algorithm X and the separate schedule branch enumerator emit exchanges. Small
scheduling horizons are also checked against a direct domain-level brute-force
oracle. This is the adapter contract future OR integrations must preserve:
strategy diversity does not itself prove model fidelity, while replay does not
replace an independent oracle.

### 13.7 Nature is enough to make chance and priors auditable

Encoding unresolved state, scenario weights, truth, and seed removes both the
prior and stochastic mutation from the ambient runtime. Instantiation and
observation exchanges record everything needed for exact replay.

### 13.8 Rejection should be explanatory, not binary

The same explicit baskets that make an exchange verifiable can explain why it
is infeasible. A complete multi-account shortfall vector is therefore not an
optional diagnostics layer; it is a direct product capability of the economic
representation. Projected deltas extend that explanation from “what is
missing?” to “what would change?”, while receipts remain the authoritative
record of what actually changed.

### 13.9 Rollout must precede Monte Carlo

Rescue, logistics, and the team mission now share one rollout executor. This
prevents policy evaluation, long-horizon simulation, and future learned
controllers from creating subtly different transition loops. Monte Carlo is
an aggregator over randomly sampled trajectories, not an environment. When a
finite support is deliberately traversed exactly, the product and docs call it
scenario evaluation rather than Monte Carlo.

### 13.10 Adversarial terminal truth can remain encoded

Connect Four maintains gravity, turns, and every winning-line count as assets.
A winning move produces the winner directly, so MCTS reads terminal values
without defining an external board rule. This is more verbose than a callback
but preserves the core's independent authority.

### 13.11 Learning interfaces should preserve economic explanations

Action masks derive from assessment status, dense failure features derive from
complete shortfalls, and successful transitions retain receipts. Replayed
traces can therefore become datasets without constructing a second mutable
environment or discarding why an action failed.

### 13.12 Hidden information requires a separate search contract

Filtering an agent's hand-written policy is insufficient if the search tree
still uses the full economy as its node identity. Information-set search must
receive an actor-visible observation, merge indistinguishable worlds, sample
possible encoded worlds from that observation, and avoid passing hidden
accounts to decision generation or rollout policy. Environment resolution may
use hidden truth only through concrete Nature exchanges.

Belief sets are caller-owned derived artifacts, not authoritative parallel
world state. The caller supplies complete encoded determinizations, advances
them through public exchanges and required Nature responses, filters them by
the new canonical observation, and may invoke the same information-set planner
again. Every determinization remains an economy and every selected live action
is still core-revalidated.

The mission also shows why public intent and hidden outcome should be different
rates. `BeginScan` is a decision available in every indistinguishable world;
`ResolveScan` is Nature's encoded reaction. Otherwise a supposedly public
action identifier would itself reveal the truth or random seed.

The smaller Rescue fixture applies the same contract with `BeginObserve` and
`ResolveObservation`. Its ordinary policy receives only the agent's restricted
economic view; the full world is consulted solely by the separate Nature
resolution path.

### 13.13 Semantic identity must be witnessed by assets

A descriptive rate ID cannot authorize an account binding because the generic
kernel does not interpret application identifiers. If a transition means
“this cell,” “this machine slot,” “this bidder,” or “this agent role,” the
bound account must preserve the corresponding identity or capability asset.
The same principle applies to terminal state: a consumable active lifecycle
asset prevents repeated completion and makes terminal quiescence economic
rather than a candidate-generator convention. Conformance tests must bypass
trusted action helpers and attempt incorrect bindings directly.

### 13.14 Remote statelessness must be semantic, not merely transport-level

Disabling HTTP sessions does not help if a server retains an ambient mutable
economy. Immutable explicit snapshot handles make every operation reproducible
and every branch visible. Snapshot and task metadata may outlive one request
because they identify immutable input and control derived computation, but
their lifetime is caller policy and a worker can only return a trace of
core-validated exchanges. This is the same authority boundary as an in-process
priority queue expressed across a protocol.

### 13.15 Shared fate should be represented once

The perishables fixture separates fungible claims from one unique condition
fact per cohort. A power or decay exchange changes the shared condition while
leaving every claim balance untouched; claim use remains gated by that fact.
This is more than a storage optimization: it is the correct economic model for
many claims governed by one pool, epoch, contract, or physical condition.

The same fixture distinguishes this semantic indirection from disposable
indexes. Its holdings index is rebuilt from balances and maintained from
receipts, while its event agenda is rebuilt from fresh-condition facts. Both
can be stale or absent without permitting an invalid transition. Explicit
before/reached assets ensure the core rejects early decay and late use even if
the agenda is delayed.

This pressure also improved the kernel application path. Applicable exchanges
prepare and commit only touched account contents. Global linear invariants are
normally validated by comparing the weighted consumed and produced receipt
deltas; a complete before/after state is constructed only to explain an actual
violation. Fork creation still copies the top-level account index and remains
a separate measured scaling target.

## 14. Roadmap

The remaining items are a pressure-driven backlog, not an instruction to add
machinery speculatively. In particular, lazy concrete action generation is the
current minimum; parameterized schemas should begin only when benchmarked
domains show that concrete rate construction or enumeration is the limiting
factor.

1. Measure concrete rate construction and lazy candidate enumeration in larger
   scheduling, game, and logistics models.
2. If that pressure is material, define a serializable parameterized
   rate-schema language with typed finite binding domains and automatic
   candidate instantiation without arbitrary hidden guards.
3. Add explicit wire-schema versioning only when compatibility with deployed
   historical data becomes a real requirement.
4. Measure large fork trees and many-holder simulations; if the top-level
   account-index copy dominates, adopt a persistent map and incremental
   fingerprints. Extract generic holdings, dependency, and event indexes only
   after a second domain confirms the perishables shapes.
5. Extend invariants with carefully constrained local and inequality forms.
6. Build an external OR adapter that compiles one closed schedule and replays
   the returned assignment.
7. Extend encoded Nature schemas and distribution updates beyond finite
   weighted supports.
8. Extend the exact-cover, scheduling, and Connect Four reference-oracle tests
   into broader bounded model checking where failures justify the cost.
9. Decide whether dynamic rate availability is represented by rate assets,
    capability assets, or immutable schemas plus explicit enabling state.
10. Add PUCT priors, progressive widening, and deterministic parallel workers
    when learned priors, measured branching, or throughput justify them.
11. Add capability-scoped proposal visibility and richer multi-agent belief
    and communication protocols.
12. Integrate external learned policies through the implemented RL
    projections without granting mutation authority.
13. Add persisted search checkpoints, worker leases, task notifications, and
    tenant-aware authorization only when the MCP reference boundary is moved
    into a real multi-process deployment.
14. Measure full-artifact transfer and rendering across the thirteen Studio
    adapters; add paged model projection, incremental Pareto/Monte Carlo
    publication, or compact rate schemas only where measured pressure warrants
    them.
15. Build the [agent benchmark and rating layer](AGENT_BENCHMARK.md) across
    single-agent, cooperative, competitive, and mixed-motive designs. Use it to
    pressure generated instances, long-horizon policies, coordination, work
    accounting, robustness, and replay-derived scoring.

Performance work should follow semantic clarity. Application now clones only
touched account contents and validates conserved deltas incrementally. Shared
immutable laws and account contents reduce branch cost, but fork-time account
index cloning and concrete rate expansion remain reference implementations
rather than final scale targets.

## 15. Decision record

### D-001: Semantic closure is non-negotiable

All authoritative problem state is encoded through assets, accounts, rates,
and exchanges. A parallel world model is an architectural defect.

### D-002: Exchanges are the only runtime mutation

Initialization constructs the problem. Afterward, semantic effects must pass
atomic exchange validation and produce receipts.

### D-003: User-defined ontology, core-owned instances

Users define Rust vocabulary. The economy owns balances, laws, effects, and
accepted history semantics.

### D-004: Rates are multi-account rewrite laws

Bilateral trade is one rate shape, not the universal abstraction.

### D-005: Conservation is declared, not assumed literally

Transformations preserve problem-specific dimensions expressed by invariants.

### D-006: Preserved facts are unscaled thresholds

Exchange multiplicity scales consume and produce. Preserved facts and
catalysts are read once per atomic proposal so batching matches sequential
reuse.

### D-007: Solvers are untrusted accelerators

The only accepted solution artifact is a replayable, core-validated exchange
trace.

### D-008: Goals and chance are economic state

Terminal conditions, hidden truth, observations, and random seed evolution are
encoded rather than supplied by opaque callbacks.

### D-009: Bounded structured expressiveness precedes universality claims

Thirteen executable problems provide stronger product evidence than a vacuous
one-token-per-world construction. Turing completeness is not a current goal.

### D-010: Conformance problems stay in-tree

The `axionomy-problems` crate is an executable architectural test package. It
should evolve with the API and expose when a proposed abstraction makes one
domain elegant at another domain's expense.

### D-011: Solvers and models are separate crates

The kernel contains neither reference algorithms nor domain ontologies.
Workspace crates may depend on the kernel; the kernel never depends on them.

### D-012: Assessment is derived, structured, and non-authoritative

The core exposes complete shortfall vectors and projected deltas derived from
the same preparation path as application. It does not assign universal scalar
costs. Algorithms may value an assessment, but only encoded rates determine
validity and only applied exchanges change state.

### D-013: Rollout is the foundational simulation abstraction

A rollout is a core-validated speculative exchange history. Monte Carlo,
MCTS, learned policies, and mission planners share rollout execution rather
than defining separate transition loops.

### D-014: Domain randomness and exploration randomness are distinct

Domain outcomes and their evolving seeds are economic state realized through
Nature exchanges. Solver randomness may remain external deterministic
metadata when it affects only which valid branches are explored.

### D-015: Algorithmic cutoffs are not domain terminal state

A rollout horizon, node budget, or wall-clock budget may stop computation but
cannot claim that a goal, loss, draw, deadline, or mission outcome occurred.
Those meanings require encoded state.

### D-016: Outcome aggregation is disposable policy

Monte Carlo and learning systems may aggregate encoded outcome assets into
means, variances, quantiles, tail risk, or value estimates. Aggregation may
rank proposals but cannot define the underlying outcome.

### D-017: Structural sharing is not semantic sharing

Forks may share immutable laws and untouched account storage. Applying an
exchange to one fork still prepares and commits an isolated account map, so
storage optimization cannot make speculative mutation visible elsewhere.

### D-018: Learning data is a projection, not an environment

Action masks come from assessment, transitions come from receipts or
rejection, and terminal/outcome features are read from encoded state.
Training adapters never receive a second mutation authority.

### D-019: Information identity must exclude hidden state

Partially observed planning keys its tree by actor plus canonical economic
observation, never by the complete economy. Belief samples are closed economies
consistent with that identity; they do not become accepted truth.

### D-020: Public intent and hidden reaction are separate exchanges

An actor cannot be required to propose a rate identifier containing facts it
cannot observe. Public decisions create encoded pending state, and Nature
resolves that state through the concrete hidden-dependent exchange.

### D-021: Lazy proposal generation does not create a second action model

An action source may derive exchanges on demand, but its output is immediately
subject to core assessment. It owns neither transition validity nor effects,
and full parameterized rate schemas remain deferred until measured problem
pressure justifies them.

### D-022: Asset meaning, numeric representation, and typed authoring are separate

An asset defines what a quantity means and names its atomic basis.
`Quantity<N>` defines exact non-negative arithmetic. `uom` and calendar-aware
time adapters validate richer authoring values and lower them into
asset-qualified quantities. No physical dimension, unit, clock, or schedule
becomes hidden authoritative state outside the economy.

### D-023: Ecosystem primitives precede local reinvention

The implementation uses mature crates for stable maps, serialization,
numeric traits, units, civil time, randomness, probability, pathfinding,
errors, property testing, and benchmarking when they preserve the closure
contract. Axionomy-specific traits express domain laws such as non-negativity
and signed invariant measurement; they do not reimplement general-purpose
containers or arithmetic machinery.

### D-024: Derived algorithms keep their own numeric domains

Search depth, heuristic rank, visit count, statistical confidence, and wall
clock runtime are disposable algorithm state. They do not inherit the
economy's quantity backend unless a model explicitly encodes a corresponding
asset, shortfall, cost, or reward. This keeps solver policy from silently
changing economic truth.

### D-025: Atomic denomination is intrinsic to unit-aware asset identity

Accounts, baskets, and rates do not select units. A unit-aware asset key carries
its logical ID, physical dimension, stable quantity kind, and exact atomic
basis; all typed inputs normalize into that identity. Conflicting definitions
cannot compare equal, and a model schema rejects one logical ID carrying more
than one definition. The schema is derived authoring validation because the
definition remains serialized in the asset itself. Intentional denominations
are separate assets connected by explicit rates.

### D-026: Observability is derived and consumer-owned

Structured logs explain model construction, proposals, outcomes, and replay,
but they never carry authoritative semantics. Libraries leave subscriber
selection to their caller; example binaries configure `tracing` locally. A log
filter may reveal more derived detail, but it cannot alter the economy.

### D-027: Search control is resumable, bounded, and runtime-neutral

Long-running algorithms advance in deterministic work units and expose
serializable progress plus safe interruption. Async runtimes, transport tasks,
threads, and cancellation-token implementations belong to callers. Stopping
computation never creates encoded terminal truth.

### D-028: Remote state uses explicit immutable economy handles

A stateless integration names every economy snapshot and returns a new handle
for every accepted change. Durable task records are allowed as disposable
computation control, but neither they nor protocol sessions may become a
parallel world model. Completed remote search returns a core-replayable trace.

### D-029: Effects are triggered rates, not hidden mutation

Time, environment, power, and cohort condition remain assets in accounts.
An event agenda may index those facts and propose a due exchange, but it never
mutates state or decides validity. Before/reached window assets prevent early
effects and late use even when materialization is delayed. Environment changes
atomically reclassify shared cohort state, causing previously queued proposals
to become ordinary infeasible exchanges. This preserves deterministic replay
and permits event-driven work proportional to affected cohorts rather than
individual fungible units.

### D-030: Fungibility is structural, uniqueness is modeled

One asset identity with quantity greater than one is fungible. A distinct
identity with conserved unit supply is non-fungible. Shared claims may point to
one cohort or pool condition so a global economic change touches one encoded
fact rather than every holder. The core does not add a universal fungibility
flag because interchangeability and uniqueness are ontology claims that the
problem must close with identities, rates, quantities, and invariants.

### D-031: Pareto comparison defers preference without externalizing outcomes

Multi-objective values are read from encoded terminal economies. Ordered keys,
minimize/maximize direction, dominance filtering, statistical estimation, and
the final selection are disposable policy. Exact fronts require exhaustive
finite search and replayable traces; interrupted or sampled fronts remain
explicitly approximate. This exposes allocation tradeoffs without granting a
solver authority to define utility, validity, or state.

### D-032: Visualization is a disposable replay projection

The universal viewer derives snapshots, assessments, receipts, and analysis
from a core-replayed trace. Domain scenes are optional read-only projections;
browser playback and transport lifecycle are operational state. Rust owns the
monomorphic wire contract, exact quantities serialize as text, and generated
OpenAPI/TypeScript prevents a second manually maintained type system. A viewer
may explain economic truth but cannot create it.

### D-033: Interfaces share commands and artifacts, not semantics by convention

CLI, HTTP/Studio, and MCP adapt one runtime-neutral problem service. Catalogs,
strategy requests, progress, control, and complete replay-derived artifacts
are defined once and tested for cross-interface equality. Interface pressure
may improve this shared boundary and reveal missing kernel explanations, but
transport lifecycle and presentation state never become economic authority.

### D-034: Correctness fixtures and product workloads are distinct instances

Micro keeps exhaustive laws and independent oracles cheap. Showcase is the
decision-dense interface default. Stress scales a domain-relevant dimension.
All three are ordinary closed economies with explicit artifact identity; none
may introduce external semantic state. Complexity telemetry and per-problem
Showcase thresholds prevent attractive visualizations from hiding trivial
models or forced traces.

### D-035: Browser execution reuses the Rust service

The browser does not maintain a TypeScript economy or a second set of problem
solvers. `axionomy-studio-wasm` compiles the interface-neutral service to
`wasm32-unknown-unknown`; a dedicated Worker transports Rust-owned requests,
observations, and artifacts. Native HTTP/SSE remains preferable when present,
the Worker is the first-class GitHub Pages engine, and committed artifacts are
the final read-only fallback. Connection labels report verified current
capability rather than inferring liveness from cached data.

### D-036: Visual richness is typed explanation, not extra truth

A scene has a geometric surface plus stable, role-bearing semantic entities,
anchors, typed
account/balance evidence links, paths, annotations, exact metrics, and a
constrained glyph vocabulary. Tabler is a replaceable browser rendering
dependency. Frame cues are derived from the exchange and receipt, and scene
anchors plus evidence references are validated before an artifact is
published. A generic renderer composes deterministic adjacent-step motion and
receipt effects without knowing the problem domain. Entity roles distinguish
structure, occupants, attachments, owned state, and global context so a viewer
can show containment and relationships rather than a flat collection. The
roles change only composition; they cannot authorize or alter an exchange.
This permits rich
animation and linked inspection while preserving
the rule that deleting every visual projection changes no valid exchange,
balance, goal, or replay result.

### D-037: Competitive standings are replay-derived, plural, and shareable

The engine does not own one universal winner. A problem may expose several
typed `LeaderboardView` projections at every `ViewSnapshot`; their entries
retain exact values, eligibility, ties, participant identity, and explanatory
components. Score-bearing mechanics remain assets, while ordering and Pareto
membership remain disposable policy. Studio URLs capture the selected problem,
outcome, replay step, and leaderboard so an explanation is reproducible on
native HTTP, browser Wasm, and static Pages without auto-running a task.

## 16. Success criteria

Axionomy succeeds when:

- A complete problem is reconstructible from initial accounts, assets, rates,
  invariants, and exchange history.
- No authoritative position, clock, goal, constraint, belief, reward, or
  chance state exists outside the core.
- Every semantic effect is a receipt-producing exchange.
- Users introduce new ontologies without modifying kernel source.
- Different solvers operate over identical semantics.
- Multi-objective search exposes replayable non-dominated outcomes without
  forcing one universal scalar utility or overstating sampled completeness.
- External solver results translate into replayable exchanges.
- Invalid proposals cannot bypass core constraints.
- Remote callers can store, inspect, branch, search, cancel, poll, and replay
  without relying on hidden session state.
- Browser users can load a portable document, follow a native live run, or run
  the same Rust service in a Worker; they can inspect retained solver evidence,
  scrub every accepted exchange, and inspect exact balances and deltas without
  a parallel mutable world model or handwritten cross-language contract.
- All thirteen canonical problems are discoverable, runnable, replayable, and
  meaningfully inspectable through Studio and static Showcase artifacts, with
  explicit Micro and Stress selection on live interfaces.
- CLI, HTTP, MCP, and the browser Worker return the same semantic problem
  artifacts for the same request, while retaining interface-appropriate
  operational behavior.
- Infeasible proposals expose every account-and-asset shortfall without
  mutation.
- Successful assessments project the same account deltas later confirmed by
  receipts.
- Unit-aware models cannot silently combine incompatible physical dimensions,
  quantity kinds, or atomic denominations under one asset identity.
- Encodings remain structured, local, inspectable, and compositional.
- Mathematical claims are supported by executable laws or proofs.

The guiding statement remains:

> Users encode what exists, what is true, and what may change as assets,
> accounts, and rates. Every real change is an exchange. Algorithms may
> explore or propose, but only the closed economic machine defines and
> validates reality.
