# axionomy-search

Reference, non-authoritative search strategies for
[Axionomy](https://github.com/BiomaAI/axionomy).

This crate provides algorithms and learning projections over closed economic
states. All structures are disposable accelerators; transitions remain
ordinary exchanges validated by the `axionomy` kernel.

- `bfs` and `best_first` provide deterministic graph search.
- `rollout` executes bounded speculative exchange trajectories.
- `sampling` selects among weighted, encoded Nature exchanges.
- `monte_carlo` evaluates arbitrary experiments with Bernoulli, scalar, vector,
  quantile, and tail-risk statistics.
- `mcts` provides deterministic-budget, vector-valued UCT with chance nodes
  and canonical transpositions.
- `rl` derives action masks, sparse shortfall features, transition records,
  and learning trajectories from assessments, receipts, and replay.

Algorithms may choose which valid proposal to explore and how to aggregate
encoded outcomes. They do not define domain transitions, hidden truth,
terminal state, or rewards.
