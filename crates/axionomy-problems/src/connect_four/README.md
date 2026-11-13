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

Generic vector-valued MCTS uses the encoded current turn to choose which
player value to maximize. Tree edges are exchanges, transpositions use
canonical state keys, rollouts use seeded action selection, and the final
action is revalidated by the live economy.

## Required results

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

## API pressure

- Adversarial vector outcomes and actor-relative tree selection.
- Large derived search trees over a compact authoritative state.
- Core-encoded terminal detection without solver-only win logic.
- Canonical transpositions and bounded seeded exploration.
- Concrete-rate growth from structured move schemas.

## In this folder

- `mod.rs` is the model: the compact and standard boards, their move and draw
  rates, MCTS and self-play.
- `tactics.rs` is read-only board reasoning for opponents and evidence; every
  move it suggests is still a core exchange.
- `package.rs` writes its package: its economies under refs, goals,
  strategies, experiments and view, whose scripts are `scripts/connect-four.rhai`
  and `scripts/connect-four-view.rhai`.

[`examples/connect_four.rs`](https://github.com/BiomaAI/axionomy/blob/main/crates/axionomy-problems/examples/connect_four.rs) runs it:

```console
cargo run -p axionomy-problems --example connect_four
```

Connect Four is one of the [conformance problems](https://github.com/BiomaAI/axionomy/blob/main/PROBLEMS.md).
