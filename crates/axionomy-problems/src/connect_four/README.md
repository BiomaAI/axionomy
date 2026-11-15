# Adversarial Connect Four

Adversarial Connect Four with encoded gravity and terminal truth.

## Specification

A board has one account per cell and one per column, a game account and a
result account; the compact board is four by four, the standard one seven by
six. Cell occupancy, each column's height, the turn, and each player's pieces
in every line of four cells are balances. A line is named by its direction
(across, up, rising or falling) and its rightmost column and top row; a name
whose cells leave the board names a line no drop fills.

A drop is one rate over every column and row, `drop/<player>/{column}/{row}`,
each role fixed to its account: the game, `column/{column}` and
`cell/{column}/{row}`. A condition checks the row against the column's
height, which grows by one; the cell's emptiness becomes the piece; and the
sixteen lines through the cell, their columns and rows derived from the
cell's, count it. The drop passes the turn while none of those lines holds
three of the player's pieces; the drop completing one is
`win/{player}/{column}/{row}`, which produces `Winner(player)` instead. A full
board with the turn still held is drawn by `draw/{player}`, which checks
every column's height. Terminal truth therefore never comes from an external
board callback, and both boards are four rates with variables: the compact
board's 66 instances, where it had 1,282 concrete moves, and the standard
board's 170.

The moves are the instances that apply (`connect-four/moves`,
`CandidateSet::Instances`). The searches try the same moves, but listing
every instance at every state they reach would cost them several times the
game itself, so `connect-four/drops` proposes only each column's drop on
its height, or its win, as `state.exchange_of` binds the instance. Red
searches by generic vector-valued MCTS, which uses the encoded current turn
to choose which player value to maximize; its tree edges are exchanges,
transpositions use canonical state keys, and each simulation plays a drop
and the reply uniformly with the search's tickets, then estimates the rest
by the board's worth. Yellow searches alike, drops nearest the center, or
chooses by the engine's minimax with alpha-beta pruning five drops deep.
The board's worth, `evaluate` in `scripts/connect-four.rhai`, reads the line
counts: a line only one player holds pieces in is worth 1, 8 or 60 for one,
two or three of them.

## Required results

The unit tests check the laws by applying exchanges:

- Gravity places a first piece in row zero and grows the column's height;
  no piece lands above an empty cell, or where one already is.
- Rebinding a drop to another empty cell or column is rejected by its fixed
  accounts.
- On either board, the drop completing a line cannot pass the turn, a drop
  elsewhere cannot claim the win, and the winning drop produces the encoded
  winner; nothing applies after it.
- A full board without a line is drawn, and not before it is full.

The package's experiments find the rest, pinned in `tests/outcomes`: every
game, in self-play or against the center-first or minimax opponent, is
played to an encoded win or draw, and its trace replays deterministically.

## API pressure

- Adversarial vector outcomes and actor-relative tree selection.
- Large derived search trees over a compact authoritative state.
- Core-encoded terminal detection without solver-only win logic.
- Canonical transpositions and bounded seeded exploration.
- Rates with variables and derived values in place of structured move
  schemas.

## In this folder

- `mod.rs` is the model: the compact and standard boards, their drop, win
  and draw rates, and their typed exchanges.
- `package.rs` writes its package: its economies under refs, goals,
  strategies, experiments and view, whose scripts are `scripts/connect-four.rhai`
  and `scripts/connect-four-view.rhai`.

Connect Four is one of the [conformance problems](https://github.com/BiomaAI/axionomy/blob/main/PROBLEMS.md).
