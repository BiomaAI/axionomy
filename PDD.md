# Axionomy

## Closed Economic State Machine — Product Design Document

| Field | Value |
| --- | --- |
| Status | Implemented foundation; living design |
| Product | Axionomy Cargo workspace |
| Version | `0.1.0` |
| Rust edition | 2024 |
| Minimum Rust | 1.85 |
| Last updated | 2026-07-30 |

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

`Quantity`, `Basket`, `Goal`, `Receipt`, and `Trace` support these primitives;
they do not introduce parallel semantic worlds.

Users define ordinary Rust types for assets, account IDs, rate IDs, and roles.
That is an open vocabulary, not an authorization to keep authoritative
instances elsewhere. Reference ontologies live in the separate
`axionomy-problems` crate. No one of them is built into the kernel.

## 4. Formal state and transition model

### 4.1 State

The complete current state is a finite sparse mapping:

```text
S : Account × Asset → u64
```

Missing entries have quantity zero. Zero entries are canonicalized away.

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

### 4.5 Declared invariants

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

### 4.6 Goals

A `Goal` is a set of required baskets at concrete accounts. Matching is
monotone: an account may own additional assets. Problems commonly use a final
rate that preserves domain completion evidence and produces `Solved` in a
success account.

This avoids arbitrary termination callbacks. Terminal state is part of the
economy.

## 5. Core API and ownership boundary

| Type or operation | Responsibility |
| --- | --- |
| `Quantity` | Checked non-negative multiplicity |
| `Basket<A>` | Sparse asset multiset |
| `Account<A>` | Balance container used during initialization and internal commits |
| `Rate<Role, A>` | Multi-role consume/produce/preserve law |
| `Exchange<RateId, Role, AccountId>` | Concrete proposal |
| `EconomyBuilder` | Initial problem construction |
| `Economy` | Private account/rate ownership and sole execution authority |
| `ApplyError` | Structured rejection reason |
| `Receipt` | Accepted exchange plus per-account deltas |
| `Trace` | Ordered replayable exchange sequence |
| `Goal` | Required terminal asset configuration |
| `EconomicView` | Account-restricted, immutable projection |

`Economy` exposes immutable account and rate inspection. It does not expose a
mutable account reference. Semantic mutation occurs only through `apply` or
`replay`.

Search and simulation use:

- `fork` to create an isolated branch.
- `simulate` to apply one exchange on a branch.
- `replayed` to validate a full trace without touching its source.
- `state_key` for a canonicalized logical-state key.
- `applicable` to filter concrete proposals through core validation.

`replay` mutates its target one exchange at a time. If a caller needs
all-or-nothing validation of an entire trace relative to an existing economy,
it uses `replayed`; the source remains unchanged.

## 6. Workspace and package boundaries

The repository makes authority boundaries visible as dependency boundaries:

```text
axionomy-search ──────→ axionomy
axionomy-problems ────→ axionomy
                    └─→ axionomy-search
```

- `axionomy` owns universal state and transition semantics.
- `axionomy-search` owns disposable reference search strategies.
- `axionomy-problems` owns domain ontologies, problem constructors,
  specialized proposers, and conformance tests.

The kernel must never depend on a solver or problem crate. Moving BFS and A*
out of the kernel is philosophically significant: algorithms explore the
machine but do not define it. Moving reference problems out prevents their
asset types from becoming privileged engine concepts.

The problem crate depends only on public APIs. It therefore tests not only
correctness but whether downstream model authors can express the intended
domains without kernel access.

## 7. Solver contract

A solver may keep priority queues, visited sets, rollout trees, clause
databases, and other acceleration structures outside the economy when those
structures are fully derived and disposable. Removing them must not change
problem validity, transition effects, goals, or replay.

A solver may:

- Inspect immutable core state or a permitted view.
- Construct concrete exchange bindings.
- Ask the core which proposals are applicable.
- Fork states and explore.
- Read objective or heuristic assets.
- Compile a bounded economy to another representation.
- Return an exchange trace.

A solver may not:

- Add an unencoded domain constraint.
- Depend on a hidden position, clock, goal, seed, or belief.
- Directly install a successor state.
- Declare an assignment accepted without core replay.

The `axionomy-search` crate's `bfs` and `best_first` functions are
intentionally small reference strategies. The `axionomy` kernel is an
execution and validation substrate, not an attempt to replace every mature
solver.

## 8. Partial observation and chance

Ground truth and belief belong in different accounts. An `EconomicView`
restricts which accounts a policy may inspect.

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
uncertain model. The current library does not generate ambient random numbers;
a Nature strategy selects among weighted, encoded outcome rates.

## 9. Simultaneous and multi-agent decisions

Joint effects are encoded as one multi-account rate. In the bridge benchmark,
auction resolution consumes both submitted bids and bridge capacity and
produces the winner's crossing right and the loser's waiting status atomically.

This is the current answer to simultaneous proposals: the resolution law must
be represented by a rate, not an external world update. Richer transaction
protocols and dynamic mechanism selection remain future design space.

## 10. Executable conformance suite

The project now validates the thesis with seven deliberately different
problems. Full formal specifications live in [PROBLEMS.md](PROBLEMS.md).

| Problem | What it demonstrates |
| --- | --- |
| Key-door maze | A closed graph, explicit key/lock state, encoded energy and heuristic, and different BFS versus A* choices |
| Sokoban | Atomic three-account rewrites, spatial occupancy, and core-observed infeasibility |
| Exact cover | Logical constraint state plus Algorithm X as an untrusted proposal generator |
| Workshop | Stoichiometric transformation, reusable catalysts, waste objective, and invariant rejection |
| Job shop | Precedence tokens, discrete capacity accounts, makespan assets, and solver agreement |
| Rescue | Hidden truth, restricted views, encoded seed evolution, Monte Carlo, and deterministic replay |
| Bridge | Multi-agent bids, escrow, capacity, joint resolution, and alternative mechanisms |

