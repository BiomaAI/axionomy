# axionomy-search

Reference, non-authoritative search strategies for
[Axionomy](https://github.com/BiomaAI/axionomy).

This crate provides algorithms and learning projections over closed economic
states. All structures are disposable accelerators; transitions remain
ordinary exchanges validated by the `axionomy` kernel.

- `bfs`, `dijkstra`, and `astar` use the `pathfinding` crate over implicit,
  core-validated successors; `best_first` preserves state-ranked compatibility.
- `action_source` lazily emits concrete proposals without giving generators
  transition authority.
- `rollout` executes bounded speculative exchange trajectories.
- `sampling` selects among weighted, encoded Nature exchanges through `rand`,
  including reproducible ChaCha8 and caller-owned generators.
- `monte_carlo` evaluates arbitrary experiments with Bernoulli, scalar, vector,
  quantile, credible-interval, and tail-risk statistics backed by `statrs`.
- `mcts` provides deterministic-budget, vector-valued UCT with chance nodes
  and canonical transpositions.
- `ismcts` root-samples encoded belief worlds, keys a shared tree by
  actor-visible information, and does not pass hidden full state to decision
  generation or rollout policy.
- `rl` derives action masks, sparse shortfall features, transition records,
  and learning trajectories from assessments, receipts, and replay.

Algorithms may choose which valid proposal to explore and how to aggregate
encoded outcomes. They do not define domain transitions, hidden truth,
terminal state, or rewards.

Every strategy is generic over the economy's exact quantity backend. Solver
costs, visit counts, random streams, and statistical values keep their own
derived numeric domains because they are disposable policy rather than
authoritative balances.

## Bounded, caller-controlled execution

Long-running algorithms expose runtime-neutral sessions instead of owning an
async runtime, thread, logger, or transport. `BfsSession`, `MonteCarloSession`,
`MctsSession`, and `IsmctsSession` advance by an explicit `WorkBudget` measured
in deterministic algorithm units: expanded states, samples, or tree
iterations. Each call returns an `AdvanceReport` with a serializable progress
snapshot and lifecycle status.

A `SearchObserver` may interrupt an `advance` call at a safe boundary. The
session remains valid and can be advanced again, so callers can implement UI
progress, cancellation flags, cooperative scheduling, task polling, or custom
checkpoint policy without coupling the algorithms to Tokio or MCP. Chunking a
fixed-seed computation does not change its result.

This control state is disposable. Budgets and cancellation may stop
exploration, but they cannot create domain success, failure, time, or cost;
those meanings still require encoded assets and exchanges.

Information-set search makes the visibility boundary structural. The caller
derives an `InformationState` from an account-restricted economic view; the
belief sampler receives only that root identity, and action sources receive
only the current information state. Environment chance and outcome projections
may inspect a sampled closed world, but every decision and Nature outcome is
still a concrete exchange. Belief samples inconsistent with the root
observation are rejected, and the selected live exchange is revalidated by the
kernel.
