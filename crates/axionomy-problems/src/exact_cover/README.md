# Exact cover

Exact cover solved both by generic graph search and an Algorithm X proposer.

Showcase uses fourteen globally overlapping pairs, triples, and quadruples over
eight elements. Complete covers require two, three, or four selections, and some
locally legal choices block completion. Stress couples twelve elements through
twenty-five sets. The package's Algorithm X can continue from a partial cover.
Larger unsatisfiable fixtures include every element but cannot cover them
together. Progress now advances by each selected set's actual cardinality.

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

The package's experiments find these, pinned in `tests/outcomes`:

- Breadth-first search finds a cover.
- Algorithm X reads subset membership from rate consumption baskets and
  emits ordinary exchanges, which the kernel validates step by step.
- On Micro both traces have two selections plus finish.
- Both find no cover where only `AB` and `AC` are available.

The model's tests apply the rules directly:

- Under all 16 availability combinations, the economy accepts exactly the
  selections a direct exhaustive oracle calls covers.
- Finish requires all four encoded `Covered` facts, then consumes `Active`.

## API pressure

- Constraint satisfaction through resource-sensitive facts.
- Specialized solver compilation from core data.
- Solver output translated back to the shared execution language.

## In this folder

- `mod.rs` is the model: every instance's universe and subsets.
- `package.rs` writes its package, the only implementation of its plain
  search and Algorithm X: its economies under refs, goals, strategies,
  experiments and view, whose scripts are `scripts/exact-cover.rhai`,
  `scripts/exact-cover-algorithm-x.rhai` and `scripts/exact-cover-view.rhai`.

Exact cover is one of the [conformance problems](https://github.com/BiomaAI/axionomy/blob/main/PROBLEMS.md).