These are not unrelated demos. Together they test:

- Search, constraint satisfaction, optimization, simulation, and negotiation.
- Single-, multi-, and hidden-account transitions.
- Resource consumption, fact preservation, and identity transformation.
- Feasible and infeasible instances.
- Specialized algorithms over the same core semantics.
- Replay of every accepted solution.

The benchmark ontologies live in `crates/axionomy-problems`, outside the
generic kernel package. Their presence in the workspace makes architectural
pressure executable and prevents future API changes from silently narrowing
the model.

## 11. Current guarantees

- Asset, account, role, and rate-ID types are user-defined.
- Quantities are non-negative and checked.
- Missing balances behave as zero.
- Exchange application is atomic across all affected accounts.
- Consume, produce, and preserve effects are explicit.
- Required role bindings and role distinctness are checked.
- Failed exchanges preserve complete state.
- Declared linear invariants are checked on every firing.
- Receipts describe accepted per-account deltas.
- Forks are isolated clones.
- Trace replay uses exactly the same validation path as live execution.
- Restricted views cannot inspect omitted accounts.
- Built-in benchmark results replay and specialized solvers agree where an
  oracle is provided.

## 12. Explicit limitations

The current foundation is intentionally bounded:

- Quantities are `u64`; there are no signed balances, fractions, or unbounded
  tokens.
- Rates are concrete. There is no variable matching, unification, guard
  language, or parameterized rate schema.
- Problem builders may generate many concrete rates to represent one logical
  schema.
- Candidate binding enumeration is supplied by the model or adapter; the core
  filters and validates proposals but does not enumerate every account
  permutation.
- Linear invariants are global weighted sums. There are no local, inequality,
  temporal, or arbitrary logical invariant languages.
- Hash maps are internal storage. `state_key` sorts logical entries, but
  canonical serialization and stable hashing are not yet defined.
- Traces contain exchanges, not durable schema-versioned receipts.
- The rate book is immutable after construction; dynamic laws are not yet
  modeled.
- `fork` performs a full clone; there is no persistent or copy-on-write state.
- Views restrict account data, but there is not yet a capability system for
  rate visibility or proposal authority.
- No concurrency, signatures, persistence, distributed consensus, or
  production financial controls are claimed.
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
truth/seed outcome. The next major abstraction should be a constrained,
inspectable rate-schema and binding system—not arbitrary callbacks.

### 13.5 Objectives can remain assets while algorithms remain generic

Energy spent, waste, and makespan are ordinary balances. Search callbacks only
read those balances. The callback chooses priority; it does not define or
alter the objective state.

### 13.6 Specialized solvers fit when translation and replay are explicit

Algorithm X and the independent schedule enumerator emit exchanges. This is
the adapter contract future OR integrations must preserve.

### 13.7 Nature is enough to make chance and priors auditable

Encoding unresolved state, scenario weights, truth, and seed removes both the
prior and stochastic mutation from the ambient runtime. Instantiation and
observation exchanges record everything needed for exact replay.

## 14. Roadmap

The next work should be driven by measured pressure from these encodings:

1. Define a serializable parameterized rate-schema language with typed
   variables and finite binding domains.
2. Add automatic candidate instantiation without permitting arbitrary hidden
   guards.
3. Define canonical problem, state, exchange, and trace serialization with
   schema/version identifiers.
4. Add persistent or copy-on-write forks and benchmark search memory.
5. Extend invariants with carefully constrained local and inequality forms.
6. Define first-class objective declarations and multi-objective comparison
   while keeping objective quantities encoded.
7. Build an external OR adapter that compiles one closed schedule and replays
   the returned assignment.
8. Standardize weighted Nature sampling, seed evolution, and distribution
   updates beyond the bounded reference implementation.
9. Add property-based reference-model tests and bounded model checking.
10. Decide whether dynamic rate availability is represented by rate assets,
    capability assets, or immutable schemas plus explicit enabling state.

Performance work should follow semantic clarity. The current clone-based
search and concrete rate expansion are acceptable reference implementations,
not final scale targets.

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

Seven executable problems provide stronger product evidence than a vacuous
one-token-per-world construction. Turing completeness is not a current goal.

### D-010: Conformance problems stay in-tree

The `axionomy-problems` crate is an executable architectural test package. It
should evolve with the API and expose when a proposed abstraction makes one
domain elegant at another domain's expense.

### D-011: Solvers and models are separate crates

The kernel contains neither reference algorithms nor domain ontologies.
Workspace crates may depend on the kernel; the kernel never depends on them.

## 16. Success criteria

Axionomy succeeds when:

- A complete problem is reconstructible from initial accounts, assets, rates,
  invariants, and exchange history.
- No authoritative position, clock, goal, constraint, belief, reward, or
  chance state exists outside the core.
- Every semantic effect is a receipt-producing exchange.
- Users introduce new ontologies without modifying kernel source.
- Different solvers operate over identical semantics.
- External solver results translate into replayable exchanges.
- Invalid proposals cannot bypass core constraints.
- Encodings remain structured, local, inspectable, and compositional.
- Mathematical claims are supported by executable laws or proofs.

The guiding statement remains:

> Users encode what exists, what is true, and what may change as assets,
> accounts, and rates. Every real change is an exchange. Algorithms may
> explore or propose, but only the closed economic machine defines and
> validates reality.
