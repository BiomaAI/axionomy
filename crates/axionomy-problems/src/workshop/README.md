# Stoichiometric workshop

A stoichiometric workshop with conserved material and labor accounting.

Showcase requires two of three funded contracts; Stress requires three of five.
Accepting a contract reserves the single workbench. Alternative cuts produce
frames and panels; assembly produces a chair or table; delivery transfers the
product to its customer and transfers the customer's finite payment back.
Tool durability becomes wear during production. Repair consumes kits, labor and
time; a one-time upgrade increases capacity and enables faster precise cuts.
Two offcuts can recover one wood unit with one irrecoverable unit of loss;
remaining offcuts become waste when the shift closes. Material, credit, tooling,
contract phases and labor/time are conserved. The exact Showcase frontier is
`(waste,time,revenue) = (2,10,22), (0,12,22), (0,13,35)`. Micro retains its original
two-recipe mass-conservation proof.

## Specification

The workshop owns wood, labor, three process-time units, and one reusable tool.
It may fire:

```text
Basic:     2 Wood + 1 Labor + 1 Time --Tool--> 1 Chair + 1 Waste
Efficient: 3 Wood + 2 Labor + 3 Time --Tool--> 2 Chair + 1 Waste
```

Labor becomes `SpentLabor`. The tool is preserved. Material and labor
accounting are global linear invariants.

Every rate fixes its roles' accounts: the shop is the workshop, and a
delivery's customer is its order's own, so no identity token pins it. The
searches therefore try the economy's own recipes, each rate's exchange that
applies, in the rates' order; no script proposes them.

The rate book deliberately includes a malformed proposal:

```text
1 Wood → 2 Chair
```

## Required results

The package's experiments find these, pinned in `tests/outcomes`:

- Waste minimization chooses the efficient batch with waste 1.
- Breadth-first search also reaches the two-chair goal.
- Exact Pareto search, ranked by waste or by time, finds the efficient
  `(waste=1,time=3)` batch and the two-basic `(waste=2,time=2)` plan.

The model's tests apply the rules directly:

- Both plans replay at those costs.
- The malformed rate is rejected by the material invariant.
- Rejection is atomic.
- Completion consumes `Active`; no recipe remains applicable afterward.

## API pressure

- Identity-changing production rather than literal asset transfer.
- Catalysts/read arcs.
- Domain-declared conservation dimensions.
- Core enforcement even when the rate proposer is wrong.
- Process tradeoffs remain terminal assets and replayable plans.

## In this folder

- `mod.rs` is the model: the Micro recipes.
- `production.rs` builds the Showcase and Stress workshops: contracts, the
  single workbench, tool wear, repair, upgrades and recycling.
- `package.rs` writes its package, the workshop's only implementation of its
  searches: its economies under refs, goals, strategies, experiments and
  view, whose scripts are `scripts/workshop.rhai` and
  `scripts/workshop-view.rhai`.

The workshop is one of the [conformance problems](https://github.com/BiomaAI/axionomy/blob/main/PROBLEMS.md).
