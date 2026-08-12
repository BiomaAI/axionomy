# Axionomy Closed-Problem Conformance Suite

This document specifies the problem set used to evolve and test Axionomy's
API. Each problem has a small proof fixture and a materially larger default
workload. The suite is deliberately different enough to expose distinct
architectural pressure without confusing raw size with semantic difficulty.

The suite is successful only when a problem is genuinely closed:

1. All domain state is held as assets in accounts.
2. Every domain transition is a rate firing.
3. Every concrete action is an exchange with explicit role bindings.
4. Goals are asset configurations.
5. Costs, observations, chance state, and constraints are encoded.
6. Solver-side structures are derived, disposable caches.
7. A proposed solution is accepted only after core replay.

Rust enums and builder loops define each concrete fixture's vocabulary and
construct its initial closed problem. They are not mutable parallel world
state. These modules are examples and executable conformance tests, not public
maze, scheduling, market, or game frameworks. Users define their own
economies; the examples exist to prove that the common axioms survive very
different domains and to expose unnecessary friction in the generic engine.

A solver may use an adapter to translate a rate ID into proposed role
bindings, but the bindings become part of the exchange and the rate remains
the authority for validity and effects. A field in `RateId` is descriptive,
not authorization. When a role must represent a particular cell, slot,
participant, institution, or result account, the rate requires preserved
identity or capability assets from that role. Adversarial tests deliberately
rebind roles to ensure the core rejects domain-invalid exchanges even when
they bypass the example's trusted action helper.

## Conformance matrix

| Capability | Maze | Sokoban | Exact cover | Workshop | Job shop | Rescue | Bridge | Marketplace | Logistics | Connect Four | Mission | Perishables | Work League |
| --- | :---: | :---: | :---: | :---: | :---: | :---: | :---: | :---: | :---: | :---: | :---: | :---: | :---: |
| Multi-account atomic rewrite | ✓ | ✓ |  |  | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| Preserved facts/catalysts | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| Declared invariants | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |  |
| Resource objective | ✓ |  |  | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |  | ✓ | ✓ | ✓ |
| Exact Pareto front | ✓ |  |  | ✓ | ✓ |  | ✓ | ✓ |  |  |  | ✓ |  |
| Approximate or derived Pareto comparison |  |  |  |  |  | ✓ |  |  | ✓ |  | ✓ |  | ✓ |
| Infeasible instance |  | ✓ | ✓ |  | ✓ |  |  | ✓ | ✓ |  | ✓ | ✓ |  |
| Specialized proposer |  |  | Algorithm X |  | Branch optimizer | Scenario/MC evaluator | Auction | Assessment clearing | Monte Carlo/MCTS | MCTS/minimax oracle | ISMCTS/scenario evaluator | Event agenda/index | Competing seeded policies |
| Generic algorithm | BFS/A*/Dijkstra/Pareto | BFS | BFS | BFS/best-first/Pareto | Best-first/Pareto | Rollout/MC/Pareto | BFS/Pareto | Pareto | Rollout/MC/Pareto | MCTS | ISMCTS/Rollout/MC/Pareto/RL | Pareto | Replay-derived vector comparison |
| Hidden or stochastic state |  |  |  |  |  | ✓ |  |  | ✓ |  | ✓ |  | ✓ |
| Multi-agent resolution |  |  |  |  |  |  | ✓ | ✓ |  | ✓ | ✓ |  | ✓ |
| Long-horizon trajectory |  |  |  |  |  |  |  |  | ✓ | ✓ |  | ✓ | ✓ |
| Per-step leaderboards |  |  |  |  |  |  |  |  |  |  |  |  | ✓ |
| Learning trajectory |  |  |  |  |  |  |  |  |  |  | ✓ |  |  |
| Observation-scoped tree |  |  |  |  |  |  |  |  |  |  | ✓ |  |  |
| Deterministic replay test | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |

## Runnable examples

