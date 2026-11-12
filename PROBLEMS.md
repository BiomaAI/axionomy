# Axionomy Closed-Problem Conformance Suite

This document indexes the problem set used to evolve and test Axionomy's
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

## Problems

Each problem lives in one folder of `crates/axionomy-problems/src`, and the
README there explains it: its specification, the results its tests require,
the pressure it puts on the API, and the files beside it. The same README is
that module's Rust documentation.

| Problem | Folder | Pressure on the API |
| --- | --- | --- |
| [Key-door energy maze](crates/axionomy-problems/src/maze/README.md) | `maze` | Graph facts, encoded costs and heuristics, exact multi-objective search |
| [Sokoban](crates/axionomy-problems/src/sokoban/README.md) | `sokoban` | Three-account atomic pushes, explicit emptiness, stable identity, deadlock evidence |
| [Exact cover](crates/axionomy-problems/src/exact_cover/README.md) | `exact_cover` | Constraints as resource-sensitive facts, a specialized solver compiled from core data |
| [Stoichiometric workshop](crates/axionomy-problems/src/workshop/README.md) | `workshop` | Identity-changing production, catalysts, declared conservation |
| [Discrete job-shop scheduling](crates/axionomy-problems/src/scheduling/README.md) | `scheduling` | Variable multi-account reservation, precedence as tokens, encoded makespan |
| [Stochastic rescue](crates/axionomy-problems/src/rescue/README.md) | `rescue` | Restricted observation, hidden truth versus belief, an encoded Nature |
| [Single-lane bridge negotiation](crates/axionomy-problems/src/bridge/README.md) | `bridge` | Atomic joint resolution, escrow, capacity as a conserved asset |
| [Multi-party marketplace settlement](crates/axionomy-problems/src/marketplace/README.md) | `marketplace` | Six-role settlement, shortfalls as matching information, disposable ranking |
| [Stochastic delivery logistics](crates/axionomy-problems/src/logistics/README.md) | `logistics` | Long stochastic horizons, recurrent Nature, Monte Carlo and MCTS |
| [Adversarial Connect Four](crates/axionomy-problems/src/connect_four/README.md) | `connect_four` | Adversarial vector outcomes, encoded terminal truth, canonical transpositions |
| [Partially observed team mission](crates/axionomy-problems/src/mission/README.md) | `mission` | Agent-specific observations, caller-owned beliefs, information-set search |
| [Cohort-indexed perishables](crates/axionomy-problems/src/perishables/README.md) | `perishables` | Structural fungibility, unique condition facts, event-driven time |
| [Autonomous Work League](crates/axionomy-problems/src/work_league/README.md) | `work_league` | Competitive allocation without a scheduler, several honest leaderboards |
| [Living Market AMM](crates/axionomy-problems/src/amm/README.md) | `amm` | State-evaluated nonlinear rates, causal attribution through replay |
| [Lending market](crates/axionomy-problems/src/lending/README.md) | `lending` | `MulDiv` rounded against the caller, fixed-point indexes, safety gates |

## Conformance matrix

