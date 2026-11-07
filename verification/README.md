# Proof-bearing core

Every new or changed Axionomy core feature carries a Lean specification and a
checked proof of its relevant contract. Rust remains the implementation language
and retains ownership, borrowing, generic ontology types, private state, and
exact numeric backends. Proofs are an additional requirement, not a replacement
for static typing or runtime validation.

## Current assurance level

**Partial implementation verification.** The four production `Quantity<u64>`
checked arithmetic operations are extracted through Charon/Aeneas. Lean proves
that their generated implementations terminate without panic and return exactly
the specified result for every pair of u64 inputs, including checked rejection.
This is conditional on the documented compiler, translation, and primitive-model
trust boundary; it is not a machine-code verification claim.

Basket construction also has an Aeneas refinement proof for u64 quantities and
finite vector inputs with lawful asset equality, under an explicit ordered-map
model. It proves duplicate rejection (including zeros), exact nonzero contents,
and preserved insertion order. See [the basket proof](BASKETS.md) for the
shared-source extraction and the trusted storage boundary.

Account deposits and withdrawals also have implementation proofs for u64
quantities and lawful asset equality/cloning under that storage abstraction.
They match a natural-number specification for final balances, ordered errors,
complete shortfalls, and atomic rejection. Both subtraction `expect` calls are
proved safe on these paths; withdrawal requires distinct requested assets.
See [the account proof](ACCOUNTS.md) for the exact scope and source refactors.

Per-account exchange staging also has an implementation proof. The production
helper clones an account, withdraws consumed assets, and deposits produced assets.
Its Aeneas extraction returns exactly the specified new account or error; a later
deposit overflow returns no partially prepared account. Rust's immutable source
borrow complements the proof. This does not yet verify the multi-account commit,
effect aggregation, preservation preflight, or invariant validation.

The other core contracts still have handwritten design proofs and finite Rust
conformance tests. Neither an entire source module nor every existing core
behavior is declared proved just because it has a coverage entry.
See [the Aeneas integration](AENEAS.md) for the exact scope and reproduction steps.

`coverage.json` maps core feature contracts to checked theorem names, Rust test
files, and explicit limitations. All root `src/**/*.rs` files, `Cargo.toml`,
`Cargo.lock`, the extraction entry points, and the pinned extraction toolchain
and its reviewed backend patch must be accounted for. SHA-256 entries record the exact reviewed source version;
changing source invalidates the review until the proof relationship is reviewed
and the entry is deliberately updated. Hashes do not mechanically establish that
a theorem describes Rust. Reviewers must check that relationship and the adequacy
and satisfiability of theorem premises. Adding an unrelated theorem is not coverage.

The current library covers:

| Module | Checked properties |
| --- | --- |
| `QuantityRefinement` | Actual Aeneas-extracted u64 addition, subtraction, multiplication, and division refine `Quantity`, including all checked failures |
| `BasketRefinement` | Extracted production construction refines `Basket` for u64 vector inputs, under lawful equality and the explicit collection model |
| `Basket` | Success iff asset identifiers are unique; zeros cannot hide duplicates; canonical output preserves nonzero entries, uniqueness, and order |
| `AccountRefinement` | Extracted deposit, withdrawal, and shortfall terminate and match exact natural-number outcomes under lawful equality/cloning and the collection model |
| `AccountRefinement.prepare_refines_naturals` | Extracted production exchange account staging returns exact consumption-before-production results, including shortfall and late-overflow errors |
| `Account` / `AccountMap` | Atomic rejection, exact positive shortfalls, selected-asset update frame property, and arithmetic representation conversion |
| `Quantity` | Exact checked addition/multiplication, subtraction without underflow, floor division, asset-qualified amount round trip |
| `State` | Typed account/asset updates and frame property, zero canonicalization, complete scalar shortfall, hidden-account projection, observation noninterference, monotone goals |
| `Rates` | Checked expression evaluation refines mathematical evaluation and bounds its result; effect merging is associative/commutative in exact arithmetic; preservation does not scale |
| `Exchange` | Exact prepared deltas, preserved resources, assessment/application agreement, atomic rejection, output bounds, signed weighted conservation |
| `Replay` | Trace concatenation, deterministic outcomes, invariant induction given an invariant-preserving step |

`Exchange` starts with aggregated effects for distinct account/asset entries.
Its arbitrary signed weights prove conservation algebra over mathematical integers.
`Rates` assumes a typed lookup context. `State` is an extensional map; it does not
claim that every function has finite support. The sparse representation proof
covers individual entries. `Replay`'s step premise must be discharged when linking
it to the Rust exchange implementation; it does not prove arbitrary transitions safe.

## Required feature workflow

1. State the feature's contract and failure behavior in Lean. Preserve separate
   account, asset, role, parameter, and numeric meanings.
2. Prove the property with explicit premises, including overflow and invalid
   input behavior where relevant. Check that valid examples satisfy the premises.
