# axionomy-search

Reference, non-authoritative search strategies for
[Axionomy](https://github.com/BiomaAI/axionomy).

This crate provides algorithms and learning projections over closed economic
states. All structures are disposable accelerators; transitions remain
ordinary exchanges validated by the `axionomy` kernel.

- `bfs` and `best_first` provide deterministic graph search.
- `action_source` lazily emits concrete proposals without giving generators
  transition authority.
- `rollout` executes bounded speculative exchange trajectories.
- `sampling` selects among weighted, encoded Nature exchanges.
- `monte_carlo` evaluates arbitrary experiments with Bernoulli, scalar, vector,
  quantile, and tail-risk statistics.
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

Information-set search makes the visibility boundary structural. The caller
derives an `InformationState` from an account-restricted economic view; the
belief sampler receives only that root identity, and action sources receive
only the current information state. Environment chance and outcome projections
may inspect a sampled closed world, but every decision and Nature outcome is
still a concrete exchange. Belief samples inconsistent with the root
observation are rejected, and the selected live exchange is revalidated by the
kernel.
