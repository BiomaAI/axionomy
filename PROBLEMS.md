# Axionomy Closed-Problem Conformance Suite

This document specifies the problem set used to evolve and test Axionomy's
API. Each problem is intentionally small enough to solve exhaustively and
different enough to expose a distinct architectural pressure.

The suite is successful only when a problem is genuinely closed:

1. All domain state is held as assets in accounts.
2. Every domain transition is a rate firing.
3. Every concrete action is an exchange with explicit role bindings.
4. Goals are asset configurations.
5. Costs, observations, chance state, and constraints are encoded.
6. Solver-side structures are derived, disposable caches.
7. A proposed solution is accepted only after core replay.

Rust enums and builder loops define the vocabulary and construct the initial
closed problem. They are not mutable parallel world state. A solver may use an
adapter to translate a rate ID into proposed role bindings, but the bindings
become part of the exchange and the rate remains the authority for validity
and effects.

## Conformance matrix

| Capability | Maze | Sokoban | Exact cover | Workshop | Job shop | Rescue | Bridge | Marketplace | Logistics | Connect Four | Mission |
| --- | :---: | :---: | :---: | :---: | :---: | :---: | :---: | :---: | :---: | :---: | :---: |
| Multi-account atomic rewrite | ✓ | ✓ |  |  | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| Preserved facts/catalysts | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |  | ✓ |
| Declared invariants | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| Resource objective | ✓ |  |  | ✓ | ✓ | ✓ | ✓ |  | ✓ |  | ✓ |
| Infeasible instance |  | ✓ | ✓ |  | ✓ |  |  | ✓ | ✓ |  | ✓ |
| Specialized proposer |  |  | Algorithm X |  | Branch optimizer | Monte Carlo | Auction | Assessment matcher | Monte Carlo | MCTS | Monte Carlo |
| Generic algorithm | BFS/A*/Dijkstra | BFS | BFS | BFS/best-first | Best-first | Rollout | BFS |  | Rollout/MC | MCTS | ISMCTS/Rollout/MC/RL |
| Hidden or stochastic state |  |  |  |  |  | ✓ |  |  | ✓ |  | ✓ |
| Multi-agent resolution |  |  |  |  |  |  | ✓ | ✓ |  | ✓ | ✓ |
| Long-horizon trajectory |  |  |  |  |  |  |  |  | ✓ | ✓ |  |
| Learning trajectory |  |  |  |  |  |  |  |  |  |  | ✓ |
| Observation-scoped tree |  |  |  |  |  |  |  |  |  |  | ✓ |
| Deterministic replay test | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |

## Runnable examples

