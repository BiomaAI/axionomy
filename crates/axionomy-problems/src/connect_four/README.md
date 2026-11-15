# Adversarial Connect Four

Compact adversarial Connect Four with encoded gravity and terminal truth.

## Specification

A compact four-by-four Connect Four board has one account per cell and one
account per column. Every game, result, column, and cell account preserves an
encoded identity. Cell occupancy, gravity progress, alternating turns, and
both players' counts for every row, column, and diagonal are assets.

Move rates consume the exact current line counters and produce their
successors. A move completing a line consumes the current turn and produces
`Winner(player)` directly. A full board with no winner enables a draw rate
that preserves all four `ColumnFull` facts. Terminal truth therefore never
comes from an external board callback.

The package's games search with generic vector-valued MCTS, which uses the
encoded current turn to choose which player value to maximize. Tree edges
are exchanges, transpositions use canonical state keys, rollouts use seeded
action selection, and the final action is revalidated by the live economy.

## Required results

The unit tests check the laws by applying exchanges:

- Gravity places a first piece in row zero and advances the column token;
  no piece lands above an empty cell.
- Line counters advance atomically with cell and turn state, and a drop
  consumes exactly the counts its player holds.
- Rebinding a move to another empty cell or column is rejected by identities.
- A move completing a line produces the encoded winner asset. On the
  standard board the turn cannot pass a completed line, and its win
  certificate applies.

The package's experiments find the rest, pinned in `tests/outcomes`: every
game, in self-play or against the center-first or alpha-beta opponent, is
played to an encoded win or draw, and its trace replays deterministically.

## API pressure

- Adversarial vector outcomes and actor-relative tree selection.
- Large derived search trees over a compact authoritative state.
- Core-encoded terminal detection without solver-only win logic.
- Canonical transpositions and bounded seeded exploration.
- Concrete-rate growth from structured move schemas.

## In this folder

- `mod.rs` is the model: the compact and standard boards, their move, win
  and draw rates, and their typed exchanges.
- `package.rs` writes its package: its economies under refs, goals,
  strategies, experiments and view, whose scripts are `scripts/connect-four.rhai`
  and `scripts/connect-four-view.rhai`.

Connect Four is one of the [conformance problems](https://github.com/BiomaAI/axionomy/blob/main/PROBLEMS.md).
