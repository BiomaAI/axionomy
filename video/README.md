# Axionomy video

A silent video, about 85 seconds at 1920x1080 and 30 fps, made with
[fframes](https://github.com/dmtrKovalenko/fframes). It shows an economy's
accounts, rates and exchanges, then what that gives an agent (value, utility,
simulation, training), then what can drive it and why its runs can be trusted.

No number on screen is typed in. `src/facts.rs` writes every sample package
into a store in memory when the video starts and reads what it shows from the
engine:

- the maze Micro's balances, its first move's rate, the move applied, and the
  shortfall the kernel reports for unlocking the door without the key;
- the AMM Showcase's pool reserves over a season its strategy played;
- where the maze Showcase experiment's searches end, the points its start
  forks into, and the plan A* commits;
- each shown package's accounts and rate instances at its Showcase start;
- a recorded run's state fingerprint and the same run replayed;
- the Lean theorems it names, each checked to be in its file.

`data/learned` holds what a DQN learned on the maze Showcase, written by the
Burn crate's example:

```text
cd ../crates/axionomy-burn
cargo run --release --example maze -- ../../video/data/learned
```

The video replays each learned route through the kernel and measures it. Its
training caption says the learner found the searches' routes only where every
learned route ends on one of them.

## Commands

```text
cargo run --release -- timeline     # scenes and their times
cargo run --release -- inspect      # missing fonts, clipped text, bad SVG
cargo run --release -- strip ModelScene -n 12
cargo run --release -- frame "TrainingScene@8.5s"
cargo run --release -- preview      # a window, for watching
cargo run --release -- render -o axionomy.mp4
```

The video is its own workspace: fframes builds Skia and ffmpeg, so the root
workspace excludes it. `Cargo.lock` keeps `fframes-media` and
`fframes-media-dir-macro` at 1.4.0, since their 1.4.1 releases do not build
with fframes 1.4.0.

The fonts in `media/`, DM Sans and JetBrains Mono, are under the SIL Open
Font License; their licenses are in `licenses/`.
