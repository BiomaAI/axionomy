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

| Capability | Maze | Sokoban | Exact cover | Workshop | Job shop | Rescue | Bridge |
| --- | :---: | :---: | :---: | :---: | :---: | :---: | :---: |
| Multi-account atomic rewrite | ✓ | ✓ |  |  | ✓ | ✓ | ✓ |
| Preserved facts/catalysts | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| Declared invariants | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| Resource objective | ✓ |  |  | ✓ | ✓ | ✓ | ✓ |
| Infeasible instance |  | ✓ | ✓ |  | ✓ |  |  |
| Specialized proposer |  |  | Algorithm X |  | Branch optimizer | Monte Carlo policy | Auction mechanism |
| Multiple generic strategies | BFS/A*/Dijkstra | BFS | BFS | BFS/best-first | Best-first |  | BFS |
| Hidden or stochastic state |  |  |  |  |  | ✓ |  |
| Multi-agent resolution |  |  |  |  |  |  | ✓ |
| Deterministic replay test | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |

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

## Cross-problem acceptance tests

The repository test suite additionally verifies:

- Exact basket shortfalls.
- Checked scale, addition, withdrawal, and deposit.
- Multi-role effect merging.
- Required and unknown role errors.
- Missing rate and account errors.
- Distinct-role enforcement.
- Zero-unit rejection.
- Rate-scaling and destination-balance overflow.
- Non-mutating feasibility, simulation, and replay on forks.
- Receipt deltas for every touched account.

Run the suite with:

```console
cargo test --workspace --all-targets
```

## What the suite says to build next

The benchmarks support the generalized kernel, but they also show its next
limits clearly:

1. Scheduling and stochastic outcomes need typed parameterized rate schemas to
   avoid eager concrete expansion.
2. Solvers need a standard finite binding enumerator derived from schemas and
   account capabilities.
3. Durable replay needs canonical serialization and problem/rate versioning.
4. Search-heavy workloads need persistent or copy-on-write forks.
5. More domains will require constrained local and inequality invariants.
6. External OR adapters should compile encoded semantics, emit exchanges, and
   use replay as a mandatory proof checker.
7. Nature needs a standardized deterministic weighted-sampling protocol and
   richer encoded distribution updates.

Any future abstraction must continue to pass all seven problems without
moving authoritative meaning into solver callbacks or an external world.