Each model has a symmetric consumer example. These binaries contain
orchestration and display logic only; all authoritative state, rules, goals,
and constraints remain in that concrete problem economy.

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
cargo run -p axionomy-problems --example work_league
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

## Instance profiles and anti-toy pressure

Every service-visible problem exposes the same explicit profile contract:

- **Micro** preserves the compact exact fixtures and independent oracles used
  by unit tests and quick integration checks.
- **Showcase** is the Studio, CLI, HTTP, and MCP default. It adds decision
  density: longer plans, competing actions, coupled resources, more accounts,
  additional constraints, or broader uncertainty.
- **Stress** increases a problem-appropriate dimension such as board size,
  sample work, scheduling horizon, production target, or fungible inventory.

These are problem instances, not alternate semantics. A profile may change
initial accounts, the closed rate book, and encoded goals, but it never moves
truth into the service or viewer. Instance identity is carried in
`RunRequest` and `RunArtifact`, so a result cannot be compared or replayed as
though it came from a different workload.

The committed Showcase artifacts currently apply this pressure. This table is
also the source for the per-problem Showcase descriptions in the service
catalog, so those two public explanations must change together:

| Problem | Default Showcase pressure |
| --- | --- |
| Maze | 14 nodes, 16 directed edges, four energy/time route families, and an eight-transition low-energy plan |
| Sokoban | 7×5 board, 35 cell accounts, two-dimensional repositioning, and a ten-transition solution |
| Exact cover | 8 universe elements, 12 competing subsets, four selections, and an independently proposed Algorithm X trace |
| Workshop | Six-chair multi-batch target with seven-step fast and four-step efficient extremes |
| Job shop | Six precedence-constrained operations across three machines and 18 capacity slots |
| Rescue | Four hidden sites, 32 encoded Nature scenarios, noisy sensing, contact, and return evacuation |
| Bridge | Two consecutive auctions/allocations with escrow, recharge, atomic round reset, and fairness tradeoffs |
| Marketplace | Four coupled orders across 14 accounts with shared budgets, inventory, shipping capacity, taxes, and commissions |
| Logistics | Four deliveries, recurrent weather and breakdowns, refueling/repair loops, up to 53 accepted transitions, Monte Carlo, and MCTS |
| Connect Four | Standard 7×6 board and 69 four-cell win certificates; 226 concrete rates replace the old 1,282-rate 4×4 projection |
| Mission | 16 hidden joint scenarios, private observations, belief filtering, information exchange, hazard response, Monte Carlo, and ISMCTS |
| Perishables | 10,000 fungible claims, cohort-level decay, refrigeration, outage effects, indexed deadlines, and an eight-point storage frontier |
| Work League | Four autonomous workers, 12 finite jobs, eight locations, shared repair/recycling facilities, seeded failures, 60+ atomic transitions, and six replay-derived standings |

**Model size** telemetry records accounts, rules, trace steps, rule-check
probes, and compared alternatives for every result. The service test also
enforces per-problem minimum pressure (and a
maximum compact-rate bound for standard Connect Four), preventing future
refactors from silently collapsing Showcase back into a toy.

The detailed specifications below describe the Micro conformance contract
unless a larger profile is named explicitly. Showcase builds on those same
laws; it does not replace the exact fixtures that make correctness auditable.

## 1. Key-door energy maze

Source: `crates/axionomy-problems/src/maze.rs`

### Specification

The agent starts at `Start` with nine energy and six time. The world owns directed edge
facts, a key in `KeyRoom`, a locked door, a target at `Exit`, and one encoded
distance estimate per node.

Two routes exist:

```text
Key route:    Start --2--> KeyRoom --2--> Door --2--> Exit
Detour route: Start --4--> Detour --5--> Exit
```

The key route also requires `TakeKey` and `UnlockDoor`. The detour therefore
has fewer exchanges, while the key route spends less energy.

