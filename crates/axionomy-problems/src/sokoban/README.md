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
`Player`, `Empty`, or a stable `Crate(id)`. The board's dimensions are the
whole warehouse's and never change, so they are the economy's constants,
`{board-width}` and `{board-height}`, which the search and the view read
(`state.constant`): neither needs hidden geometry state. `Floor`, `Wall`
and `DeadSquare` stay facts each cell holds: no law reads them, only the
view and the model's tests. A move atomically exchanges player and empty
tokens between adjacent floor accounts.
A push atomically rewrites three accounts while preserving crate identity:

```text
[Player][Crate(0)][Empty] → [Empty][Player][Crate(0)]
```

Each is one rate with variables over the board's geometry, which no exchange
changes and so is a relation: `move/{from}/{to}` over the floors that adjoin,
`push/{behind}/{crate-at}/{to}/{crate}` over the floors three in a row, and
`latch/{circuit}/{at}` over the latch panels. Every role is fixed to the cell
its identifier names, so no cell holds an identity token. Whether a cell is
the exit, or a circuit's door or plate, never changes either: a relation over
each variable gives it, `1` or `0`, for every floor, named after the
variable, such as `{to-door-0}` or `{crate-at-plate-1}`, and the one law
reads it of the cells it binds. A door's cell admits the player or a crate
only while its circuit is pressed or latched, a crate pressing or releasing a
plate moves the circuit, and the player reaching or leaving the exit moves
the puzzle's record. Micro has 3 rates as written, for 15 instances;
Showcase 4, for 132; Stress 4, for 259.

The Showcase warehouse has boundary walls, an internal obstruction, two crates,
two goals, and legal moves that do not advance the objective. Completion is
one `Finish` law with a goal role fixed to each goal cell, which must hold a
crate, whichever crate it is; the goal cells are the law's, and the search
and view read them from it. A second trace
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
- Completion is one law fixed to the drawn goal cells, refused while any of
  them is empty, consumes the active puzzle lifecycle, and is quiescent.

## API pressure

- Atomic effects over three locations.
- Explicit emptiness rather than a hidden absence test.
- Stable object identity across spatial motion.
- Terrain/occupant separation for generic grid projections.
- Spatial invariants, admissible search policy, and deadlock evidence.
- Geometry as relations of rates with variables, whose rows give what each
  cell they bind is, and whose effects depend on it.

## In this folder

- `mod.rs` is the model: every warehouse, its moves, pushes and mechanisms.
- `package.rs` writes its package, the warehouse's only implementation of
  its searches: its economies under refs, goals, strategies, experiments and
  view. The searches' moves are the rates' instances that apply
  (`CandidateSet::Instances`); `scripts/sokoban.rhai` holds only A*'s lower
  bound and a step's cost, and `scripts/sokoban-view.rhai` draws the board.

Sokoban is one of the [conformance problems](https://github.com/BiomaAI/axionomy/blob/main/PROBLEMS.md).
