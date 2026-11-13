# Acceptance

An application drives Axionomy from outside, over the Studio server's HTTP
API, the way the [plan](../docs/SIMULATION.md#plan-axionomy-on-primitives)
(P13) and D-046 describe: every step is an apply at a point, and a ref says
which steps count.

`test_live_game.py` plays Connect Four on the standard board from the
connect-four package's `connect-four/showcase` point, with four roles in
threads of their own, each over its own connection:

- **Two players**, Red and Yellow, move the ref `connect-four/live`. Each
  reads where the ref is and asks for the moves there (`connect-four/moves`,
  each assessed). When they are its drops, it picks one at random from its
  seed, applies it at the ref's point and advances the ref from that point
  to the new one; when they are the other player's, it waits with a long
  poll on the ref's frames for the ref to move. Both adjudicate every drop
  (the line claimed, the draw or the turn passing on), meeting first so both
  propose from the same point: one advance counts and the other is refused
  with 409 and where the ref went, and its player looks again from there.
- **A thinker** tries every move out at each point it finds the ref on,
  applying each there and never moving the ref, then waits for the ref to
  move.
- **A learner** follows the ref from the start through `frames(after)` with
  a long poll, each frame measuring both players' values, until a frame says
  the game is over.

The test then checks that of the advances from one point at most one
counted and they lead from the start to where the ref is; that every advance
that did not count was refused with where the ref was, once for each
adjudication; that the learner received exactly the steps the players
advanced over, in order, each the step after the one before, ending at the
ref's point; that no point the thinker made, nor one a refused advance
would have moved the ref to, was delivered; that the game is won or drawn at
the last frame and not before, with no move left; that the players alone,
from the same start and with no ref, make the same moves; and that Studio
lists the ref with the package. The game itself does not depend on timing:
every run plays the same one, today Yellow's win in 40 steps, 20 of them
adjudications.

The session builds every sample package into a temporary directory with
`axionomy package samples` (they are never committed), starts
`axionomy studio` on a free port with `connect-four.zip` loaded into a store
in memory, and stops it at the end. It uses `AXIONOMY_BIN`, or else the
debug build. Python is managed with uv:

```console
cargo build -p axionomy-cli --bin axionomy
cd acceptance && uv run --locked pytest
```

It takes a few seconds, most of them building the packages.