| Capability | Maze | Sokoban | Exact cover | Workshop | Job shop | Rescue | Bridge | Marketplace | Logistics | Connect Four | Mission | Perishables | Work League | Living Market | Lending |
| --- | :---: | :---: | :---: | :---: | :---: | :---: | :---: | :---: | :---: | :---: | :---: | :---: | :---: | :---: | :---: |
| Multi-account atomic rewrite | ✓ | ✓ |  |  | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| Preserved facts/catalysts | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| Declared invariants | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |  | ✓ | ✓ |
| Resource objective | ✓ |  |  | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |  | ✓ | ✓ | ✓ | ✓ | ✓ |
| Exact Pareto front | ✓ |  |  | ✓ | ✓ |  | ✓ | ✓ |  |  |  | ✓ |  |  |  |
| Approximate or derived Pareto comparison |  |  |  |  |  | ✓ |  |  | ✓ |  | ✓ |  | ✓ |  |  |
| Infeasible instance |  | ✓ | ✓ |  | ✓ |  |  | ✓ | ✓ |  | ✓ | ✓ |  | ✓ |  |
| Specialized proposer |  |  | Algorithm X |  | Branch optimizer | Scenario/MC evaluator | Auction | Assessment clearing | Monte Carlo/MCTS | MCTS/minimax oracle | ISMCTS/scenario evaluator | Event agenda/index | Competing seeded policies | Seeded actors/Shapley | Scripted liquidation and first-depositor attack |
| Generic algorithm | BFS/A*/Dijkstra/Pareto | BFS | BFS | BFS/best-first/Pareto | Best-first/Pareto | Rollout/MC/Pareto | BFS/Pareto | Pareto | Rollout/MC/Pareto | MCTS | ISMCTS/Rollout/MC/Pareto/RL | Pareto | Replay-derived vector comparison | Exact coalition replay | BFS/Dijkstra, property-based round trips |
| Hidden or stochastic state |  |  |  |  |  | ✓ |  |  | ✓ |  | ✓ |  | ✓ |  |  |
| Multi-agent resolution |  |  |  |  |  |  | ✓ | ✓ |  | ✓ | ✓ |  | ✓ | ✓ |  |
| Long-horizon trajectory |  |  |  |  |  |  |  |  | ✓ | ✓ |  | ✓ | ✓ | ✓ |  |
| Per-step leaderboards |  |  |  |  |  |  |  |  |  |  |  |  | ✓ | ✓ |  |
| Learning trajectory |  |  |  |  |  |  |  |  |  |  | ✓ |  |  |  |  |
| Observation-scoped tree |  |  |  |  |  |  |  |  |  |  | ✓ |  |  |  |  |
| Deterministic replay test | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |

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
cargo run -p axionomy-problems --example amm
cargo run -p axionomy-problems --example marketplace
cargo run -p axionomy-problems --example logistics
cargo run -p axionomy-problems --example connect_four
cargo run -p axionomy-problems --example mission
cargo run -p axionomy-problems --example perishables
cargo run -p axionomy-problems --example work_league
cargo run -p axionomy-problems --example lending
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

Every canonical problem declares the same three instances, under the keys
links, the CLI's `--instance` and MCP's `instance` take:

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
`StudyRequest` and `Study`, so a result cannot be compared or replayed as
though it came from a different workload.

What each instance holds and what each strategy does is written once, in the
problem's catalog entry in its `study.rs`, with its family and what it
exercises: each a path with the words people read for it, from the suite's
shared vocabulary in `support.rs`. Studio shows those words, and
`cargo run -p axionomy-cli -- describe <problem>` prints them. A problem of
your own declares its own instances, family and capabilities the same way.

**Model size** telemetry records accounts, rules, trace steps, rule-check
probes, and compared alternatives for every result. The study tests
(`crates/axionomy-problems/tests/studies.rs`) also enforce per-problem minimum
pressure (and a maximum compact-rate bound for standard Connect Four),
preventing future refactors from silently collapsing Showcase back into a toy.

Each problem's README describes the Micro conformance contract unless it names
a larger profile explicitly. Showcase builds on those same laws; it does not
replace the exact fixtures that make correctness auditable.

## Cross-problem acceptance tests

The repository test suite additionally verifies:

- Exact basket shortfalls.
- Checked scale, addition, withdrawal, and deposit.
- Exact checked `u64` economies.
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
- Rounding against the caller: every model that divides names its `Rounding`;
  amounts a participant pays round up and amounts it receives round down, so
  no round trip creates value and conservation invariants hold. Full-width
  `MulDiv` agrees with a wide-integer reference.
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

1. Scheduling and stochastic outcomes need richer typed parameter schemas to
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

Any future abstraction must continue to pass every problem without
moving authoritative meaning into solver callbacks or an external world.
