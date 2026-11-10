# Exact cover

Exact cover solved both by generic graph search and an Algorithm X proposer.

Showcase uses fourteen globally overlapping pairs, triples, and quadruples over
eight elements. Complete covers require two, three, or four selections, and some
locally legal choices block completion. Stress couples twelve elements through
twenty-five sets. Algorithm X can continue from a partial cover. Larger
unsatisfiable fixtures include every element but cannot cover them together.
Progress now advances by each selected set's actual cardinality.

## Specification

The universe is `{A,B,C,D}`. Four available subsets are encoded:

```text
AB, CD, AC, BD
```

Selecting a subset preserves the active problem lifecycle, consumes its availability token and every corresponding
`Uncovered(element)` token. It produces `Selected(set)`,
`Covered(element)`, and advances an explicit progress token. Overlapping
subsets become inapplicable because they require already-consumed uncovered
tokens.

## Required results

- Generic BFS finds a cover.
- Algorithm X reads subset membership from rate consumption baskets.
- Algorithm X emits ordinary exchanges and validates them by replay.
- Both traces have two selections plus finish.
- Both solvers reject an instance containing only `AB` and `AC`.
- All 16 availability combinations agree with a direct exhaustive oracle.
- Finish requires all four encoded `Covered` facts, then consumes `Active`.

## API pressure

- Constraint satisfaction through resource-sensitive facts.
- Specialized solver compilation from core data.
- Solver output translated back to the shared execution language.

## In this folder

- `mod.rs` is the model: every instance's universe and subsets, plain search,
  and Algorithm X.
- `study.rs` is `ExactCover`, the problem's `axionomy_study::Problem`: its
  catalog entry, strategies and the documents a study holds, each derived by
  replay.
- `view.rs` presents it: labels, units, scene and leaderboards.

[`examples/exact_cover.rs`](https://github.com/BiomaAI/axionomy/blob/main/crates/axionomy-problems/examples/exact_cover.rs) runs it:

```console
cargo run -p axionomy-problems --example exact_cover
```

Exact cover is one of the [conformance problems](https://github.com/BiomaAI/axionomy/blob/main/PROBLEMS.md).