Every semantic action consumes time and produces `SpentTime`. Movement also
consumes `At(from)` and energy, preserves `Edge(from,to)` and any door
permission, and produces `At(to)` plus `SpentEnergy`. Search cost is
derived from the before/after `SpentEnergy` delta rather than copied from the
rate identifier. A final rate consumes the maze's `Active` lifecycle asset and
produces `Solved`, leaving no post-terminal moves applicable.

### Required results

- BFS chooses the three-exchange detour.
- Dijkstra and A* choose the six-energy key route.
- Dijkstra and A* agree on cost.
- Exact Pareto search retains `(energy=6,time=6)` and `(energy=9,time=3)`;
  neither route is hidden behind one scalar weight.
- Every trace replays to `Solved`.
- Actor/environment role swapping is rejected by encoded requirements.
- A solved maze has no applicable actions.

### API pressure

- Structured graph facts without an external graph state.
- Preserved topology and permissions.
- Objective and admissible heuristic values encoded as assets.
- One state graph served by multiple traversal strategies.
- Exact multi-objective comparison without weighted-sum scalarization.

## 2. Sokoban

Source: `crates/axionomy-problems/src/sokoban.rs`

### Specification

Five cell accounts each preserve their coordinate identity and hold exactly
one of `Player`, `Crate`, or `Empty`.
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
- Rebinding a legal move rate to nonadjacent cells is rejected.
- Completion consumes the active puzzle lifecycle and is quiescent.

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

Selecting a subset preserves the active problem lifecycle, consumes its availability token and every corresponding
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
- All 16 availability combinations agree with a direct exhaustive oracle.
- Finish requires all four encoded `Covered` facts, then consumes `Active`.

### API pressure

- Constraint satisfaction through resource-sensitive facts.
- Specialized solver compilation from core data.
- Solver output translated back to the shared execution language.

## 4. Stoichiometric workshop

Source: `crates/axionomy-problems/src/workshop.rs`

### Specification

The workshop owns wood, labor, three process-time units, and one reusable tool.
It may fire:

