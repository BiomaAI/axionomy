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

Basket addition, scaling, containment, and equality also have implementation
proofs, including atomic addition failure, exact overflow order, and equality
independent of insertion order. Representation proofs preserve distinct assets
and the omission of zero quantities through updates and staging. Equality also
requires collection lengths representable by `usize`.

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
borrow complements the proof. The complete transaction contract below composes
this staging proof with aggregation, preservation preflight, invariant diagnostics,
and multi-account commit.

Static rate builders (`try_consume`, `try_produce`, `try_preserve`) now have
implementation proofs for exact basket merging, ordered overflow errors, role
registration, and unchanged other fields. Roles remain a separate type from
assets. These paths use an explicit sorted logical `BTreeMap`/`BTreeSet` model;
physical trees, lawful ordering, allocation and cloning remain trust boundaries.
Model construction and computed rate construction/discovery are now extracted
and proved as well. Builders retain the first account/rate under duplicate IDs,
report every duplicate in encounter order, and publish exactly the registered
model only when there are no issues. Constructors establish and preserve the
unique-key precondition used by `build`. Rate proofs cover computed effects,
conditions, canonical distinct-role pairs, and complete recursive role/parameter/
asset discovery, preserving borrowed Rust return types and all other rate fields.
Vec growth and UTF-8 lengths have explicit `usize` bounds; condition labels use
the `String` specialization. Static convenience panics, invariant constructors
and arbitrary `Into<String>` callbacks remain in item 16.

Complete structural proposal validation now has an implementation proof as well.
It returns the selected stored rate or the exact ordered list of structural issues:
zero units, missing rate, missing/unknown bindings and parameters, distinct-role
violations, and sorted deduplicated missing accounts. A missing rate ends validation
after the units check. Acceptance implies nonzero units, existing bound accounts
for every registered role, all required parameters, and the declared distinctness
constraints. Rejection always contains an issue. This covers the private production
helper called by `analyze`; the transaction proof composes its accepted and rejected
branches with condition/effect orchestration. Error-vector
growth and UTF-8 names require explicit `usize` bounds under the collection model.

The runtime `LinearInvariant::conserves` path is also extracted and proved:
every `u64 × i64` contribution fits in `i128`, every intermediate addition is
checked, and the result is exactly the ordered integer specification. `None`
means an intermediate overflow; `Some(false)` means unequal totals;
`Some(true)` implies equal mathematical weighted consumed/produced totals.
The proof covers both nested loops, missing weights (zero), negative weights,
empty deltas and preserved baskets being excluded from transferred quantities.
Whole-state `measure` now also refines the ordered checked integer fold over
all accounts. The complete transaction contract also proves the caller's error
classification and exact before/after diagnostic totals.

The recursive expression evaluator and condition helper now have implementation
proofs for every finite expression and u64 input. They preserve the original
snapshot, left-to-right error precedence, missing-input distinctions and exact
checked Nat arithmetic, including floor division and min/max. Typed internal
errors retain the existing public text; message rendering is proved for valid
UTF-8 names when the resulting message fits `usize`. Static effect scaling/merging
and computed single-quantity merging also have exact implementation proofs,
including the original target on failure. Full condition/effect orchestration
in `Economy::analyze` is now composed with these proofs. See [ACCOUNTS.md](ACCOUNTS.md) for the read-only
Arc, ordered lookup, UTF-8 and allocation boundaries.

The complete production u64 `analyze`, `assess`, and `apply` paths now refine an
ordered transaction specification. Rejection preserves the entire raw model.
Success returns the submitted proposal and specified deltas, commits exactly
the prepared accounts, and preserves account keys/order, rates, invariants, and
accounts outside the receipt deltas. Every internal lookup/`expect` precondition
on these paths is derived. Public applicability checks and finite vector candidate
filtering also refine the same original-snapshot analysis, retaining order and
duplicates. See the complete transaction contract in [ACCOUNTS.md](ACCOUNTS.md).

