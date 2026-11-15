# Axionomy specification

`lean/` is the specification of the Axionomy kernel: a Lean 4 model of its
semantics with proofs of the properties that make the economy sound. It has no
dependencies beyond Lean itself and builds in seconds.

```console
cd verification/lean
lake build                                   # check every proof; warnings fail
lake exe conformance > ../conformance.json   # regenerate the fixtures
```

## How the Rust kernel is held to it

The Lean model is executable. `Conformance.lean` runs it over boundary inputs
(zero, one, the numeric limit, overflow, duplicates, every ordering of small
baskets, every combination of transaction outcomes) and writes the expected
results to `conformance.json`. The Rust test suite replays every case through
the real kernel and requires identical outcomes. CI rebuilds the specification,
regenerates the fixtures and fails if they differ from the committed file, so the
fixtures can only come from the model.

What this establishes: the specification's theorems hold for all inputs, and
the Rust kernel agrees with the specification on every generated case. The Rust
implementation itself is tested, not proved.

## What is specified

| Area | Modules | Key properties |
| --- | --- | --- |
| Identifiers | `Ordered`, `Path` | Every identifier is a path, ordered naturally: segment by segment, numbers by value and before words, a path before the longer paths it starts; the order is lawful, so entries kept in it are sorted, unique and independent of the order they came in; a name compared as text (an exchange's parameter) is ordered as the path it reads as, then by its characters |
| Exact arithmetic | `Quantity`, `MulDiv`, `RoundingDiscipline` | Checked operations are exact or reject; full-width rounded mul-div is exact and never overflows in the intermediate product; floor never exceeds and ceiling never falls below the exact value; vault round trips never create value |
| Holdings | `Basket`, `BasketOperations`, `BasketCollection`, `Account`, `AccountMap`, `State` | Construction rejects duplicates before dropping zeros; deposits and withdrawals are exact and fail without mutation |
| Rates and expressions | `Rates`, `Evaluation`, `RateConstruction`, `Discovery`, `ModelConstruction` | Expressions evaluate against one pre-exchange snapshot; the first error wins; metadata discovery is complete |
| Exchanges | `Exchange`, `Effects`, `OrderedEffects`, `Transaction`, `TransactionAggregation`, `TransactionPreparation`, `TransactionSafety`, `TransactionReceipts`, `TransactionExecution`, `Validation`, `InvariantInput` | Validation precedes effects; every account is prepared before any commits; a failed exchange changes nothing; receipts describe the actual balance change |
| Account lifecycle | `Lifecycle`, `DivergenceLifecycle` | Opened accounts are new and empty; closing removes only empty committed accounts; actual per-asset supply changes exactly by receipt deltas; valid state is preserved; rejection changes nothing; accounts afterwards are exactly those before plus the opened less the closed |
| Conservation | `Conservation` | Declared linear invariants are preserved exactly by accepted exchanges |
| History | `Execution`, `Replay`, `Observations`, `Inspection` | Ordered execution and replay compose deterministically; a replay applies its whole trace or none of it and names the action that failed; read-only queries cannot mutate |
| Divergence | `Divergence`, `DivergenceKernel`, `DivergenceLifecycle` | Two successful public replays, including opening, closing and reopening accounts, differ in per-asset supply exactly by what their receipts made, produced less consumed; shared steps make none of the difference |

Lifecycle accounting starts from `DivergenceLifecycle.Sound`: canonical
baskets, distinct account identifiers, and distinct opening roles in each
rate. These are representation invariants enforced by Rust's baskets, account
index and rate sets. `built_sound` connects them to model construction;
`apply_supply` preserves them and proves the supply equation for each step;
`replay_supply` lifts both guarantees to successful public replay. Existing
`Lifecycle.rejection_atomic` and `Execution.replay_rejection_atomic` cover
failed steps and whole-trace rollback. No assumption asserts conservation or
the supply equation being proved.

The fixtures' `pathOrder` pairs pin the kernel's `Path` order to `Path.compare`
on paths that set numbers against words, `2` against `10`, a path against
the paths it starts, and `-` within a segment against `/` between segments.

`Conformance.lean`, `TransactionConformance.lean`,
`LifecycleConformance.lean`, `BasketCollectionConformance.lean` and
`InvariantInputConformance.lean` only generate fixtures. Lifecycle cases check
multiple opens/closes, untouched holdings, reopening, shared history with
different branches, positive and negative supply changes, and rollback after
a successful lifecycle prefix. Rust compares both whole outcomes and actual
per-asset totals and receipt changes against the Lean-generated results.
