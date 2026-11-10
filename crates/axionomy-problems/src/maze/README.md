# Key-door energy maze

A key-door maze where action count and encoded energy cost disagree.

Showcase and Stress add shared vault/transit power, finite protective equipment,
persistent injuries that slow later actions, and medicine-funded recovery. An
unlocked vault still requires power, and diverting power disables the canal
crossing. The exact Showcase frontier is `(energy, time) = (9,12), (11,6),
(12,5), (15,4)`. The fewest-exchanges detour takes five ticks with untreated
injuries; collecting protection adds an exchange but reduces time to four.
Micro retains the original rules.

## Specification

The Micro agent starts at `Start` with nine energy and six time. The world owns directed edge
facts, a key in `KeyRoom`, a locked gate, a target at `Exit`, and one encoded
distance estimate per node.

Two routes exist:

```text
Key route:    Start --2--> KeyRoom --2--> Gate --2--> Exit
Detour route: Start --4--> Detour --5--> Exit
```

The key route also requires `TakeKey` and `UnlockDoor`. The detour therefore
has fewer exchanges, while the key route spends less energy.

Every semantic action consumes time and produces `SpentTime`. Movement also
consumes `At(from)` and energy, preserves `Edge(from,to)` and any door
permission, and produces `At(to)` plus `SpentEnergy`. Search cost is
derived from the before/after `SpentEnergy` delta rather than copied from the
rate identifier. A final rate consumes the maze's `Active` lifecycle asset and
produces `Solved`, leaving no post-terminal moves applicable.

## Required results

- BFS chooses the three-exchange detour.
- Dijkstra and A* choose the six-energy key route.
- Dijkstra and A* agree on cost.
- Exact Pareto search retains `(energy=6,time=6)` and `(energy=9,time=3)`;
  neither route is hidden behind one scalar weight.
- Every trace replays to `Solved`.
- Actor/environment role swapping is rejected by encoded requirements.
- A solved maze has no applicable actions.

The Showcase keeps those laws but expands them into the 15-room Vault
District: 25 directed passages form cyclic route families with real
backtracking, the key visibly transfers from the district to the Explorer,
and the gate can only be unlocked while the Explorer is standing there. Its
exact frontier holds the four tradeoffs above. Dijkstra and A* agree on the
least-energy result while A* expands materially fewer states; Studio retains
each algorithm's own expansion history rather than relabeling one shared log.

## API pressure

- Structured graph facts without an external graph state.
- Preserved topology and permissions.
- Objective and admissible heuristic values encoded as assets.
- One state graph served by multiple traversal strategies.
- Exact multi-objective comparison without weighted-sum scalarization.
- Stable scene subjects for generic search-frontier visualization.
- Truthful inventory attachment, gate lifecycle, planned route, and replayed
  traversal without a browser-side maze model.

## In this folder

- `mod.rs` is the model: the Micro maze, the Showcase and Stress districts,
  and the searches over them.
- `dynamics.rs` adds the larger mazes' shared power, protective gear,
  persistent injuries and recovery.
- `study.rs` is `Maze`, the maze's `axionomy_study::Problem`: its catalog
  entry, strategies and the documents a study holds, each derived by replay.
- `view.rs` presents it: labels, units, scene and leaderboards.

[`examples/maze.rs`](https://github.com/BiomaAI/axionomy/blob/main/crates/axionomy-problems/examples/maze.rs) runs it:

```console
cargo run -p axionomy-problems --example maze
```

The maze is one of the [conformance problems](https://github.com/BiomaAI/axionomy/blob/main/PROBLEMS.md).