Production fork, simulation, ordered replay and isolated replay now refine a
state-retaining execution specification. Replay rejection leaves the accepted
prefix committed; successful receipts retain proposal order and satisfy every
registered weighted-transfer conservation check. Structural caller preconditions
are derived through the transaction frame. Physical Arc/clone isolation remains
an explicit dependency model. See the [execution contract](ACCOUNTS.md#execution-fork-simulation-and-replay).

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
| `BasketOperations` / `BasketOperationsRefinement` | Extracted addition/scaling/containment/equality, exact natural-number results/errors, unordered equality, and canonical representation preservation |
| `Basket` | Success iff asset identifiers are unique; zeros cannot hide duplicates; canonical output preserves nonzero entries, uniqueness, and order |
| `AccountRefinement` | Extracted deposit, withdrawal, and shortfall terminate and match exact natural-number outcomes under lawful equality/cloning and the collection model |
| `AccountRefinement.prepare_refines_naturals` | Extracted production exchange account staging returns exact consumption-before-production results, including shortfall and late-overflow errors |
| `Account` / `AccountMap` | Atomic rejection, exact positive shortfalls, selected-asset update frame property, and arithmetic representation conversion |
| `Quantity` | Exact checked addition/multiplication, subtraction without underflow, floor division, asset-qualified amount round trip |
| `State` | Typed account/asset updates and frame property, zero canonicalization, complete scalar shortfall, hidden-account projection, observation noninterference, monotone goals |
| `RateConstruction` / `RateConstructionRefinement` | Extracted static builders, exact per-role merges/errors, role registration and unchanged rate fields under the sorted collection model |
| `ModelConstruction` / `ModelConstructionRefinement` | Extracted model builders, exact ordered duplicate errors, first-entry retention, unique-key preservation and complete build result |
| `Discovery` / `DiscoveryRefinement` / `RateMetadataRefinement` / `RateDiscoveryRefinement` | Extracted computed/condition/distinct construction, exact recursive roles, sorted parameter union, complete ordered asset references and unchanged fields |
| `Validation` / `ValidationCompleteRefinement` | Extracted full structural validation, exact selected rate or ordered diagnostics, nonempty rejection, accepted binding/account/parameter existence and distinctness |
| `Conservation` / `ConservationRefinement` | Extracted runtime invariant check and whole-state measurement, exact signed widening/weighting, intermediate overflow and mathematical conservation on acceptance |
| `Evaluation` / `EvaluationRefinement` / `DiagnosticRefinement` | Extracted recursive expressions and conditions against one snapshot, all checked outcomes, first-error order, exact public error text |
| `Effects` / `EffectsRefinement` | Extracted bounded static scaling/merging and computed single-quantity addition, exact errors and atomic failure |
| `Transaction` / `AnalysisRefinement` / `ApplyRefinement` | Complete extracted transaction analysis, assessment and application; ordered stages/errors, atomic rejection, exact receipts/state and unchanged frame |
| `AggregationContract` / `TransactionSafety` / `RateWellFormed` | Builder-established target registration; canonical, sorted unique existing affected accounts; derived staging/commit preconditions |
| `FilteringRefinement` | Extracted boolean applicability and stable finite-vector filtering, preserving duplicates against the same snapshot |
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

### Active completion goal

Complete the current root crate's production-to-Lean contracts in the four
milestones below. Keep Rust's typed domains, numeric backends, and ownership
guarantees. A milestone is complete only when its composed implementation
theorems, caller preconditions, failure behavior, fresh extraction, conformance,
mutation controls, and workspace checks pass. Commit and push validated work
to `origin/main`; theorem counts do not measure percentage completion.

| Milestone | Item | Contract | Status |
| --- | --- | --- | --- |
| 1. u64 transaction kernel | 1 | Basket addition/scaling, containment/equality, canonical representation preservation | Implementation proved under documented models |
| 1 | 2 | Model/rate construction, duplicate IDs, role/parameter/asset discovery, distinct roles | Implementation proved under documented models; remaining convenience APIs tracked in item 16 |
| 1 | 3 | Exchange units, rate/account existence, binding/parameter validation | Implementation proved under documented models; composed through complete transaction execution |
| 1 | 4 | Full expression/condition evaluation against the original snapshot | Implementation proved: complete ordered condition loop and original-snapshot evaluation |
| 1 | 5 | Account effect aggregation, bounded scaling/merging, computed-effect semantics | Implementation proved: complete static/computed aggregation, aliasing and canonical/sorted/unique existing-account invariants |
| 1 | 6 | Consumption plus preservation availability and complete shortfalls | Implementation proved: consume plus preserve, exact ordered assessments and complete shortfalls |
| 1 | 7 | Complete preparation, unique affected accounts, established staging preconditions | Implementation proved: complete staging, unique existing accounts and exact ordered deltas |
| 1 | 8 | Runtime signed conservation, intermediate i128 overflow, error classification | Implementation proved: complete runtime invariant checks, checked intermediate measures and exact diagnostics |
| 1 | 9 | Multi-account atomic commit, unchanged frame, safe prepared-account lookups | Implementation proved: safe lookups, atomic rejection, exact committed state and unchanged frame |
| 1 | 10 | Assessment/application agreement, filtering, immutability, exact diagnostics/order | Implementation proved: analyze/assess/apply agreement, boolean applicability and stable finite-vector filtering; arbitrary iterators remain in item 16 |
| 1 | 11 | Receipts/deltas agree with the actual proposal and committed changes | Implementation proved: submitted proposal, unique receipt accounts and exact observable committed balance for every delta |
| 2. Execution and observations | 12 | Fork/simulation isolation and equivalence | Implementation proved under documented logical storage/clone models |
| 2 | 13 | Ordered traces, replay prefix-on-error, isolated replay, invariant composition | Implementation proved: first error, accepted prefix state, ordered receipts and derived caller invariants; generic trace extension remains in item 16 |
| 2 | 14 | Goals, visibility, hidden-state noninterference, missing/zero distinctions | Design proofs |
| 2 | 15 | Canonical state/observation keys and deterministic fingerprints | Open |
| 3. Public data boundary | 16 | Remaining typed constructors/accessors/conversions/iterators/equality | Partial implementation proofs |
| 3 | 17 | Serialization round trips, validation/canonicalization, accurate schema contracts | Open |
| 4. Numeric backends | 18 | BigUint/BigInt and explicit algebraic requirements for custom backends | Open |

The completed u64 transaction milestone extracts the complete production `assess`/`apply`
path. `Transaction` and `TransactionDecode` specify its ordered stages and typed
errors. The refinements compose structural validation, conditions, complete
static/computed aggregation, preflight, preparation, and invariant diagnostics
into the production `analyze` and public `assess`/`apply` paths. These establish exact
ordered results and termination without panic under the explicit dependency
models, lawful identifier operations, registered computed targets, and resource
bounds. Aggregation derives canonical baskets, sorted unique existing accounts,
and a size bound from the model; those properties discharge the preflight and
preparation preconditions. Rate-builder proofs establish and preserve computed
target registration. Preparation establishes ordered account/delta correspondence
and the existence needed by the commit loop. The complete `apply` refinement
discharges its internal `expect` preconditions, preserves the entire original
model on rejection, and returns the specified receipt and committed model on
success. Its frame property preserves account keys, rates, invariants, and
accounts outside the receipt deltas. Account ordering requires
reflexivity, equality consistency, transitivity, and reversal of greater-than;
matching Rust comparisons to a logical function alone is insufficient.

The Lean fixture generator now produces 596 complete transaction scenarios;
the public Rust API matches their exact assessments, diagnostics, receipts,
post-state, and rejection behavior. The scenarios include aliasing, original
snapshot evaluation, preservation, overflow, and conservation diagnostics.
These finite checks supplement the general proofs. Filtering now has an
implementation proof for finite vector candidates. Arbitrary user-provided
iterator behavior remains a separate public-boundary obligation in item 16.

The transaction contracts are registered in `coverage.json` and the mandatory
proof gate. This milestone passed fresh extraction, the Lean kernel/axiom audit,
generated conformance, all verification controls (including production mutations),
and Rust 1.89 workspace formatting, Clippy, tests and documentation checks.
Milestone 2 is partially complete (items 12–13); items 14–18 remain open or partial. This is not full core coverage.

The execution paths now compose the transaction proof. `ExecutionRefinement`
proves production `fork`, `simulate`, `replay` and `replayed`, including exact
ordered receipts and error results. The replay loop derives each next step's
metadata/resource conditions from the previous step's preserved rates, invariants
and account keys. `Execution` retains the state on failure; `TransactionExecution`
proves that this is exactly the successful prefix state, and that each successful
receipt describes the balance change at its own intermediate state. Trace
construction, append-by-push and borrowed/consuming access preserve proposal order.
See [the execution contract](ACCOUNTS.md#execution-fork-simulation-and-replay).
The fixture bridge adds 246 replay scenarios and checks simulation against all
596 existing transaction cases. Generic `Trace::extend` and serialized traces
remain in items 16–17. Physical clone/Arc isolation remains a dependency boundary;
Rust's immutable source borrows and typed APIs remain intact.
This execution slice passed the full proof gate (238 registered contracts), fresh
extraction and kernel/axiom audit, 246 replay plus 596 simulation conformance
scenarios, 294 workspace tests, 75 boundary/gate controls and three new production
mutation controls. Formatting, Clippy with warnings denied, documentation and
compile-fail doctests also passed. The prior 32 production controls remain in CI.

Items 14–15 include a reproduced observation-key gap across different account
domains: a visible empty account and an absent account currently produce the same
key, although `balance` returns `Some(0)` for the former and `None` for the latter.
The observation contract must preserve that distinction in its declared
equivalence; identical account domains cannot be silently assumed for arbitrary
models. This remains open in the execution/observation milestone.

Every internal `expect` needs a discharged precondition. Intentionally panicking
convenience constructors need explicit contracts; do not claim unconditional
panic freedom for them. Generic numeric proofs require algebraic laws beyond
Rust's trait bounds. Prove canonical-key properties for their declared
equivalence, never collision freedom of finite fingerprints.

The existing compiler/translation/primitive and collection/allocation/clone
trust boundaries remain explicit. Verifying those dependencies themselves is
outside this goal. Game rules, equilibrium, search, transport, and Studio are
also outside it. The detailed legacy gaps below remain until their production
contracts are discharged; updating this table does not certify code.

### Per-feature checks

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
finite regression evidence. The separate `ApplyRefinement.apply_refines` theorem
now connects the complete production `Economy::apply` path to its specification.
Separate existing Rust tests cover additional behavior.

The basket operation specification adds 654 cases for addition, scaling,
containment, and equality. It checks ordered results and errors, zero
canonicalization, and unchanged inputs. Another 56 rate-merge cases run through
all three builders, checking registered roles and unaffected effect maps. The
464 signed-conservation cases include both i64 extremes, missing/zero weights,
positive and negative overflow, cancellation reordered within/across accounts,
and all three outcomes. The private conservation method is exercised directly
by a crate unit test included in the proof gate. Another 463 cases cover
whole-state measurement. The new evaluator corpus has 2,210 expressions and
6,630 condition checks, including nested errors, all comparisons, Unicode names,
missing bindings/accounts/assets, zero units and u64 boundaries. Another 73 cases
exercise atomic static merging and unscaled computed quantities. These run
directly through the production private helpers, alongside existing public
whole-economy tests; they do not substitute for the implementation theorems.
An additional 345 model sequences and 77 rate sequences check every construction
prefix against Lean-generated snapshots, using both `new` and `default`. They
cover interleaved duplicate IDs, retained values, exact issue order, all expression
constructors, repeated/reversed/self distinct pairs, Unicode names, all computed
kinds, all condition comparisons and repeated asset references.
Another 765 structural-validation cases cover all eight issue categories, exact
ordering, zero-plus-missing-rate early return, accepted rate selection, aliased
missing accounts, distinct pairs, unknown/missing parameters, Unicode names and
u64 boundaries. Each runs both directly and through `assess`/`apply`, with and
without a failing condition, checking structural-error precedence and unchanged
state. These caller checks are finite integration evidence, not caller proofs.

## Explicit legacy refinement gaps

The following existed before this proof requirement and are not discharged by
the initial theorems. They are a backfill plan, not an exemption for new behavior:

- Charon/Aeneas extraction and refinement beyond the four `Quantity<u64>` checked
  arithmetic operations, basket construction/operations, account updates/staging,
  model/rate construction and metadata discovery, complete u64 transaction
  assessment/application, finite-vector filtering, fork/simulation/replay and ordered trace access, snapshot expression/condition
  evaluation, bounded effect helpers, runtime conservation and whole-state measurement.
- Correctness of `IndexMap`, `BTreeMap`, `Arc`, iterator order, allocation, cloning,
  and copy-on-write isolation; Rust compilation still enforces its ownership rules.
- Constructor and deserialization composition for the remaining public APIs.
  Transaction caller preconditions are now discharged through structural
  validation, aggregation, preparation and commit; arbitrary deserialized metadata
  still needs its separate representation contract.
- Remaining finite-map adapters, serialization/schema round trips,
  canonical state/observation ordering, and fingerprints. Fingerprints are not
  collision-free identities and must not be given such a theorem.
- Invariant constructors and other remaining convenience APIs. Runtime invariant
  checks, intermediate `i128` overflow, and caller diagnostic totals are now
  covered by the complete transaction contract.
- Laws for custom `QuantityScalar` backends and BigUint/BigInt implementations.
  Rust trait bounds specify operations, not their algebraic correctness.
- Rust replay/fork refinement and its composition with the transaction contract.
  Rust `replay` retains an accepted prefix on failure; `replayed` protects its source.

For further implementation proofs, extend the pinned Aeneas pipeline from u64
arithmetic, baskets, accounts, model/rate construction, discovery, validation, evaluation, effects and conservation
to execution/observation and remaining public data contracts, connecting
extracted definitions to the existing specifications. Model unavoidable external dependencies explicitly and
report their trust status. Do not maintain a second Rust algorithm only for proofs
or change the public API to an untyped representation to satisfy extraction.

The specification also deliberately avoids universal batch/sequential equivalence:
consume-one/produce-one can fire twice from one token, while a two-unit batch
requires two tokens upfront. Computed effects are evaluated once against the
pre-exchange snapshot and are not automatically multiplied by exchange units.

References: [Aeneas workflow and limitations](https://github.com/AeneasVerif/aeneas),
[Lean/Aeneas introduction](https://lean-lang.org/use-cases/aeneas/).
