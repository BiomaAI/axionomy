# Sokoban

Sokoban encoded entirely as accounts, assets, rates, and exchanges.

The board is not solver-owned state. Each cell is an account whose terrain
and occupant are assets, and a push is one atomic exchange across three cell
accounts. Crates keep stable identities so viewers and callers can follow
the same object through a replay.

The larger warehouses contain crate-weight pressure plates, corresponding
doors, remote latch panels, and an exit beyond the doors. The player must stage
a crate to cross, latch the door, recover the crate, complete the goals, and
reach the exit. Walking onto a plate does not activate it. All mechanism and
exit state changes are balances in the same atomic move/push exchanges. Stress
composes two circuits across three chambers. Search excludes statically proven
dead squares using reverse-push reachability with every door optimistically open;
those losing pushes remain legal for counterexample replays. Micro is unchanged.

## Specification

Every board cell is an account that preserves its coordinate identity and
owns either `Floor` or `Wall`. Walkable cells hold exactly one occupancy asset:
`Player`, `Empty`, or a stable `Crate(id)`. Goal squares own `GoalCell` in
addition to their occupant. Board dimensions are ordinary assets on the puzzle
account, so neither the search nor viewer needs hidden geometry state. A move
atomically exchanges player and empty tokens between adjacent floor accounts.
A push atomically rewrites three accounts while preserving crate identity:

```text
[Player][Crate(0)][Empty] → [Empty][Player][Crate(0)]
```

The Showcase warehouse has boundary walls, an internal obstruction, two crates,
two goals, and legal moves that do not advance the objective. Completion is
one `Finish` law: its final exchange binds every stable crate's goal role to
the distinct goal cell that crate rests on. A second trace
starts from that same economy and replays a short sequence ending in a legal
push into a non-goal corner, making the difference between validity and
decision quality directly inspectable.

## Required results

- Resumable BFS and A* both traverse only core-applicable exchanges.
- A* uses an admissible crate-to-goal lower bound and reports expanded,
  generated, frontier, and visited state counts without inventing a percentage.
- Every floor retains exactly one occupancy token and every crate identity is
  conserved across the full replay.
- Rebinding a legal move rate to nonadjacent cells is rejected.
- Completion binds all crates to distinct goals, consumes the active puzzle
  lifecycle, and is quiescent.
- The losing alternative replays a legal push and ends with a crate on a
  structurally identified non-goal dead square.

## API pressure

- Atomic effects over three locations.
- Explicit emptiness rather than a hidden absence test.
- Stable object identity across spatial motion.
- Terrain/occupant separation for generic grid projections.
- Spatial invariants, admissible search policy, and deadlock evidence.

## In this folder

- `mod.rs` is the model: every warehouse, its moves, pushes and mechanisms,
  and the searches over them.
- `study.rs` is `Sokoban`, the puzzle's `axionomy_study::Problem`: its catalog
  entry, strategies and the documents a study holds, each derived by replay.
- `view.rs` presents it: labels, units, scene and leaderboards.

[`examples/sokoban.rs`](https://github.com/BiomaAI/axionomy/blob/main/crates/axionomy-problems/examples/sokoban.rs) runs it:

```console
cargo run -p axionomy-problems --example sokoban
```

Sokoban is one of the [conformance problems](https://github.com/BiomaAI/axionomy/blob/main/PROBLEMS.md).