3. Implement or update the typed Rust API and add meaningful conformance evidence.
   Reuse the Lean definitions in generated cases where they are executable.
4. Update the coverage mapping, exact limitations, and reviewed source hashes.
   Refactors may reuse proofs if their connection to Rust is reviewed; semantic
   additions require corresponding contracts and proofs. Do not weaken a theorem
   or extend the legacy gap list merely to make a new feature pass.
5. Run the proof gate and appropriate Rust tests. Review both the theorem statements
   and the source-to-spec relationship. CI cannot decide whether a specification
   adequately captures a feature's intent.

The root `AGENTS.md` makes this workflow persistent for coding agents. CI's
`proofs` job executes the same local command. Repository administrators should
include `proofs` in required branch checks; adding a workflow does not change
GitHub branch-protection settings.

## Run locally

Install Python 3.12+, Git, rustup, and [elan](https://github.com/leanprover/elan), then run from the repository root:

```sh
python3 scripts/setup_aeneas.py
(cd verification/lean && lake update)
python3 scripts/check_proofs.py
python3 -m unittest discover -s scripts -p 'test_*.py'
```

The project pins Lean in `lean/lean-toolchain`, and pins Aeneas/Charon in
`aeneas-toolchain.json`. The Aeneas backend brings pinned Mathlib dependencies.
The toolchain pin also records a checksum-guarded Lean support patch that prevents
generated enum instance-name collisions; it changes no translated operations.
Rust builds do not require Lean. The proof command regenerates the implementation
model from Rust, checks it against the committed extraction, builds the Lean
library, rechecks compiled proof terms with `leanchecker`, audits every Axionomy
declaration and registered theorem, checks that
the generated conformance cases match the committed file, runs those cases
against Rust, and runs Rust doctests including compile-fail type contracts.

The audit allows only `propext`, `Classical.choice`, and `Quot.sound`, Lean's
standard logical foundations. It rejects `sorryAx`, custom axioms, and extra
native-evaluation trust, including dependencies hidden behind helper declarations.
All specification modules must be imported by `lean/Axionomy.lean`.

When intentionally changing executable specifications, regenerate the cases:

```sh
cd verification/lean
lake build
./.lake/build/bin/conformance > ../conformance.json
```

Formatting of JSON is immaterial. Review the actual changed values. The cases
include u64 boundaries, overflow, underflow, zero units/divisors, preserved
thresholds, conservation, computed effects, parameter lookup, conditions and
pre-exchange snapshots. They also include 260 basket cases and 328 account update
cases covering duplicates, zero quantities, order, exact errors, atomic rejection,
and u64 boundaries. Another 492 staging cases run through two-account exchanges,
checking assessment, exact errors and receipts, an earlier staged account on late
failure, unaffected accounts, and fork isolation. These whole-economy checks are
finite regression evidence, not a refinement theorem for `Economy::apply`.
Separate existing Rust tests cover additional behavior.

## Explicit legacy refinement gaps

The following existed before this proof requirement and are not discharged by
the initial theorems. They are a backfill plan, not an exemption for new behavior:

- Charon/Aeneas extraction and refinement beyond the four `Quantity<u64>` checked
  arithmetic operations, basket construction, account deposit/withdrawal, and
  per-account exchange staging, including the remaining exchange `expect` and
  termination obligations.
- Correctness of `IndexMap`, `BTreeMap`, `Arc`, iterator order, allocation, cloning,
  and copy-on-write isolation; Rust compilation still enforces its ownership rules.
- Full structural validation: required/unknown roles and parameters, account/rate
  existence, distinct bindings, duplicate model IDs, and aggregate overflow.
- Sparse finite-map operations beyond the proved constructor, serialization/schema round trips,
  canonical state/observation ordering, and fingerprints. Fingerprints are not
  collision-free identities and must not be given such a theorem.
- Refinement of checked signed invariant accumulation to mathematical `Int`,
  including intermediate `i128` overflow and error classification.
- Laws for custom `QuantityScalar` backends and BigUint/BigInt implementations.
  Rust trait bounds specify operations, not their algebraic correctness.
- Complete diagnostic/error ordering, receipts, and Rust replay/fork refinement.
  Rust `replay` retains an accepted prefix on failure; `replayed` protects its source.

For further implementation proofs, extend the pinned Aeneas pipeline from u64
arithmetic, baskets, accounts, and account staging to complete exchange preparation and commit, connecting
extracted definitions to the existing specifications. Model unavoidable external dependencies explicitly and
report their trust status. Do not maintain a second Rust algorithm only for proofs
or change the public API to an untyped representation to satisfy extraction.

The specification also deliberately avoids universal batch/sequential equivalence:
consume-one/produce-one can fire twice from one token, while a two-unit batch
requires two tokens upfront. Computed effects are evaluated once against the
pre-exchange snapshot and are not automatically multiplied by exchange units.

References: [Aeneas workflow and limitations](https://github.com/AeneasVerif/aeneas),
[Lean/Aeneas introduction](https://lean-lang.org/use-cases/aeneas/).