```text
Basic:     2 Wood + 1 Labor + 1 Time --Tool--> 1 Chair + 1 Waste
Efficient: 3 Wood + 2 Labor + 3 Time --Tool--> 2 Chair + 1 Waste
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
- Exact Pareto search retains the efficient `(waste=1,time=3)` batch and the
  two-basic `(waste=2,time=2)` plan, and every retained trace replays.
- The malformed rate is rejected by the material invariant.
- Rejection is atomic.
- Completion consumes `Active`; no recipe remains applicable afterward.

### API pressure

- Identity-changing production rather than literal asset transfer.
- Catalysts/read arcs.
- Domain-declared conservation dimensions.
- Core enforcement even when the rate proposer is wrong.
- Process tradeoffs remain terminal assets and replayable plans.

## 5. Discrete job-shop scheduling

Source: `crates/axionomy-problems/src/scheduling.rs`

### Specification

Two jobs each have two ordered operations:

```text
Job 1: M1 for 2 slots → M2 for 1 slot
Job 2: M1 for 2 slots → M2 for 1 slot
```

Every `(machine,time)` pair is an account preserving
`SlotIdentity(machine,time)` and initially holding `Available`.
Scheduling consumes a job's `ReadyAt` token and every required slot's
availability, then produces `Reserved(operation)` and the successor readiness
or final completion token.

Finish preserves both completion tokens and produces `Makespan(n)` and
`Solved`.

### Required results

- Generic best-first search finds makespan 5.
- A separate depth-first branch strategy also finds makespan 5.
- Its proposal replays through the core.
- A horizon of two slots is infeasible for both algorithms.
- Horizons zero through five agree with a direct job-shop brute-force oracle.
- Rebinding an operation to the wrong machine or time slot is rejected.
- Exact Pareto search retains the two completion allocations `(3,5)` and
  `(5,3)`, making the shared machine's stakeholder tradeoff explicit.

### API pressure

- Atomic reservation of variable numbers of accounts.
- Precedence represented as tokens, not solver-only constraints.
- Optimization objective encoded in final state.
- Concrete-rate explosion across ready/start times.
- Stakeholder completion allocation without a privileged scalar compromise.

## 6. Stochastic rescue

Source: `crates/axionomy-problems/src/rescue.rs`

### Specification

The agent begins at base with energy and one sensor. An unresolved Nature
account owns `Unresolved` and a user-provided
`ScenarioWeight(truth,seed)` prior. A sampling exchange preserves the selected
weight, consumes `Unresolved`, and produces private `Truth(North|South)` and
`Seed(0..3)` assets. The policy first proposes a truth-independent public
`BeginObserve` exchange, which consumes `Planning` and the sensor and produces
`AwaitingObservation`. Nature then fires the uniquely applicable hidden
`ResolveObservation` exchange, which:

- Preserves the agent at base.
- Consumes `AwaitingObservation` and restores `Planning`.
- Preserves Nature's matching truth.
- Advances the seed.
- Produces an agent belief.

Seed zero reports the wrong location; the other three seeds report correctly.
A rescue exchange succeeds only when the agent's location and Nature's truth
match.

### Required results

- An agent view cannot inspect the Nature account.
- Equal agent views under different truths choose the same public observation
  intent.
- Public intent and Nature's chosen resolution are separate recorded exchanges.
- A successful sampled rollout, including Nature instantiation, replays from
  the unresolved weighted model.
- Across all eight encoded scenarios, observe-then-follow succeeds six times while
  a north-only policy succeeds four.
- Exact scenario evaluation selects observe-then-follow.
- A separate seeded Monte Carlo entry point draws randomly from the same
  encoded weighted prior and is reproducible.
- A sampled Pareto front retains the higher-success sensor policy and the
  sensor-free direct policy, and is explicitly labeled approximate.

### API pressure

- Restricted observation without a second state model.
- Hidden truth and belief as different account holdings.
- Reproducible priors and chance through an encoded Nature participant.
- Exact finite-support evaluation and genuine seeded Monte Carlo over isolated
  forks.
- Approximate multi-objective estimates keep sampling uncertainty visible.

## 7. Single-lane bridge negotiation

Source: `crates/axionomy-problems/src/bridge.rs`

### Specification

Two agents begin west of a bridge with energy, credits, and bidding status.
The bridge owns one `CapacityFree` token and an encoded first/second turn.

Two mechanisms are encoded:

- First-come consumes first-turn capacity and gives one agent
  `CrossingRight` plus `PriorityBenefit`.
- Auction submission escrows credits; one atomic resolution consumes both
  bids and capacity, charges the winner, refunds the loser, and produces
  `CrossingRight` plus `Waiting`.

Crossing returns the bridge's capacity token and enables the second turn. A
waiting or first-come agent can then receive the second right. Finish requires
both agents to hold `Crossed`.

Every participant and the bridge preserve identity assets. Agent names in
rate identifiers therefore cannot be used to impersonate another account or
swap the encoded auction winner. Finish consumes the bridge's active lifecycle
asset, making the terminal economy quiescent.

### Required results

- First-come and auction proposals both replay to the same goal.
- A bid of A=2 and B=1 resolves to A.
- A second claim while capacity is held is rejected.
- Bidder impersonation and winner/loser rebinding are rejected.
- Rejection leaves every account unchanged.
- Exact Pareto search retains both possible priority allocations with both
  agents' credits intact; auction outcomes with the same priority but spent
  credit are correctly dominated under the encoded objectives.

### API pressure

- Multi-agent proposals and atomic joint resolution.
- Escrow and resource accounting.
- Alternative mechanisms over one state vocabulary.
- Capacity as a conserved asset.
- Dominance can reject an economically wasteful mechanism without rejecting
  its core validity.

## 8. Multi-party marketplace settlement

Source: `crates/axionomy-problems/src/marketplace.rs`

### Specification

Three buyer accounts, three seller accounts, two carrier accounts, and two
order accounts form a bounded market. Participant identity comes from account
IDs; eligibility comes only from account assets. Order A requests a widget for
100 and Order B requests a gadget for 90. Buyer A can fund both, Buyer B can
fund Order A, and Buyer C is 25 short on Order A. Two sellers offer widgets,
one offers a gadget, Carrier A owns two units of capacity, and Carrier B has no
capacity.

Each order-specific settlement rate binds the same six distinct roles:

```text
Buyer pays 100 and receives widget + receipt
Seller provides widget + offer and receives 80
Platform preserves its license and receives 5
Tax authority preserves its policy and receives 10
Carrier spends one capacity and receives 5
Order account converts matching OpenOrder into SettledOrder + SettledValue
```

The settlement terms also produce declared `Utility` assets for the bound
buyer and seller. They make participant benefit part of the outcome rather
than a clearing callback.

Money, widgets, buyer and seller lifecycle tokens, shipping capacity, and
order status are protected by declared invariants. Candidate enumeration
derives the buyer, seller, and carrier sets from economy accounts and produces
ordinary exchanges. Exact matching uses core applicability. Near matching
uses complete core shortfalls plus a caller-provided valuation; that scalar is
disposable policy and never changes settlement law or feasibility. A
disposable clearing search explores compatible applicable settlements,
maximizing encoded settled-order count and gross value. Its result is an
ordinary replayable exchange trace, not a second market state.

### Required results

- The account-derived Cartesian set contains 36 candidate exchanges.
- Five candidates are initially exact and filtering does not mutate the market.
- One candidate reports buyer, seller, and carrier shortfalls together.
- A successful exchange settles six accounts atomically.
- Projected assessment deltas exactly equal the resulting receipt deltas.
- Failed settlement preserves every account.
- Changing caller-owned shortfall weights changes near-match ranking without
  changing validity.
- Clearing selects two compatible settlements with gross value 190.
- The two-exchange clearing reaches both encoded order goals and replays
  deterministically.
- Exact Pareto clearing retains four participant-utility allocations covering
  the two eligible Order A buyers and two widget sellers; every entry replays.

### API pressure

- Atomic settlement beyond bilateral exchange, including tax, commission, and
  fulfillment.
- Complete multi-account distance to feasibility as matching information.
- Separation between authoritative economic truth and disposable ranking
  policy.
- Finite role-binding enumeration derived from account capabilities.
- Global compatibility search over sequential multi-order settlement.
- Ergonomic all-distinct role constraints for larger joint transactions.
- Conservation and lifecycle invariants across heterogeneous participants.
- Participant-level allocation fronts without an external market state.

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
- Mean and lower-decile selection both use caller-chosen projections of the
  same sampled encoded outcomes.
- MCTS selects a live applicable route while deriving chance branches from
  Nature's encoded weights.
- A vector Monte Carlo evaluation exposes completion probability, mean
  deliveries, and mean elapsed time as an explicitly approximate policy front.
- A repeatable workload reports rollouts and exchanges per second.

### API pressure

- Long-horizon stochastic trajectories rather than one-step decisions.
- Recurrent Nature outcomes and retry loops.
- Outcome distributions, mean values, and risk statistics.
- Disposable MCTS route planning over the same transition system.
- Efficient forks sharing immutable laws and untouched account data.
- Candidate generation from current route, custody, and resource assets.
- Sampled dominance may remove a policy while preserving its underlying
  encoded rollouts for audit and alternative statistics.

## 10. Adversarial Connect Four

Source: `crates/axionomy-problems/src/connect_four.rs`

### Specification

A compact four-by-four Connect Four board has one account per cell and one
account per column. Every game, result, column, and cell account preserves an
encoded identity. Cell occupancy, gravity progress, alternating turns, and
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
- Rebinding a move to another empty cell or column is rejected by identities.
- MCTS selects an immediately winning fourth column.
- A plain-board minimax oracle independently agrees on that winning move.
- Generated legal prefixes through depth five agree with the plain-board
  oracle's legal actions and terminal values.
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
preserving explicit evidence for the Scout. A scanned team can make the
coordinated move only when the Scout preserves shared intelligence and the
Medic preserves the received intelligence. A direct pre-scan commitment is a
separate rate that requires the Scout's still-unused sensor. At
the true site, Nature resolves the encoded safe or injury hazard. Injury
requires an atomic treatment exchange using the Medic's kit before the team
can rescue the victim. Every public mission action consumes encoded deadline
time except the final goal-marking exchange.

The coordinated policy scans, shares, and follows the Medic's visible
intelligence. A direct policy always sends both agents north. Replayed traces
are also projected into observation, outcome, and termination transitions for
learning.

Observation-scoped ISMCTS receives only the Scout's canonical information
identity when generating decisions or choosing rollout actions. The caller
supplies complete encoded belief worlds; the planner retains only worlds
consistent with the live observation and samples among them. After a public
action and required Nature response, caller-owned belief worlds are advanced,
filtered to the new observation, and passed back to the same planner. Tree
statistics merge by information state rather than hidden world state. Nature
resolution may inspect the sampled world, but only to propose concrete encoded
outcome exchanges. The selected live action is core-revalidated.

### Required results

- Agent views hide Nature and the other agent.
- Two worlds with different hidden truths have equal Scout observation keys,
  equal public proposal sets, and the same seeded ISMCTS decision.
- The initial ISMCTS decision is the public `BeginScan` exchange in either
  hidden world.
- Hidden instantiation, scan resolution, and encounter exchanges never appear
  in the Scout's decision source.
- Intelligence changes ownership only through a share exchange.
- Role identities reject scan results, injuries, or intelligence bound to the
  wrong agent.
- After observing, the caller filters 16 initial belief worlds to a posterior
  and replans; the next selected action is the now-causal share exchange.
- Joint movement, encounter, treatment, and rescue are multi-account rates.
- A coordinated successful trace contains instantiation, scan intent, scan
  resolution, share, encounter, and finish exchanges and replays exactly.
- Across all 16 weighted scenarios, coordination succeeds 12 times and direct
  north succeeds 8.
- The complete trace becomes one RL transition per exchange, ending with an
  encoded terminal outcome.
- A sampled Pareto front retains the reliability/time/medical-use tradeoff and
  remains explicitly approximate.

### API pressure

- Agent-specific observations and information boundaries.
- Information-state identities and observation-safe tree transpositions.
- Belief sampling from encoded priors without giving policies hidden state.
- Lazy public proposal generation followed by core applicability filtering.
- Multi-agent communication as economic state change.
- Hidden truth, belief, hazard, and deadline in one model.
- Exact 16-scenario comparison plus genuine seeded Monte Carlo over partially
  observed policies.
- Caller-owned belief evolution and repeated information-set planning.
- Assessment-derived masks and replay-derived learning trajectories.
- Approximate fronts provide set-valued policy supervision without claiming
  exact hidden-world performance.

## 12. Cohort-indexed perishables

Source: `crates/axionomy-problems/src/perishables.rs`

### Specification

Warehouse and fridge accounts hold ten thousand fungible fruit claims in two
cohorts. The claim quantity is stable ownership or custody state; it does not
duplicate freshness on every unit. Each cohort account instead holds one
unique condition fact:

```text
Warehouse:       Claim(Ambient) = 7,000, Ambient
Fridge:          Claim(Refrigerated) = 3,000, Cold, Powered, CoolingEnergy = 7,000
Ambient cohort:  Fresh(AmbientExpiry) = 1
Cold cohort:     Fresh(ColdExpiry) = 1
```

A single exchange with multiplicity 1,000 moves claims from the ambient cohort
to the refrigerated cohort while preserving both location identities, both
cohort identities, cold power, freshness, and the open transfer window. It
also converts `CoolingEnergy` into `SpentCoolingEnergy`; it does not fire one
thousand transitions.

Time is explicit economic state. The World account owns exactly one `Now`
fact, plus complementary `Before(deadline)` and `Reached(deadline)` facts.
Advancing the clock consumes the applicable `Before` fact and produces its
`Reached` fact. Consequently fruit cannot spoil early, but it also cannot be
eaten or moved after expiry while a simulator is still materializing due
effects.

At ambient expiry, one spoil exchange changes the ambient cohort's unique
fresh condition into `Rotten`; its six thousand claims are untouched. A power
loss then atomically changes the fridge from cold/powered to
ambient/unpowered and reclassifies the refrigerated cohort from cold expiry to
an earlier warmed-expiry condition. The event agenda schedules that newly
produced condition. Its old cold-expiry candidate remains in the disposable
queue but is later rejected as infeasible because the authoritative condition
and environment no longer exist.

The `ClaimIndex` is a receipt-maintained `cohort → account → quantity`
projection. It provides holder and aggregate-supply queries without becoming
truth: it can be rebuilt from the economy, and tests compare the incremental
index to a full rebuild. The `EffectAgenda` similarly indexes fresh condition
facts and only proposes ordinary spoil exchanges.

### Required results

- Ten thousand claims occupy the same number of state entries as ten claims.
- A 1,000-unit move is one scaled multi-account exchange.
- Each cohort condition has unit supply; multi-unit or repeated spoilage is
  infeasible.
- Before/reached facts reject early decay and late consumption independently
  of event-loop discipline.
- Power loss touches no claim balance and changes one shared cohort fact.
- The newly warmed cohort spoils at its earlier deadline.
- The queued cold event becomes stale and is rejected by core assessment.
- Incorrect location, cohort, or World bindings are rejected by encoded
  identities and conditions.
- The complete outage trace replays to the encoded goal and becomes quiescent.
- 128 generated inventory splits and transfers agree with a plain independent
  inventory oracle.
- The receipt-maintained holdings index always equals a complete rebuild.
- Bounded exact planning evaluates zero through seven 1,000-claim transfers and
  retains all eight preservation/cooling-energy tradeoffs as replayable traces.

### API pressure

- Structural fungibility through shared asset identity and quantity.
- Non-fungible facts through unique identity, unit supply, and invariants.
- Semantic cohort indirection versus disposable physical indexes.
- Event-driven temporal effects without mutation-on-read or an external clock.
- Trigger indexes, stale proposal rejection, and receipt-driven projections.
- Work proportional to shared-fate cohorts rather than physical units.
- Incremental touched-account commit and receipt-delta invariant validation
  without weakening atomicity.
- Bounded policy discretization can coexist with an arbitrary-unit fungible
  exchange rate.

## 13. Autonomous Work League

Source: `crates/axionomy-problems/src/work_league.rs`

### Specification

Four autonomous workers—Atlas, Bolt, Coda, and Delta—contend for twelve jobs
distributed across four work sites and three shared facilities. A mixed field
assigns a different proposal policy to each worker: Sprinter, Steward, Value
Hunter, and Resilient. Policy identity is itself an asset. It may guide the
caller's proposals, but it grants no authority and cannot directly install an
outcome.

Every job account owns a unique identity and one availability token. Claiming
a job atomically consumes availability, assigns the job to one identified
worker, and gives that worker the corresponding claim. Work then requires
location, energy, time, material, operational condition, and the live claim.
Rush, Lean, and Safe modes consume different resource baskets and expose
different encoded success/failure weights. Nature resolves an attempted job
through an ordinary multi-account exchange. Success transfers contract value
and completion assets; failure records the attempt, converts `Operational`
into `Damage`, and returns the still-assigned job to a pending state.

A damaged worker must travel to the workshop and atomically exchange shared
repair supply for restored condition. Steward policies may travel to the
recycler and convert residual waste into recycled waste while consuming time
and energy. Movement, repair, charging, and recycling are all rates; there is
no external vehicle position, job owner, facility lock, resource meter, random
result, or score store.

The Showcase match produces at least sixty replay-verified exchanges. At every
snapshot, the presentation ontology independently derives six standings:

- Contract value.
- Jobs completed per elapsed tick.
- Value per energy-plus-material unit.
- Least residual waste, with no-work agents ineligible.
- Successful jobs per attempt.
- Non-dominance over value, completions, waste, and elapsed time.

Those rankings are intentionally allowed to disagree. Earned value, elapsed
time, resource expenditure, waste, attempts, and completions are authoritative
assets because they affect and result from mechanics. Rank, eligibility,
trend, and Pareto membership are disposable comparisons over the selected
replayed snapshot.

### Required results

- Showcase contains four agents, twelve finite jobs, eight locations, and at
  least fifty accepted exchanges; Stress doubles the job pool and materially
  lengthens the replay.
- Every job is claimed once and every terminal job account satisfies the
  encoded completion goal.
- Agent and job identity assets reject rebinding a valid-looking claim to the
  wrong participant.
- Failure, repair, retry, recycling, and shared-facility effects replay through
  ordinary atomic exchanges.
- Energy, material, time, worker condition, and repair supply invariants hold
  across the complete match.
- Every snapshot carries all six leaderboards, and at least two different
  agents lead the final set of objectives.
- Native SSE and browser Wasm publish each derived frame with the same
  leaderboard state retained by the completed portable artifact.

### API pressure

- Competitive multi-agent allocation without a privileged scheduler.
- Several honest definitions of winning instead of one universal utility.
- Eligibility, exact ratios, ties, rank changes, and Pareto comparison as a
  generic view contract.
- Live frame publication without moving transport state into the economy.
- Shared URLs that restore a precise problem, outcome, replay step, and
  leaderboard on both a server and static GitHub Pages.
- Concrete-rate expansion pressure that strengthens the case for future typed
  parameterized rate schemas without prematurely changing kernel semantics.

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
- Identity- and capability-witnessed role bindings, including adversarial
  rebinding outside trusted action helpers.
- Consumable terminal lifecycle assets and quiescent marker-based goals.
- Zero-unit rejection.
- Rate-scaling and destination-balance overflow.
- Non-mutating feasibility, simulation, and replay on forks.
- Complete multi-account distance-to-feasibility reports.
- Applicable assessment parity with eventual receipt deltas.
- Receipt deltas for every touched account.
- Rollout goal, cutoff, rejection, retention, and replay behavior.
- Deterministic systematic and seeded weighted sampling.
- Bernoulli, scalar, vector, quantile, and lower-tail statistics.
- Objective-schema validation, four-way dominance, incremental Pareto
  filtering, exact replayable fronts, and approximate sampled fronts.
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
- Exhaustive fixture variation against direct domain oracles for exact cover,
  small scheduling horizons, and generated Connect Four prefixes, plus
  generated perishables inventories against an independent cohort oracle.

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
4. Search- and simulation-heavy workloads need receipt-maintained holdings,
   supply, dependency, and event indexes plus incremental state fingerprints;
   these must remain rebuildable projections rather than semantic authority.
5. More domains will require constrained local and inequality invariants.
6. External OR adapters should compile encoded semantics, emit exchanges, and
   use replay as a mandatory proof checker.
7. MCTS and ISMCTS may need PUCT priors, progressive widening, and deterministic
   parallel workers once learned priors, measured branching, or throughput
   justify them.
8. Nature needs richer parameterized distribution updates for very large or
   continuous outcome spaces.
9. Larger multi-objective spaces will need measured frontier representations
   or domain-proven pruning; approximate epsilon dominance must never be
   mislabeled as exhaustive Pareto truth.

Any future abstraction must continue to pass all thirteen problems without
moving authoritative meaning into solver callbacks or an external world.
