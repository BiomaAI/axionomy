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
Asset A             = what the value means and its atomic economic basis
Quantity<N = u64>   = how its exact non-negative coefficient is represented
Typed binding       = how a physical or calendar value is validated and lowered
```

For example, a `uom` mass value may be converted exactly into an
`Asset::CargoGram` and a `Quantity<u64>`. The physical dimension is checked at
the authoring boundary, the asset preserves the unit and economic identity,
and the quantity supplies the coefficient. A heterogeneous basket therefore
does not erase meaning merely because it stores one numeric backend.

One economy uses one numeric backend `N`. This gives every balance, rate,
exchange, shortfall, receipt, and invariant one coherent arithmetic contract.
Using a different numeric representation for every asset is deliberately out
of scope: it would introduce dynamic value dispatch into the authoritative
state and make conservation and generic algorithms substantially less clear.

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

## 6. Workspace and package boundaries

The repository makes authority boundaries visible as dependency boundaries:

```text
axionomy-search ──────→ axionomy
axionomy-problems ────→ axionomy
                    └─→ axionomy-search
```

- `axionomy` owns universal state and transition semantics.
- `axionomy-search` owns disposable action generation, search, rollout,
  sampling, Monte Carlo, perfect-information MCTS, information-set MCTS, and
  learning projections.
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

The `axionomy-search` crate contains deliberately inspectable reference
implementations: BFS, best-first search, rollout execution, weighted sampling,
Monte Carlo aggregation, vector-valued MCTS, observation-scoped ISMCTS, and RL
trajectory projections. The `axionomy` kernel remains an execution and
validation substrate, not an attempt to replace every mature solver.

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
settlement. One rate transfers a widget and gross payment while splitting
seller proceeds, tax, platform commission, and shipping fees; it also consumes
shipping capacity and advances buyer, seller, and order lifecycle assets.
There is no privileged bilateral transaction hidden underneath it and no
compensating rollback protocol: either every role requirement and invariant
passes and all six accounts commit, or nothing changes.

Potential matches are ordinary exchanges built by deriving participant sets
from economy accounts. `applicable` finds exact matches, while `assess` exposes
complete shortfalls for near matches. A caller may rank those shortfalls for
procurement, search, or learned policy, but the ranking is not market truth.
Only account assets, the settlement rate, its role bindings, and the applied
exchange determine whether and how settlement occurs.

This is the current answer to simultaneous proposals: the resolution law must
be represented by a rate, not an external world update. Richer transaction
protocols and dynamic mechanism selection remain future design space.

## 10. Executable conformance suite

The project now validates the thesis with eleven deliberately different
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
| Marketplace | Account-derived matching, complete shortfalls, six-party atomic settlement, and caller-owned near-match ranking |
| Logistics | Recurrent encoded chance, long rollout horizons, repair loops, replenishment, and risk-aware Monte Carlo |
| Connect Four | Encoded gravity, turns, line counters, terminal truth, vector outcomes, and adversarial MCTS |
| Mission | Canonical private observations, public intent versus hidden Nature resolution, ISMCTS, exchanged intelligence, joint action, and RL trajectory projection |

These are not unrelated demos. Together they test:

- Search, constraint satisfaction, optimization, simulation, negotiation,
  market matching, adversarial play, and learning-data generation.
- Single-, multi-, and hidden-account transitions.
- Resource consumption, fact preservation, and identity transformation.
- Feasible and infeasible instances.
- Specialized algorithms over the same core semantics.
- Replay of every accepted solution.

The benchmark ontologies live in `crates/axionomy-problems`, outside the
generic kernel package. Their presence in the workspace makes architectural
pressure executable and prevents future API changes from silently narrowing
the model. Each ontology also has a minimal example binary that uses only its
public API, replays the proposed trace, and checks the encoded goal. Examples
are teaching and integration surfaces; authoritative problem semantics remain
in the reusable model modules.

## 11. Current guarantees

- Asset, account, role, and rate-ID types are user-defined.
- Quantities are non-negative and checked.
- Missing balances behave as zero.
- Exchange application is atomic across all affected accounts.
- Consume, produce, and preserve effects are explicit.
- Required role bindings and role distinctness are checked.
- Failed exchanges preserve complete state.
- Declared linear invariants are checked on every firing.
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
- Monte Carlo provides Bernoulli, scalar, vector, quantile, and tail
  statistics without defining outcome truth.
- MCTS supports vector values, encoded chance nodes, deterministic budgets,
  random rollouts, and canonical transpositions.
- ISMCTS root-samples encoded belief worlds, rejects inconsistent
  determinizations, merges actor-visible information states, and revalidates
  the selected live exchange.
- RL projections expose assessment masks, sparse shortfalls, receipts,
  observations, outcomes, and replay-derived transitions.
- Built-in benchmark results replay and specialized solvers agree where an
  oracle is provided.

## 12. Explicit limitations

The current foundation is intentionally bounded:

- Every economy uses one exact quantity backend, defaulting to `u64`. Numeric
  representations are not selected independently per asset, and floating
  point is not accepted as authoritative balance storage.
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
an aggregator over trajectories, not an environment.

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

The mission also shows why public intent and hidden outcome should be different
rates. `BeginScan` is a decision available in every indistinguishable world;
`ResolveScan` is Nature's encoded reaction. Otherwise a supposedly public
action identifier would itself reveal the truth or random seed.

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
4. Replace the cloned account index with a persistent map, add incremental
   fingerprints, and continue the checked-in logistics rollout benchmark.
5. Extend invariants with carefully constrained local and inequality forms.
6. Define first-class objective declarations and multi-objective comparison
   while keeping objective quantities encoded.
7. Build an external OR adapter that compiles one closed schedule and replays
   the returned assignment.
8. Extend encoded Nature schemas and distribution updates beyond finite
   weighted supports.
9. Add property-based reference-model tests and bounded model checking.
10. Decide whether dynamic rate availability is represented by rate assets,
    capability assets, or immutable schemas plus explicit enabling state.
11. Add PUCT priors, progressive widening, and deterministic parallel workers
    when learned priors, measured branching, or throughput justify them.
12. Add capability-scoped proposal visibility and richer multi-agent belief
    and communication protocols.
13. Integrate external learned policies through the implemented RL
    projections without granting mutation authority.

Performance work should follow semantic clarity. Shared immutable state and
copy-on-write account contents reduce branch cost, but the cloned account
index and concrete rate expansion remain reference implementations rather
than final scale targets.

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

Eleven executable problems provide stronger product evidence than a vacuous
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
- Infeasible proposals expose every account-and-asset shortfall without
  mutation.
- Successful assessments project the same account deltas later confirmed by
  receipts.
- Encodings remain structured, local, inspectable, and compositional.
- Mathematical claims are supported by executable laws or proofs.

The guiding statement remains:

> Users encode what exists, what is true, and what may change as assets,
> accounts, and rates. Every real change is an exchange. Algorithms may
> explore or propose, but only the closed economic machine defines and
> validates reality.
