# Proof-bearing core

Every new or changed Axionomy core feature carries a Lean specification and a
checked proof of its relevant contract. Rust remains the implementation language
and retains ownership, borrowing, generic ontology types, private state, and
exact numeric backends. Proofs are an additional requirement, not a replacement
for static typing or runtime validation.

Diagnostic `Debug`/`Display` rendering (text, field layout, indentation and writer
behavior) is outside the required Lean proof scope. Public APIs and supporting
operations remain in scope when they establish engine guarantees. In particular,
typed input validation, state equality, cloning for isolated execution, ordering,
and canonical state/observation keys retain their proof requirements. Diagnostic
formatting is not a prerequisite for completing any milestone below.

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
the `String` specialization. The inspection batch below proves invariant
construction, including its `Into<String>` success/failure boundary. Static
u64 convenience panics and string-conversion results have separate contracts below;
other arbitrary callback contracts remain in item 16.

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

Full-width rounded mul-div also has an implementation proof. For every triple
of u64 inputs and both `Rounding` directions, the extracted
`QuantityScalar::checked_mul_div` widens to `u128`, multiplies, divides and,
for `Ceil` with a remainder, adds one without any primitive failure, returning
exactly `MulDiv.mulDiv u64Max`: `None` iff the denominator is zero or the
rounded quotient exceeds u64. The product itself is never range-checked. The
evaluator's `MulDiv` node evaluates its operands in order, reports a zero
denominator before attempting the quotient, and is covered by the same
evaluator refinement. Discovery, asset inspection, construction and the derived
`Rounding` traits are proved alongside; `BigUint` and custom backends are tested
only. See the `full_width_mul_div` contract in `coverage.json`.
This milestone passed the full proof gate (708 registered contracts, 4,525
audited Axionomy theorems), fresh extraction, 468 new Lean-generated evaluation
cases (all 2,678 independently recomputed with a wide-integer reference), all
372 workspace tests, 110 boundary/gate/extraction controls and production
mutations, including a new control that drops the ceiling increment. Formatting
and Clippy with warnings denied also passed.

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
| `RoundingDiscipline` | Model-level laws: floor payouts never exceed and ceiling charges never fall below exact values; vault deposit/mint round trips never profit; withdrawals burn enough shares; explicit-remainder splits conserve |
| `MulDiv` / `MulDivRefinement` | Exact rounded quotients with full-width products, floor/ceil bounds and monotonicity; extracted u64 widening, rounding and narrowing return exactly the specification for every input |
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
| 1 | 10 | Assessment/application agreement, filtering, immutability, exact diagnostics/order | Implementation proved: analyze/assess/apply agreement, boolean applicability and stable finite-input iterator filtering under explicit iterator laws |
| 1 | 11 | Receipts/deltas agree with the actual proposal and committed changes | Implementation proved: submitted proposal, unique receipt accounts and exact observable committed balance for every delta |
| 2. Execution and observations | 12 | Fork/simulation isolation and equivalence | Implementation proved under documented logical storage/clone models |
| 2 | 13 | Ordered traces, replay prefix-on-error, isolated replay, invariant composition | Implementation proved: first error, accepted prefix state, ordered receipts and derived caller invariants; generic trace extension also proved under explicit iterator laws |
| 2 | 14 | Goals, visibility, hidden-state noninterference, missing/zero distinctions | Implementation proved and validated; finite-input iterator construction and documented collection models |
| 2 | 15 | Canonical state/observation keys and deterministic fingerprints | Implementation proved and validated under documented sorting/hash models and native 64-bit little-endian scope |
| 3. Public data boundary | 16 | Remaining typed constructors/accessors/conversions/iterators/equality | Partial: foundational typed data, expression construction, iterator consumers, basket collection boundaries, core inspection, u64 convenience constructors, derived traits and inherited helpers proved under documented models; remaining APIs listed below |
| 3 | 17 | Serialization round trips, validation/canonicalization, accurate schema contracts | Partial: invariant weight validation/normalization proved; serializer/codec composition and other inputs remain open |
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
implementation proof for finite vector candidates. The iterator-consumer batch below extends this to finite user-provided
streams under independent IntoIterator/next laws; arbitrary callback implementations
still require their own evidence.

