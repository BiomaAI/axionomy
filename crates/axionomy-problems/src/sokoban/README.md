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
composes two circuits across three chambers. A floor cell no reverse push
reaches, every door optimistically open, holds a `DeadSquare` fact; pushes onto
one remain legal, as each instance's deadlock replay shows. Micro is unchanged.

## Specification

Every board cell is an account, `cell/{index}`, that owns either `Floor` or
`Wall`. Walkable cells hold exactly one occupancy asset:
`Player`, `Empty`, or a stable `Crate(id)`. Goal squares own `GoalCell` in
addition to their occupant. Board dimensions are ordinary assets on the puzzle
account, so neither the search nor viewer needs hidden geometry state. A move
atomically exchanges player and empty tokens between adjacent floor accounts.
A push atomically rewrites three accounts while preserving crate identity:

```text
[Player][Crate(0)][Empty] → [Empty][Player][Crate(0)]
```

Each is one rate with variables over the board's geometry, which no exchange
changes and so is a relation: `move/{from}/{to}` over the floors that adjoin,
`push/{behind}/{crate-at}/{to}/{crate}` over the floors three in a row, and
`latch/{circuit}/{at}` over the latch panels. Every role is fixed to the cell
its identifier names, so no cell holds an identity token. A door, a plate or
the exit is a fact of its cell, and the one law reads it from the cells it
binds: a door's cell admits the player or a crate only while its circuit is
pressed or latched, a crate pressing or releasing a plate moves the circuit,
and the player reaching or leaving the exit moves the puzzle's record.

The Showcase warehouse has boundary walls, an internal obstruction, two crates,
two goals, and legal moves that do not advance the objective. Completion is
one `Finish` law: its final exchange binds every stable crate's goal role to
the distinct goal cell that crate rests on, as the package's search script
binds it. A second trace
starts from that same economy and replays a short sequence ending in a legal
push into a non-goal corner, making the difference between validity and
decision quality directly inspectable.

## Required results

The package's experiments and refs find these, pinned in `tests/outcomes`:

- Breadth-first search and A* both traverse only core-applicable exchanges
  and solve every warehouse.
- A* uses an admissible crate-to-goal lower bound and reports expanded,
  generated, frontier, and visited state counts without inventing a percentage.
- Each instance's `deadlock` ref replays a legal plan that ends with a crate
  on a structurally identified non-goal dead square.

The model's tests apply the rules directly:

- Every floor retains exactly one occupancy token and every crate identity is
  conserved across the full replay.
- A move bound to cells other than those its identifier names is refused,
  and cells that do not adjoin have no move.
- Completion binds all crates to distinct goals, consumes the active puzzle
  lifecycle, and is quiescent.

## API pressure

- Atomic effects over three locations.
- Explicit emptiness rather than a hidden absence test.
- Stable object identity across spatial motion.
- Terrain/occupant separation for generic grid projections.
- Spatial invariants, admissible search policy, and deadlock evidence.
- Geometry as relations of rates with variables, whose effects depend on
  facts of the cells they bind.

## In this folder

- `mod.rs` is the model: every warehouse, its moves, pushes and mechanisms.
- `package.rs` writes its package, the warehouse's only implementation of
  its searches: its economies under refs, goals, strategies, experiments and
  view, whose scripts are `scripts/sokoban.rhai` and
  `scripts/sokoban-view.rhai`.

Sokoban is one of the [conformance problems](https://github.com/BiomaAI/axionomy/blob/main/PROBLEMS.md).
