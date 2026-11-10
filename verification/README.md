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
| Exact arithmetic | `Quantity`, `MulDiv`, `RoundingDiscipline` | Checked operations are exact or reject; full-width rounded mul-div is exact and never overflows in the intermediate product; floor never exceeds and ceiling never falls below the exact value; vault round trips never create value |
| Holdings | `Basket`, `BasketOperations`, `BasketCollection`, `Account`, `AccountMap`, `State` | Construction rejects duplicates before dropping zeros; deposits and withdrawals are exact and fail without mutation |
| Rates and expressions | `Rates`, `Evaluation`, `RateConstruction`, `Discovery`, `ModelConstruction` | Expressions evaluate against one pre-exchange snapshot; the first error wins; metadata discovery is complete |
| Exchanges | `Exchange`, `Effects`, `OrderedEffects`, `Transaction`, `TransactionAggregation`, `TransactionPreparation`, `TransactionSafety`, `TransactionReceipts`, `TransactionExecution`, `Validation`, `InvariantInput` | Validation precedes effects; every account is prepared before any commits; a failed exchange changes nothing; receipts describe the actual balance change |
| Account lifecycle | `Lifecycle` | Opened accounts are new and closed accounts are left empty by the exchange's own effects; a rejected exchange changes nothing; every receipt conserves every invariant; the accounts afterwards are exactly those before plus the opened less the closed |
| Conservation | `Conservation` | Declared linear invariants are preserved exactly by accepted exchanges |
| History | `Execution`, `Replay`, `Observations`, `Inspection` | Ordered execution and replay compose deterministically; a replay applies its whole trace or none of it and names the action that failed; read-only queries cannot mutate |

`Conformance.lean`, `TransactionConformance.lean`,
`BasketCollectionConformance.lean` and `InvariantInputConformance.lean` only
generate fixtures.