The transaction contracts are registered in `coverage.json` and the mandatory
proof gate. This milestone passed fresh extraction, the Lean kernel/axiom audit,
generated conformance, all verification controls (including production mutations),
and Rust 1.89 workspace formatting, Clippy, tests and documentation checks.
Items 1–15 are validated within their documented domains and dependency models.
Items 16–18 remain open or partial. This is not full core coverage, and the item
count is not a percentage of implementation effort.

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
596 existing transaction cases. Generic `Trace::extend` is covered by the iterator-consumer batch below;
serialized traces remain in item 17. Physical clone/Arc isolation remains a dependency boundary;
Rust's immutable source borrows and typed APIs remain intact.
This execution slice passed the full proof gate (238 registered contracts), fresh
extraction and kernel/axiom audit, 246 replay plus 596 simulation conformance
scenarios, 294 workspace tests, 75 boundary/gate controls and three new production
mutation controls. Formatting, Clippy with warnings denied, documentation and
compile-fail doctests also passed. The prior 32 production controls remain in CI.

Items 14–15 fix the observation-key gap across account domains: keys now include
`existing_accounts`, so a visible empty account and an absent account differ,
matching `Some(0)` versus `None` balance results. The new field is required in
serialized keys; old keys are rejected because their balances cannot recover
empty-account presence. Arbitrarily deserialized keys still need item 17's
validation contract. See [the observation and key contracts](ACCOUNTS.md#goals-views-and-canonical-keys).

The new refinements cover goal new/default/require/requirements/asset discovery,
matching, account/balance queries, finite-vector view construction, both key
builders and accessors, and the fingerprint byte loop and caller. Equal produced
observation keys imply equal actual public balance queries for canonical baskets;
hidden accounts do not contribute. Sorting derives a canonical permutation under
independent identifier order laws. State keys deliberately omit empty-account
presence and metadata, so they are not identities of complete economy models.
Fingerprint proofs describe FNV-1a, never collision freedom. Hashing is scoped to
native 64-bit little-endian targets and default-method byte streams with explicit
identifier encoding laws. The extraction guard enforces the platform scope.
504 Lean-generated observation/goal/key/fingerprint scenarios supplement the proofs.

This observation milestone passed the full proof gate (281 registered contracts,
3,083 audited Axionomy theorems), fresh extraction, all 298 workspace tests,
80 boundary/gate controls and five new production mutation controls. The mutations
confirm that missing-goal acceptance, reversed visibility, omitted empty-account
presence, skipped state sorting and an incorrect FNV prime break their implementation
proofs. Formatting, Clippy with warnings denied, documentation and compile-fail
doctests also passed. The prior 35 production mutation controls remain in CI.

The remaining completion work is:

**Completed item 17 invariant input validation:** the production loader now rejects every
repeated asset weight, including duplicates preceded by zero. Its extracted
`from_serialized_weights` helper proves first-duplicate rejection, termination,
name preservation, exact signed coefficients and ordered zero normalization.
Accepted weights are unique and nonzero. Rust's existing asset bounds are
unchanged. The serde handoff is source-guarded and tested; the codec itself is
not proved. See [the input contract](ACCOUNTS.md#invariant-input-validation).

This milestone passed the full proof gate (666 registered contracts, 4,338
audited Axionomy theorems), fresh extraction, 262 regenerated Lean conformance
cases, all 348 workspace tests and 91 boundary/gate/extraction controls,
including the new production mutation that drops zero weights before duplicate
validation. Formatting and Clippy with warnings denied also passed.

**Completed item 16 inherited-trait batch:** extracted 58 public forwarding roots for
29 `clone_from` methods, 19 `ne` methods (including basket/account equality),
quantity/fingerprint `min`/`max`/`clamp`, and four `hash_slice` methods. The
pinned std defaults and backend representations have been reviewed. Four proof
modules compile with 78 contracts for result composition, tie selection, clamp
rejection and hash traversal under explicit callback laws. Lean-generated order,
clamp and hash-chunk fixtures pass the Rust bridge, including 36 generated tagged
cases that distinguish equal-key values. The full gate passed with 655 registered
contracts and 4,289 audited Axionomy theorems. All 333 Rust workspace tests in 59
groups, workspace doctests, formatting, Clippy with warnings denied and strict
documentation builds passed. All 105 boundary/gate controls passed. Both new
production mutations compiled after re-extraction and then broke the intended
semantic proofs: ignored clone replacement and omitted slice hashing. The prior
54 production mutation controls remain in CI; all 56 source targets were checked.
Item 16 remains partial, and items 17–18 remain open.
See [the exact dependency boundary](ACCOUNTS.md#inherited-public-traits).

**Completed item 16 derived-trait batch:** implementation contracts cover equality/clone on typed
values, recursive expressions, conditions, proposals/traces, errors and observation
keys, plus quantity/fingerprint ordering and typed hashing. All remaining public
derived clone APIs and account-error equality are included: 62 new API probes and
one generic clone forwarding helper. Fresh extraction and eight proof
modules compile, with 90 registered contracts: typed values, recursive expressions,
record/error payloads, generic basket/account equality, all public derived clones,
proposal/trace/model-build-error/observation-key equality, numeric ordering and
scoped hash composition. Reviewed BTreeMap and tuple equality models preserve
field order, short-circuiting and callback failures. Hash contracts retain the
native/default-method restrictions, and pure clone contracts retain the logical
tuple/collection/Arc boundary; see [the exact scope](ACCOUNTS.md#derived-public-traits).
Lean-generated comparisons and encoding cases pass seven Rust tests, including
generic non-Copy BigUint payload transport. All 48 account-extraction boundary
tests pass, including removal of each of 54 production methods and bypass of all
63 forwarding roots. The full proof gate passed (577 registered contracts; 4,192
audited Axionomy theorems), along with all 329 Rust 1.89 workspace tests in 58
groups, formatting, Clippy with warnings denied, strict documentation builds and
workspace doctests. All 102 boundary/gate controls passed. Both new re-extracted
production mutants compiled and then broke their semantic proofs: reversed amount
hash order and observation-key equality omitting existing accounts. The prior 52
production mutation controls remain in CI; all 54 source targets were checked.
Remaining iterator/callback adapters remain visible obligations alongside
items 17–18. The inherited convenience methods are covered by the
separate batch above; this derived-method batch does not establish those contracts
by itself.

**Completed item 16 convenience-constructor batch:** implementation proofs cover
`Rate::consume`/`produce`/`preserve` overflow panics, duplicate rejection by the
`basket` literal helper, and `Into<String>` results in expression/proposal
parameters, invariant names and condition labels. Seven forwarding roots connect
the production APIs to Aeneas. The sole new external interface is std array-map,
with a reviewed model that threads callback state in input order and propagates
failure. The 26 contracts are registered in the mandatory gate; 781 Lean-generated
literal arrays, 72 callback cases and the 56 existing merge cases connect them to
Rust. This batch passed the full proof gate (487 registered contracts; 3,706
audited Axionomy theorems), fresh extraction, all 322 workspace tests in 57 groups,
99 boundary/gate controls and both new production mutation controls. Rust 1.89
formatting, Clippy with warnings denied, documentation and workspace doctests
passed. The prior 50 production mutation controls remain in CI; all 52 source
targets were checked. Item 16 remains partial.
See [the exact contract and dependency boundary](ACCOUNTS.md#convenience-constructors-and-string-conversion-callbacks).

**Completed item 16 inspection batch:** assessment lookup and
rate/invariant/model inspection now have fresh extraction for 20 public APIs plus
seven iterator-consumption probes. The development `Inspection*Refinement` modules
compile, including first-match lookup with comparison failure propagation, generic
invariant weight updates, borrowed iterator contents and exact size hints, and
complete model asset collection from balances, registered static rate roles,
computed effects/conditions and invariant weights. The numeric payloads remain
generic; asset collection does not assume arithmetic laws. The pinned Aeneas
filtering mode retains concrete iterator methods while omitting its unsupported
generic `size_hint` dictionary field. A diagnostic guard permits only the exact
reviewed warning; other warnings/errors and missing concrete definitions fail.
Sixty contracts are registered in the mandatory gate, and 537 Lean-generated
scenarios check lookup, weight updates and inspection. This batch passed the full
proof gate (461 registered contracts; 3,654 audited Axionomy theorems), fresh
extraction, all 319 workspace tests in 56 groups, 97 boundary/gate controls and
all three new production mutation controls. Rust 1.89 formatting, Clippy with
warnings denied, documentation and workspace doctests passed. The prior 47
production mutation controls remain in CI, with all source targets checked.
See the
[inspection contract](ACCOUNTS.md#core-inspection-and-complete-asset-collection).

- **16 — Typed public APIs:** the foundational batch now has composed proofs for
  amounts, quantity validation/conversion, account wrappers/equality, typed basket
  insertion, proposal builders, receipt/delta/assessment accessors and expression
  constructors. Generic trace extension, finite-input iterator views and candidate
  filtering are also proved under independent iterator laws. Basket construction
  from entries and typed amounts, FromIterator/array conversions, and basic borrowed
  iteration/queries including sorted views are now covered as well. Remaining work
  includes remaining collection iterator and arbitrary callback contracts,
  arbitrary erased hash overrides/portability beyond the scoped models.
  The u64 static-rate and basket
  convenience panic contracts are covered by the batch above. See the [public data contracts](ACCOUNTS.md#typed-public-data-and-expression-construction)
  for the exact scope of this batch.
- **17 — Serialization and schema:** connect production serialization and
  deserialization to round-trip, rejection and canonicalization contracts. Cover
  quantity validation, duplicate basket entries, rate normalization, economy
  metadata, traces, receipts, goals and keys. Check schemas against accepted values,
  including custom signed scalar schemas and inconsistent serialized key fields.
  Invariant weight validation is covered; composition with the codec remains open.
- **18 — Numeric backends:** extend the implementation contracts from u64 to the
  supported BigUint quantities and BigInt signed measures. State and discharge the
  algebraic requirements for custom `QuantityScalar`/`QuantityMeasure` backends,
  with explicit dependency and resource assumptions. Rust trait bounds alone do
  not establish these laws; BigInt itself is not currently a quantity backend.

The iterator-consumer batch adds 19 registered contracts for generic trace
extension, finite-input view construction and stable candidate filtering. The
protocol records primitive `into_iter`/`next` results up to the first `None`,
with concrete vector and non-fused scripted witnesses. Separate contracts propagate
iterator panics after a finite prefix and failures during conversion to an iterator.
Views derive sorted uniqueness and an output-size bound; filtering composes the
existing complete transaction analysis. Trace extension preserves the original
prefix and every yielded proposal, including duplicates. See the
[iterator contracts](ACCOUNTS.md#generic-iterator-consumers) for dependency and
failure scope. These contracts do not prove arbitrary callbacks terminate or
establish state after Rust unwinding. This batch passed the full proof gate
(350 registered contracts; 3,292 audited theorems), all 307 workspace tests,
86 boundary/gate controls, the new production mutation control, formatting,
Clippy, documentation and compile-fail doctests.

The basket-collection batch adds 38 contracts for generic entry/typed-amount
construction, FromIterator and owned-array conversions, and basic basket queries
and borrowed iteration. It proves first-duplicate rejection from a finite prefix
without requiring the unused tail to terminate, propagation of input-iterator
failures, canonical nonzero contents in insertion order, and exact duplicate
panic conditions for convenience conversions. The u64 constructor refines the
existing natural-number specification; generic numeric instances require an
independent successful Zero predicate law. Array iteration adds a reviewed std
alive-range model. See [the collection boundary](BASKETS.md#public-collection-boundary).
Sorted borrowed iteration is now covered by the subsequent batch below.

This basket batch passed the full proof gate (388 registered contracts;
3,378 audited Axionomy theorems), fresh extraction, all 311 workspace tests,
90 boundary/gate controls and both new production mutation controls. Formatting,
Clippy with warnings denied, documentation and compile-fail doctests also passed.
The prior 44 production mutation controls remain in CI.

The sorted-view batch resolves the nested-borrow translation gap by scoping the
temporary mutable slice inside an extracted generic owned-vector helper. The
public API still borrows its entries, orders only by asset, and imposes no new
scalar or cloning bounds. Thirteen contracts prove exact stable sorted contents,
length, strict order and nonzero/unique representation for canonical baskets,
and canonical output independent of input insertion order. The constructor-to-view
contract derives the representation and size obligations from primitive input
laws. The std stable-sort implementation remains modeled explicitly; its physical
algorithm, comparator call counts and unwind behavior are not proved. See
[sorted borrowed iteration](BASKETS.md#sorted-borrowed-iteration) for the exact
contract, reviewed dependency extension and 326 generated conformance cases.
This batch passed the full proof gate (401 registered contracts; 3,412 audited
Axionomy theorems), all 314 workspace tests, 93 boundary/gate controls and the new
production mutation control. Formatting, Clippy with warnings denied,
documentation and compile-fail doctests passed; the prior 46 production mutation
controls remain in CI.

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

## Remaining contracts and dependency boundaries

The unfinished core work remains items 16–18 of the completion goal:

- Remaining public API, finite-map/iterator adapter and arbitrary
  callback contracts; arbitrary erased hash overrides and fingerprint portability
  beyond the documented targets. The named inherited helpers in the batch above
  have their own contracts. Fingerprints are not collision-free identities.
- Serialization/deserialization composition, representation validation and schema
  contracts, including arbitrary deserialized metadata and keys. Transaction
  caller preconditions are discharged for the documented well-formed domain;
  input boundaries must establish that domain separately.
- BigUint arithmetic, BigInt measures and laws for custom `QuantityScalar` and
  `QuantityMeasure` implementations. Rust trait bounds specify operations, not
  their algebraic correctness.

The correctness of compiler/translation infrastructure, physical `IndexMap`,
`BTreeMap`, `Arc`, allocation, cloning and copy-on-write implementations remains
an explicit dependency boundary. Their logical adapters must match the concrete
operations used by each contract; proving the dependencies themselves is outside
this goal. Rust compilation continues to enforce ownership and strong typing.

Extend the pinned Aeneas pipeline for the remaining contracts, connecting fresh
extracted definitions to typed specifications. Model unavoidable external
dependencies explicitly and report their trust status. Do not maintain a second
Rust algorithm only for proofs or change the public API to an untyped
representation to satisfy extraction.

The specification also deliberately avoids universal batch/sequential equivalence:
consume-one/produce-one can fire twice from one token, while a two-unit batch
requires two tokens upfront. Computed effects are evaluated once against the
pre-exchange snapshot and are not automatically multiplied by exchange units.

References: [Aeneas workflow and limitations](https://github.com/AeneasVerif/aeneas),
[Lean/Aeneas introduction](https://lean-lang.org/use-cases/aeneas/).