Each model has a symmetric consumer example. These binaries contain
orchestration and display logic only; all state, rules, goals, and constraints
remain in the reusable problem modules.

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
```

The examples emit structured `tracing` events at `INFO` by default. Set
`RUST_LOG=debug` to inspect accepted exchange traces, assessments, sampled
outcomes, and search details. Logging is presentation logic owned by the
example binaries; it is never part of problem state or transition validity.

The separate long-horizon throughput probe is intentionally a Cargo bench
target, not a consumer example:

```console
cargo bench -p axionomy-problems --bench rollout_throughput
```

## 1. Key-door energy maze

Source: `crates/axionomy-problems/src/maze.rs`

### Specification

The agent starts at `Start` with nine energy. The world owns directed edge
facts, a key in `KeyRoom`, a locked door, a target at `Exit`, and one encoded
distance estimate per node.

Two routes exist:

```text
Key route:    Start --2--> KeyRoom --2--> Door --2--> Exit
Detour route: Start --4--> Detour --5--> Exit
```

The key route also requires `TakeKey` and `UnlockDoor`. The detour therefore
has fewer exchanges, while the key route spends less energy.

Movement consumes `At(from)` and energy, preserves `Edge(from,to)` and any
door permission, and produces `At(to)` plus `SpentEnergy`. A final rate
preserves `At(Exit)` and the target fact and produces `Solved`.

### Required results

- BFS chooses the three-exchange detour.
- Dijkstra and A* choose the six-energy key route.
- Dijkstra and A* agree on cost.
- Every trace replays to `Solved`.

### API pressure

- Structured graph facts without an external graph state.
- Preserved topology and permissions.
- Objective and admissible heuristic values encoded as assets.
- One state graph served by multiple traversal strategies.

## 2. Linear Sokoban

Source: `crates/axionomy-problems/src/sokoban.rs`

### Specification

Five cell accounts each hold exactly one of `Player`, `Crate`, or `Empty`.
One cell also owns `GoalCell`. A move atomically exchanges player and empty
tokens between two adjacent cells. A push atomically rewrites three accounts:

```text
[Player][Crate][Empty] → [Empty][Player][Crate]
```

The solvable instance requires two pushes. The deadlocked instance places the
crate against a wall from which no legal push can move it to the goal.

### Required results

- BFS produces two pushes and a finish exchange.
- Every cell retains exactly one occupancy token.
- The deadlocked instance returns no solution.
- Failed search leaves the source economy unchanged.

### API pressure

- Atomic effects over three locations.
- Explicit emptiness rather than a hidden absence test.
- Spatial invariants and infeasibility.

## 3. Exact cover

Source: `crates/axionomy-problems/src/exact_cover.rs`

### Specification

The universe is `{A,B,C,D}`. Four available subsets are encoded:

```text
AB, CD, AC, BD
```

Selecting a subset consumes its availability token and every corresponding
`Uncovered(element)` token. It produces `Selected(set)`,
`Covered(element)`, and advances an explicit progress token. Overlapping
subsets become inapplicable because they require already-consumed uncovered
tokens.

### Required results

- Generic BFS finds a cover.
- Algorithm X reads subset membership from rate consumption baskets.
- Algorithm X emits ordinary exchanges and validates them by replay.
- Both traces have two selections plus finish.
- Both solvers reject an instance containing only `AB` and `AC`.

### API pressure

- Constraint satisfaction through resource-sensitive facts.
- Specialized solver compilation from core data.
- Solver output translated back to the shared execution language.

## 4. Stoichiometric workshop

Source: `crates/axionomy-problems/src/workshop.rs`

### Specification

The workshop owns wood, labor, and one reusable tool. It may fire:

```text
Basic:     2 Wood + 1 Labor --Tool--> 1 Chair + 1 Waste
Efficient: 3 Wood + 2 Labor --Tool--> 2 Chair + 1 Waste
```

Labor becomes `SpentLabor`. The tool is preserved. Material and labor
accounting are global linear invariants.

The rate book deliberately includes a malformed proposal:

```text
1 Wood → 2 Chair
```

### Required results

- Waste minimization chooses the efficient batch with waste 1.
- BFS also reaches the two-chair goal.
- The malformed rate is rejected by the material invariant.
- Rejection is atomic.

### API pressure

- Identity-changing production rather than literal asset transfer.
- Catalysts/read arcs.
- Domain-declared conservation dimensions.
- Core enforcement even when the rate proposer is wrong.

## 5. Discrete job-shop scheduling

Source: `crates/axionomy-problems/src/scheduling.rs`

### Specification

Two jobs each have two ordered operations:

```text
Job 1: M1 for 2 slots → M2 for 1 slot
Job 2: M2 for 2 slots → M1 for 1 slot
```

Every `(machine,time)` pair is an account initially holding `Available`.
Scheduling consumes a job's `ReadyAt` token and every required slot's
availability, then produces `Reserved(operation)` and the successor readiness
or final completion token.

Finish preserves both completion tokens and produces `Makespan(n)` and
`Solved`.

### Required results

- Generic best-first search finds makespan 3.
- An independent depth-first branch optimizer also finds makespan 3.
- The independent proposal replays through the core.
- A horizon of two slots is infeasible for both algorithms.

### API pressure

- Atomic reservation of variable numbers of accounts.
- Precedence represented as tokens, not solver-only constraints.
- Optimization objective encoded in final state.
- Concrete-rate explosion across ready/start times.

## 6. Stochastic rescue

Source: `crates/axionomy-problems/src/rescue.rs`

### Specification

The agent begins at base with energy and one sensor. An unresolved Nature
account owns `Unresolved` and a user-provided
`ScenarioWeight(truth,seed)` prior. A sampling exchange preserves the selected
weight, consumes `Unresolved`, and produces private `Truth(North|South)` and
`Seed(0..3)` assets. An observation exchange then:

- Preserves the agent at base.
- Consumes the sensor.
- Preserves Nature's matching truth.
- Advances the seed.
- Produces an agent belief.

Seed zero reports the wrong location; the other three seeds report correctly.
A rescue exchange succeeds only when the agent's location and Nature's truth
match.

### Required results

- An agent view cannot inspect the Nature account.
- A chosen Nature observation is recorded as an exchange.
- A successful sampled rollout, including Nature instantiation, replays from
  the unresolved weighted model.
- Over eight bounded scenarios, observe-then-follow succeeds six times while
  a north-only policy succeeds four.
- Monte Carlo selects observe-then-follow.

### API pressure

- Restricted observation without a second state model.
- Hidden truth and belief as different account holdings.
- Reproducible priors and chance through an encoded Nature participant.
- Monte Carlo over isolated forks.

## 7. Single-lane bridge negotiation

Source: `crates/axionomy-problems/src/bridge.rs`

### Specification

Two agents begin west of a bridge with energy, credits, and bidding status.
The bridge owns one `CapacityFree` token.

Two mechanisms are encoded:

- First-come consumes capacity and gives one agent `CrossingRight`.
- Auction submission escrows credits; one atomic resolution consumes both
  bids and capacity, charges the winner, refunds the loser, and produces
  `CrossingRight` plus `Waiting`.

Crossing returns the bridge's capacity token. A waiting agent can then receive
the right. Finish requires both agents to hold `Crossed`.

### Required results

- First-come and auction proposals both replay to the same goal.
- A bid of A=2 and B=1 resolves to A.
- A second claim while capacity is held is rejected.
- Rejection leaves every account unchanged.

### API pressure

- Multi-agent proposals and atomic joint resolution.
- Escrow and resource accounting.
- Alternative mechanisms over one state vocabulary.
- Capacity as a conserved asset.

## 8. Multi-party marketplace settlement

Source: `crates/axionomy-problems/src/marketplace.rs`

### Specification

Three buyer accounts, three seller accounts, and two carrier accounts form a
bounded market. Participant identity comes from account IDs; readiness comes
only from account assets. Two buyers hold enough money and a purchase intent,
two sellers hold a widget and sale offer, and one carrier holds shipping
capacity. A third buyer is short 25 money, a third seller lacks one widget,
and a second carrier lacks one unit of capacity.

The single generic settlement rate binds six distinct roles:

```text
Buyer pays 100 and receives widget + receipt
Seller provides widget + offer and receives 80
Platform preserves its license and receives 5
Tax authority preserves its policy and receives 10
Carrier spends one capacity and receives 5
Order book converts OpenOrder into SettledOrder
```

Money, widgets, buyer and seller lifecycle tokens, shipping capacity, and
order status are protected by declared invariants. Candidate enumeration
derives the buyer, seller, and carrier sets from economy accounts and produces
ordinary exchanges. Exact matching uses core applicability. Near matching
uses complete core shortfalls plus a caller-provided valuation; that scalar is
disposable policy and never changes settlement law or feasibility.

### Required results

- The account-derived Cartesian set contains 18 candidate exchanges.
- Four candidates are exact matches and filtering does not mutate the market.
- One candidate reports buyer, seller, and carrier shortfalls together.
- A successful exchange settles six accounts atomically.
- Projected assessment deltas exactly equal the resulting receipt deltas.
- Failed settlement preserves every account.
- Changing caller-owned shortfall weights changes near-match ranking without
  changing validity.
- Settlement reaches the encoded `SettledOrder` goal and replays
  deterministically.

### API pressure

- Atomic settlement beyond bilateral exchange, including tax, commission, and
  fulfillment.
- Complete multi-account distance to feasibility as matching information.
- Separation between authoritative economic truth and disposable ranking
  policy.
- Finite role-binding enumeration derived from account capabilities.
- Ergonomic all-distinct role constraints for larger joint transactions.
- Conservation and lifecycle invariants across heterogeneous participants.

## 9. Stochastic delivery logistics

Source: `crates/axionomy-problems/src/logistics.rs`

### Specification

One vehicle must deliver four identified packages. Orders encode waiting,
in-transit, and delivered state. The vehicle encodes its location, cargo
capacity, package custody, fuel, money, repair tool, elapsed time, and
remaining deadline. A fuel station holds replenishment stock.

Two route policies are available:

- Direct routes use fewer exchanges but have encoded clear, delay, and
  breakdown weights of 2:1:1.
- Reliable routes use two legs each way with weights 8:1:1 per leg.

Departure converts an encoded location into a traveling-route state. Nature
then resolves one weighted outcome exchange. Clear or delayed arrival consumes
route-specific fuel and time; breakdown consumes time through a repair
exchange and returns the route to Nature for another resolution. Loading,
delivery, refueling, travel, repair, and order completion are all rates.

### Required results

- A reliable-policy trajectory delivers all four orders and exceeds forty
  exchanges.
- Every sampled travel outcome appears as a Nature exchange.
- The complete long trajectory replays from the initial economy.
- Fuel, money, time, cargo, order lifecycle, and position invariants hold.
- Generic Monte Carlo compares both policies over 64 seeded rollouts.
- Encoded reliability makes the reliable policy's mean utility higher.
- A repeatable workload reports rollouts and exchanges per second.

### API pressure

- Long-horizon stochastic trajectories rather than one-step decisions.
- Recurrent Nature outcomes and retry loops.
- Outcome distributions, mean values, and risk statistics.
- Efficient forks sharing immutable laws and untouched account data.
- Candidate generation from current route, custody, and resource assets.

## 10. Adversarial Connect Four

Source: `crates/axionomy-problems/src/connect_four.rs`

### Specification

A compact four-by-four Connect Four board has one account per cell and one
account per column. Cell occupancy, gravity progress, alternating turns, and
both players' counts for every row, column, and diagonal are assets.

Move rates consume the exact current line counters and produce their
successors. A move completing a line consumes the current turn and produces
`Winner(player)` directly. A full board with no winner enables a draw rate
that preserves all four `ColumnFull` facts. Terminal truth therefore never
comes from an external board callback.

Generic vector-valued MCTS uses the encoded current turn to choose which
player value to maximize. Tree edges are exchanges, transpositions use
canonical state keys, rollouts use seeded action selection, and the final
action is revalidated by the live economy.

### Required results

- Gravity places a first piece in row zero and advances the column token.
- Line counters advance atomically with cell and turn state.
- MCTS selects an immediately winning fourth column.
- Applying that proposal produces the encoded winner asset.
- Complete self-play terminates in a win or encoded draw.
- The full game trace replays deterministically.

### API pressure

- Adversarial vector outcomes and actor-relative tree selection.
- Large derived search trees over a compact authoritative state.
- Core-encoded terminal detection without solver-only win logic.
- Canonical transpositions and bounded seeded exploration.
- Concrete-rate growth from structured move schemas.

## 11. Partially observed team mission

Source: `crates/axionomy-problems/src/mission.rs`

### Specification

Scout and Medic accounts begin at base while Nature holds unresolved weighted
truth, sensor seed, and hazard state. Neither agent view can inspect Nature or
the other agent's private account.

The Scout may begin a scan without naming hidden truth. That public exchange
creates `AwaitingScan`; Nature then resolves the scan through the uniquely
applicable truth-and-seed-specific exchange, producing fallible private
intelligence. A share exchange transfers that intelligence to the Medic while
preserving explicit evidence for the Scout. Both agents then move jointly. At
the true site, Nature resolves the encoded safe or injury hazard. Injury
requires an atomic treatment exchange using the Medic's kit before the team
can rescue the victim. Every public mission action consumes encoded deadline
time except the final goal-marking exchange.

The coordinated policy scans, shares, and follows the Medic's visible
intelligence. A direct policy always sends both agents north. Replayed traces
are also projected into observation, outcome, and termination transitions for
learning.

Observation-scoped ISMCTS receives only the Scout's canonical information
identity when generating decisions or choosing rollout actions. Each iteration
samples a complete encoded scenario from Nature's asset-held prior, rejects a
sample inconsistent with the root observation, and merges tree statistics by
information state rather than hidden world state. Nature resolution may inspect
the sampled world, but only to propose concrete encoded outcome exchanges. The
selected live action is core-revalidated.

### Required results

- Agent views hide Nature and the other agent.
- Two worlds with different hidden truths have equal Scout observation keys,
  equal public proposal sets, and the same seeded ISMCTS decision.
- The initial ISMCTS decision is the public `BeginScan` exchange in either
  hidden world.
- Hidden instantiation, scan resolution, and encounter exchanges never appear
  in the Scout's decision source.
- Intelligence changes ownership only through a share exchange.
- Joint movement, encounter, treatment, and rescue are multi-account rates.
- A coordinated successful trace contains instantiation, scan intent, scan
  resolution, share, encounter, and finish exchanges and replays exactly.
- Across all 16 weighted scenarios, coordination succeeds 12 times and direct
  north succeeds 8.
- The complete trace becomes one RL transition per exchange, ending with an
  encoded terminal outcome.

### API pressure

- Agent-specific observations and information boundaries.
- Information-state identities and observation-safe tree transpositions.
- Belief sampling from encoded priors without giving policies hidden state.
- Lazy public proposal generation followed by core applicability filtering.
- Multi-agent communication as economic state change.
- Hidden truth, belief, hazard, and deadline in one model.
- Generic Monte Carlo over partially observed policies.
- Assessment-derived masks and replay-derived learning trajectories.

## Cross-problem acceptance tests

The repository test suite additionally verifies:

- Exact basket shortfalls.
- Checked scale, addition, withdrawal, and deposit.
- Generic exact `u64` and non-`Copy` `BigUint` economies.
- Direct Serde round trips, canonical zero removal, and duplicate rejection.
- Exact dimension-safe and calendar-aware lowering into self-describing asset
  denominations.
- Schema rejection of duplicate logical IDs, conflicting atomic bases,
  discrete/measured collisions, foreign model keys, and malformed serialized
  definitions.
- Compile-fail proof that a measured handle cannot accept another physical
  dimension, plus shared Jiff/`uom` time identity.
- Multi-role effect merging.
- Required and unknown role errors.
- Missing rate and account errors.
- Distinct-role enforcement.
- Zero-unit rejection.
- Rate-scaling and destination-balance overflow.
- Non-mutating feasibility, simulation, and replay on forks.
- Complete multi-account distance-to-feasibility reports.
- Applicable assessment parity with eventual receipt deltas.
- Receipt deltas for every touched account.
- Rollout goal, cutoff, rejection, retention, and replay behavior.
- Deterministic systematic and seeded weighted sampling.
- Bernoulli, scalar, vector, quantile, and lower-tail statistics.
- Vector-valued MCTS selection, chance nodes, and transpositions.
- Observation identities include visible-account boundaries and balances.
- ISMCTS rejects inconsistent determinizations, passes only information states
  to decision sources and rollout policies, and revalidates its selected
  exchange.
- Lazy action sources remain concrete, duplicate-safe, and core-filtered.
- Compact state fingerprints and isolated shared-data forks.
- Construction-order-independent logical state identities.
- RL action masks, shortfall features, receipts, and trajectory extraction.
- Property laws for assessment/application parity, atomic failure, arithmetic,
  serialization, and unit conversion.

Run the suite with:

```console
cargo test --workspace --all-targets --all-features
```

## What the suite says to build next

The benchmarks support the generalized kernel, but they also show its next
limits clearly:

1. Scheduling and stochastic outcomes need typed parameterized rate schemas to
   avoid eager concrete expansion.
2. Solvers need a standard finite binding enumerator derived from schemas and
   account capabilities.
3. Durable cross-version replay needs an explicit compatibility contract and
   problem/rate versioning beyond current direct Serde support.
4. Search-heavy workloads need a persistent account index and incremental
   state fingerprints beyond the current shared account contents and laws.
5. More domains will require constrained local and inequality invariants.
6. External OR adapters should compile encoded semantics, emit exchanges, and
   use replay as a mandatory proof checker.
7. MCTS and ISMCTS may need PUCT priors, progressive widening, and deterministic
   parallel workers once learned priors, measured branching, or throughput
   justify them.
8. Nature needs richer parameterized distribution updates for very large or
   continuous outcome spaces.

Any future abstraction must continue to pass all eleven problems without
moving authoritative meaning into solver callbacks or an external world.
