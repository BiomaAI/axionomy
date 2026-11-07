# Implementation proof: account updates

The production `Account::deposit` and `Account::withdraw` paths now have Aeneas
refinement proofs for u64 quantities. `src/account.rs` is extracted without
rewriting its algorithms. The proof includes basket lookup, zero-removing
insertion, shortfall calculation, and the checked quantity operations used by
those paths.

## Checked contract

`Axionomy/Account.lean` defines an ordered, typed account specification. Deposits
perform checked addition and reject the first overflowing asset in request order.
Withdrawals return every positive shortfall in request order, or subtract the
requested quantities. Successful updates keep unaffected entries and remove
zero balances. Both failure results preserve the original ordered state.

`AccountRefinement.deposit_refines_naturals` and
`AccountRefinement.withdraw_refines_naturals` prove that the generated production
program terminates with exactly the specification's result and resulting state.
Their execution `Result.ok` witness excludes panic and divergence in the model;
a rejected deposit or withdrawal is a Rust error, not an execution failure.

The withdrawal proof requires unique requested asset identifiers. That is the
invariant of the public `Basket` representation; the preceding constructor proof
establishes it. The proof shows that updates of an earlier asset leave the later
requested assets unchanged. Thus a successful shortfall preflight guarantees each
subtraction's `expect` is safe. The separate shortfall proof also discharges its
`expect`: a positive deficiency always admits checked subtraction.

The proof quantifies over every u64 quantity and arbitrary finite baskets,
including empty baskets, missing assets, full depletion, and late overflow. It
keeps asset identities as an arbitrary type. Equality and cloning have explicit
laws: equality terminates and agrees with identity, and cloning terminates and
returns an equal asset. Concrete dictionaries satisfy these premises without
assuming the desired outcome.

`AccountMap.lean` proves that value conversion commutes with the specification.
`natural_hom` establishes that the u64 arithmetic instance maps to checked
natural-number addition bounded by `2^64 - 1` and exact natural subtraction.
Consequently the main theorem compares final quantities and errors to ordinary
natural numbers, not just another machine-integer calculation.

## Production connection and type guarantees

`extract_account.rs` includes the exact production account, basket, amount, and
quantity source files through `#[path]`, then forwards to the public methods.
The checker verifies these paths and forwarding bodies. Charon uses promoted
MIR before drop elaboration, as in the basket constructor pipeline. Generated
Lean and intermediate LLBC are not patched.

Two source refactors let the pinned translator compile the complete paths:

- `QuantityMeasure` names exactly the existing `SignedMeasure` requirements:
  `Clone + Eq + Debug + Display + Zero + CheckedAdd`. A blanket implementation
  supplies it to every type already satisfying those bounds. `QuantityScalar`
  retains every operation and bound, including those of its associated measure.
  Grouping the measure bounds prevents the generator from assigning the same
  field names to scalar and measure trait dictionaries.
- The local `quantity` parameter of `Basket::insert` is named `amount`, avoiding
  shadowing of the generated `quantity` module. Its type and behavior are identical.

Account methods, asset types, numeric backends, private state, ownership, and
borrowing boundaries retain their original Rust guarantees. The theorem covers
the default u64 extraction configuration; it makes no algebraic claim for an
arbitrary user-defined numeric backend.

## Explicit trust boundary

The compiler/MIR, Charon, Aeneas, and backend primitive models remain trusted,
as described in [AENEAS.md](AENEAS.md). This is not machine-code verification.
The pinned Lean backend includes a small, reviewed instance-naming patch. Its
discriminant attribute otherwise gives two independently extracted `QuantityError`
types the same global instance name, preventing their combined import. The patch
names each instance under its fully qualified enum type. It changes no enum
values, translated program, or proof semantics. Setup checks the patch checksum
and both source versions; extraction requires the patched source hash. The Lean
kernel checks the resulting declarations along with all other proof terms.

The account proof extends the [basket collection abstraction](BASKETS.md):

- `IndexMap` is an ordered list of typed pairs. Lookup, insertion, removal, and
  iteration have explicit executable models. Insertion reuses the basket proof's
  map model. Removal preserves the remaining insertion order.
- The cloning model invokes the supplied key/value `Clone` operations and
  propagates failures. The proof uses explicit clone laws and the extracted
  quantity clone implementation. Physical allocations, hashing, resizing,
  hasher cloning, destructor effects, and copy isolation are not extracted or
  verified. Applying the theorem to Rust assumes those collection operations
  implement their logical contracts with lawful, stable hashing and successful
  allocation.
- Option cloning/default combinators have explicit models. Actual account
  control flow, shortfall computation, checked arithmetic, `expect` calls, and
  assignment of the committed account are extracted, not external models.
- Unused hash internals and formatting return execution failure in the model.
  The shared pipeline now models exact u64-to-i128 widening for the separate
  conservation proof; account updates do not execute that conversion.

All generated and handwritten Axionomy declarations undergo the transitive
axiom audit. Models are definitions; no new axioms, unfinished proofs, or premises
asserting update correctness are admitted. Other basket operations, serialization,
BigUint/custom backends, Economy account selection, exchanges, and replay retain
separate coverage boundaries, except for the per-account staging contract below.

## Exchange account staging

`Account::prepare_exchange` now owns the production clone-withdraw-deposit
sequence previously in `Economy::analyze`. It is crate-private, borrows the source
account immutably, and returns either a new typed account or the original typed
account error. The public API and all numeric/asset bounds are unchanged.
The economy calls this helper in the same account order and maps errors as before.
There is no separate Rust algorithm maintained for proofs.

`AccountRefinement.prepare_refines_naturals` proves termination and exact returned
balances/errors for the extracted helper, including the derived account clone
and both `?` branches. It composes the previously proved withdrawal and deposit
paths. Its premises are the same lawful equality/cloning and unique consumed
asset identifiers; it does not assume sufficient funds or a successful deposit.
`Account.map_prepare` connects the u64 instance to natural-number arithmetic.

The executable specification `Account.prepare` checks consumption first. Its
theorems establish that:

- Missing consumed resources return the complete withdrawal shortfall, regardless
  of what would be produced. Future production cannot fund current consumption.
- Success holds exactly when the consumption preflight succeeds and checked
  credit of the debited account succeeds, returning that exact state and order.
- Overflow during production returns the first overflowing asset in production
  order. No intermediate account is returned. For example, consuming an asset
  and then overflowing a different asset cannot publish the withdrawal.
- `preparedOutcome` retains the original state on rejection. This is the
  specification's publication rule used by conformance cases; it is not itself
  a proof of the economy's commit loop. Rust enforces the helper's immutable
  source borrow, under the existing collection/clone assumptions above.

The extraction uses the same account harness, models, and external template
checksums. It adds no trusted model. A syntactic guard checks the exact production
handoff in `Economy::prepare_effects`. This helper theorem is composed with the
complete caller extraction and transaction proofs described below, including
aggregation, preservation preflight, invariants, preparation and commit.

## Reproduce and maintain

After the tool setup described in [README.md](README.md):

```sh
python3 scripts/extract_account.py --write
# Review the generated diff, proofs, external interfaces, and source coverage.
python3 scripts/check_proofs.py
python3 -m unittest discover -s scripts -p 'test_*.py'
```

The gate regenerates account code alongside arithmetic and basket construction,
rejects stale generated files and changed external interfaces, checks the Lean
kernel and axiom dependencies, and runs Rust conformance and type checks.

The executable natural-number specification generates 328 account cases covering
both asset orders, u64 boundaries, exact shortfalls, untouched assets, and state
preservation after a later failure. They run through the public Rust methods.
Isolated production-source mutations commit partial deposits and reverse the
withdrawal preflight condition. Each must reject stale extraction, successfully
compile its freshly generated model, and then fail its refinement proof.

Another 492 Lean-generated cases run via the public `Economy::assess` and `apply`
APIs. They vary consumption/production order and u64 boundaries, with an earlier
account that stages successfully and a separate unaffected account. They check
exact results, errors, receipts, assessment immutability, failure atomicity, and
fork isolation. Full depletion followed by reproduction must succeed even at
`u64::MAX`; producing a missing consumed asset must still fail. These are finite
integration checks, not universal whole-economy proofs. An isolated mutation
reverses deposit and withdrawal; regeneration must compile and its refinement
proof must fail.

## Shared pipeline: rates and conservation

The harness also includes the exact production `rate.rs` and `exchange.rs`
modules. Its original rate entrypoints forward to the three fallible static rate
builders, u64 weighting, and `LinearInvariant::conserves`. Local `basket` and
`quantity` bindings in private rate helpers were renamed to `assets` and `amount`
to avoid shadowing generated namespaces; no Rust bounds or algorithms changed.

`RateConstructionRefinement` composes the existing basket addition proof with
entry borrowing and its backward functions. Its exact outcomes include ordered
asset overflow and unchanged unrelated rate fields. Even an empty basket registers
a role. The private merge model inserts the default empty basket before merging,
matching Rust's entry API even on error; the public consuming builder returns no
partially built rate on failure.

BTree maps/sets use sorted lists and invoke the supplied role comparison.
Occupied entries retain their stored key. Vacant entries remember insertion
position, and backward functions restore the preceding/following entries.
`locate_restore` and `locate_key` prove the position/frame contracts; the builder
proofs require total identity-preserving cloning and comparison agreeing with a
pure comparator. The registered-role theorem additionally requires comparison
equality to imply key identity. Applying these models to Rust assumes valid
sorted storage and lawful total ordering, as well as allocation/tree correctness.
The translation does not verify the standard library's tree internals.

`ConservationRefinement` proves exact widening and multiplication for all u64
quantities and i64 coefficients, exact checked i128 addition, and total execution
of both conservation loops. It relates the result to a mathematical integer
specification with intermediate bounds, not just a final unbounded sum.
`accepted_conserves` states equality of weighted mathematical totals on success.
The Option try adapters propagate failure explicitly. None of these adapters or
proofs adds an axiom. The transaction contract below now covers economy
diagnostics; BigUint/BigInt and custom backend laws remain separate obligations.

Fresh extraction, external-interface checksums, the Lean kernel/axiom audit,
464 conservation fixtures and 56 merge fixtures (each run through three builders)
cover the new paths. Negative controls reverse the conservation result and discard
a rate overflow: the regenerated Rust programs compile, but their implementation
proofs must fail.


## Snapshot evaluation, effects and whole-state measurement

The shared pipeline now also includes the exact production `economy.rs` source.
The forwarding entry points select the recursive expression interpreter, a
condition helper, the static/computed effect merge helpers, typed-error rendering,
and `LinearInvariant::measure`. Generated Rust types retain separate account,
asset, role and rate identifiers; no public API or generic bound was removed.

`EvaluationRefinement.evaluate_refines_naturals` proves termination and exact
checked natural-number results for every finite expression and u64 input. Both
children read the same immutable context. A failing left child wins over a
failing right child; operation-specific errors occur only after both succeed.
Parameters use sorted string lookup. Balance lookup distinguishes an unbound
role, a bound missing account and an absent asset (zero). Addition, subtraction,
multiplication, floor division, minimum and maximum are covered. The condition
refinement proves the two evaluations and all three comparisons, including
ordered propagation of evaluation failures.

The existing condition code was factored into a crate-private helper called by
`Economy::analyze`. Internal failures now use `ExpressionError`; public errors
still contain exactly the existing message text. This avoids depending on erased
formatting arguments in the translation backend. `DiagnosticRefinement` proves
UTF-8 encode/decode round trips and exact production messages, including arbitrary
valid Unicode parameter names, when the complete message byte length fits `usize`.
Literal decoding is checked by Lean's kernel, not an extra native-evaluation axiom.

`EffectsRefinement` proves the exact natural-number result and state for
`merge_scaled` and `merge_quantity`. Static merging first checks every scaled
source amount, then atomically adds the resulting basket. Either stage returns
its first overflowing asset and the original target. Computed quantities receive
one checked addition with zero canonicalization, without an implicit units
multiplier. The transaction contract below composes these helpers with the
complete account aggregation loop and derives its caller preconditions.

`ConservationRefinement.measure_refines_integers` proves the complete ordered
fold over account values and each account's asset entries. It covers intermediate
signed overflow, negative weights, empty accounts and absent weights. The Rust
method now reuses `checked_add_basket_measure`, preserving arithmetic and iterator
order while avoiding an outer-loop `continue` unsupported by the translator.
This helper theorem does not prove invariant construction or the economy's before/after
error diagnostics.

The extension adds the following explicit dependency models:

- `Arc<T>` denotes its logical immutable pointee. `Deref`, `AsRef`, and logical creation through `Arc::new` are
  modeled; the model-construction extension below uses creation. Reference counts, clone/COW mutation, isolation and allocation
  are not established by this abstraction; Rust still checks borrowing.
- `IndexMap::values` traverses the existing logical list in insertion order.
  `BTreeMap::get` borrows each stored key and performs ordered early-stop lookup.
  Valid sorted maps, lawful ordering/equality, hash coherence and physical storage
  remain assumptions of applying the logical collection model.
- Strings are Lean's valid UTF-8 strings, with explicit byte-slice conversion and
  checked append length. Invalid logical UTF-8 and unrepresentable lengths fail
  closed. Correct physical UTF-8 storage, Rust/Lean string-order agreement,
  allocation and copying remain trusted. Formatting and hashing internals are
  still unsupported, not silently interpreted as correct output.
- Resource exhaustion (allocation/stack), translation correctness, primitive
  semantics and machine code retain the existing trust boundaries.

The proofs have no premise asserting that evaluation or measurement succeeded.
Lawful identity/equality/ordering dictionaries have existing concrete witnesses.
Theorems over these helpers do not certify iteration over
conditions, aggregate account effects, preservation preflight, diagnostics,
preparation completeness, receipts, multi-account commit or other numeric backends.

The evaluator/effect/measurement controls deliberately reverse operand error
order, replace `<=` with `<`, reset the running measure, alter the parameter-error
prefix and discard scaled amounts. Each control first requires fresh extraction
and successful compilation of the generated implementation, then requires the
reviewed semantic proof failure. A stale model or an unresolved external symbol
cannot satisfy these checks.


## Model construction and complete rate metadata

`ModelConstructionRefinement` extracts `EconomyBuilder::new/default`, account/rate
registration, invariant registration and `build`. Its typed specification records
first-entry retention, insertion order, the ordered sequence of duplicate errors
and unchanged unrelated data. `build` accepts exactly an issue-free builder and
returns all registered data. Its distinct-key premise is established by `new`
and preserved by all registration operations; it is not an assumed build-success
premise. Appending issues/invariants requires room for one more vector element.

`RateMetadataRefinement` covers `Rate::new/default`, the three computed builders,
condition construction with a `String` label, referenced-role registration, and
canonical distinct pairs. Complete record-update equalities prove frame behavior.
The selected role `PartialOrd::le` must agree with its comparator; u64 has a proved
witness. Generic `Into<String>` effects are covered separately by
`CallbackRefinement`, as described in the convenience-constructor contract below.

`DiscoveryRefinement` covers all ten expression constructors.
`RateDiscoveryRefinement` composes the recursive proofs through every computed
amount and condition loop. Parameter discovery returns exactly the logical sorted
union of names, including references on both condition sides. Asset discovery
retains duplicates and traverses consume, produce, preserve, then conditions;
each computed target precedes its expression's references. The capacity premise
bounds the total number of output references, and each parameter must have a
UTF-8 byte length representable by `usize`. Static basket assets are intentionally
outside `computed_asset_keys`, matching the public contract.

The production refactors replace `map/collect`, `extend/cloned` and chained
iterators with explicit loops and two private slice helpers. The pinned translator
cannot handle those iterator trait/lifetime combinations. The replacements retain
public signatures, ordering, borrowing, ownership and generic bounds. Their actual
production paths are extracted; no alternate Rust algorithm is maintained.

New dependency models cover empty trees/sets, owning ordered-map/set iteration,
borrowed vector iteration, `contains_key`, logical `Arc::new`, reference ordering,
UTF-8 string comparison and lexicographic pair comparison/equality. These are
explicit trusted standard-library contracts, not proofs of physical storage,
allocation, reference counting, COW or destructors. Unused tuple `lt/le/ge` fail
closed: the pinned Rust tuple implementations use chaining hooks absent from the
backend dictionary. No proof calls those placeholders. Existing lawful key/hash/
ordering, successful allocation, compiler/MIR/translation and backend assumptions
remain, and no axioms were added.

Lean generates 345 model sequences and 77 rate sequences with expected snapshots
at every prefix. Rust checks both `new` and `default` with separate holder, asset,
role and rate types. Six controls mutate duplicate detection, build rejection,
computed-role discovery, distinct-pair orientation, preserve parameter inclusion,
and condition asset order. Each must compile after fresh extraction and then fail
its semantic proof. This closes the model/discovery backlog item. Structural
validation and complete transaction orchestration are covered below;
remaining convenience APIs are tracked separately.

## Complete structural validation

`ValidationCompleteRefinement.validate_refines` proves successful termination of
the extracted production `Economy::validate_structure` and exact agreement with
the typed `Validation.validate` specification. It covers every branch and all
seven loops, with no assumed success of the helper or any loop. The result is
the same stored borrowed rate, or the complete diagnostic list in its existing
order: zero units; missing rate (early return); missing bindings; unknown bindings;
missing parameters; unknown parameters; distinct-role violations; sorted,
deduplicated missing bound accounts. All role/parameter issues are accumulated
before the final accept/reject decision.

Corollaries prove that accepted units are nonzero, every declared role has a
binding to an existing account, every required parameter is supplied, distinct
pairs bind different accounts, and every rejection has at least one issue.
The account-existence result requires comparator equality to imply identity,
so deduplication cannot hide a missing account. The transaction proofs below use
these facts to discharge downstream lookups. Computed-target registration is
established by the actual builders; arbitrary deserialized metadata still needs
its separate representation contract.

`analyze` now delegates its existing structural checks to this crate-private
borrowed-result helper. The bridge guard checks that exact handoff. Explicit
loops replace unsupported `Iterator::any` and chained missing-account iterators.
Required-name traversal clones the set of borrowed names, because the pinned
translator cannot handle nested shared borrows or `iter().copied()` here. This
adds a temporary set allocation, retains borrowed names and all public types,
and preserves diagnostic contents/order. Physical allocation cost and failure
remain outside the proof.

New external contracts model tree key iteration, borrowed set iteration, logical
set cloning with per-element clone effects, borrowed string views and tree/set
membership using the existing ordered lookup. They preserve the previously
documented sorted-collection, lawful key/hash/order, clone, UTF-8, allocation,
compiler and translation trust boundaries. Parameter UTF-8 byte lengths must fit
`usize`; a conservative sum bounds the maximum diagnostic-vector length. These
are input/resource conditions, not premises that validation succeeds. No axioms
or assumed loop results were added.

Lean generates 765 cases. Rust exercises the actual private helper with distinct
holder/asset/role/rate types, checks selected-rate pointer identity, and compares
all ordered diagnostics. Each case also runs through `assess` and `apply`, both
with and without a failing post-validation condition, checking first-error
behavior and unchanged state. These are caller regression tests. Five mutation
controls reverse units, binding, parameter, distinctness and account checks;
the generated mutant must compile before its semantic proof is rejected.

## Complete transaction assessment, application and filtering

`Transaction.lean` specifies the actual ordered pipeline using natural-number
quantities bounded by u64 and signed integer measures bounded by i128. Handled
transaction errors are distinct from modeled panic. `TransactionDecode` maps
the extracted typed Rust values to this specification without collapsing account,
asset, role or rate domains. The generated implementation remains the production
Rust call path, not a second Rust algorithm written for verification.

`AnalysisRefinement.analyze_refines` and `assess_refines` compose structural
validation, all conditions, static effects, computed consume/produce/preserve,
availability, account preparation and invariant checks. They prove termination
and exact ordered results, including the first applicable error, every positive
shortfall, relevant balances, projected deltas and diagnostic totals. Conditions
and computed expressions use the original snapshot. Static consumption/production
scale by units; preservation and computed quantities do not gain an implicit
units multiplier. Aliased roles accumulate into one account effect.

The composition derives rather than assumes safe downstream lookups:

- `RateWellFormed` proves that actual constructors/builders register every
  computed target and preserve that invariant through subsequent builder calls.
- Structural acceptance establishes the bindings and existing accounts.
- `AggregationContract` derives canonical baskets, sorted unique affected
  accounts and a size bound from the model. These facts discharge preflight and
  preparation premises, including checked vector growth.
- `Transaction.analyze_ready` in `TransactionSafety.lean` establishes nonempty rejection or uniquely staged
  accounts corresponding to exactly the aggregated effects and receipt deltas.

`ApplyRefinement.apply_refines` connects those facts to the full public `apply`.
It proves both internal `expect` preconditions and the existence of every mutable
account index. Rejection returns the exact error and the entire unchanged raw
model. Acceptance returns the submitted proposal and specified deltas, installs
exactly the prepared balances, and preserves account keys/order, rates, invariants
and account lookups outside the receipt deltas. Successful staging is not an input
assumption of the full theorem.

`ApplyRefinement.accepted_receipt_refines` states the receipt consequence directly:
receipt account IDs are unique, and each delta names an original and committed
account whose observable final balance equals successful checked consumption
followed by production from that original balance. The proof composes actual
`apply` refinement, preparation correspondence and exact staged-account lookup.

`FilteringRefinement` proves public boolean applicability and the production
filter for finite Vec candidates. It preserves order and duplicates, evaluates
each candidate against the same original model, and establishes successful
boolean evaluation for every candidate; the default on a missing specification
result is therefore unreachable. `IteratorFilteringRefinement` extends this to
finite streams under independent callback laws, as described below.

Premises include total identity-preserving cloning, identity-consistent equality,
comparison agreeing with the logical comparator, and lawful account ordering
(reflexivity, equality consistency, strict-order transitivity and reversal).
Computed targets must be registered. Explicit `usize` bounds cover account count,
validation issue growth and relevant UTF-8 names/rendered diagnostics. These are
representation, identifier and resource requirements, not assumptions that an
exchange succeeds. Natural-number ordering and the existing lawful dictionary
witnesses demonstrate satisfiability. Builder proofs derive target registration.

Production refactors separate the existing stages without changing their order
or public types. Explicit loops replace iterator combinators for relevant-balance
construction and candidate filtering. Existing-key mutable indexing replaces
`get_mut(...).expect(...)`; its absence panic is discharged by the caller proof.
The private receipt field is named `proposal` to avoid translation name shadowing;
the public getter, serialized/schema field `exchange`, and debug field name are
preserved. Rust still enforces ownership, private state and generic domain bounds.

The additional external models cover ordered borrowed/consuming BTreeMap
iteration, bounded map length, `Result::is_err`, logical Arc clone and mutable
IndexMap indexing with a backward update function. Absent mutable keys panic;
successful replacement retains the stored key and all other entries. Arc clone
shares its immutable logical pointee. Physical collection storage, hashing,
allocation, reference counting, destructor effects and clone/copy-on-write
isolation remain trusted, alongside compiler/MIR, Charon/Aeneas and primitive
models. The unused `MaybeUninit` type alias models no uninitialized-memory
operation. The kernel audit accepts no new axioms.

The mandatory proof gate registers these composed contracts, checks fresh
extraction and all axiom dependencies, and compares 596 Lean-generated complete
transaction scenarios with Rust. Cases cover aliasing, original-snapshot
evaluation, scaling, preservation, ordered shortfalls, late-account overflow,
invariant overflow/violation diagnostics, receipt agreement and atomic rejection.
Mutation controls require regenerated changed Rust to compile and then fail the
corresponding semantic proof. Remaining public data-boundary and numeric-backend contracts remain listed in the main backlog.


## Execution: fork, simulation and replay

`ExecutionRefinement` composes the actual extracted `Economy::apply` with the
production fork, simulation and replay wrappers. `Economy::replay` now uses an
explicit loop with the same first-error behavior as the former iterator/collect
chain. Its public generic signature is unchanged. Local proposal names avoid
shadowing generated Lean namespaces. The extraction harness forwards directly
to these methods and to Trace new/default/push/exchanges/into_exchanges.

`Execution.run` specifies an ordered trace, accumulated receipts and the state
on either outcome. A failed action ends the trace immediately. The specification
and `TransactionExecution.replay_error_prefix` derive that atomic transaction
rejection leaves exactly the state reached by the successful prefix. Success
returns receipts in proposal order. `success_history` records every intermediate
state; `accepted_changes` applies the existing per-transaction receipt theorem
at every step, including repeated visits to the same account. `apply_conserves`
derives acceptance by every registered runtime invariant from the actual
analysis control flow. `replay_conserves` composes those facts and proves exact
mathematical equality of weighted consumed and produced totals for each receipt
throughout the successful trace. The first-error prefix theorem supplies the same
result for the accepted portion of a failed trace.

The implementation theorem proves termination and exact results for all traces
whose proposals satisfy the transaction name/metadata bounds and whose starting
account count fits usize. Trace Vec bounds supply receipt capacity. The proof
does not assume intermediate apply success, replay success, or per-step caller
readiness. The actual apply frame preserves rates, invariants and account-key
order; induction uses those facts to derive all next-step obligations. General
invariant composition still requires the relevant per-step property, rather
than asserting that arbitrary user predicates are preserved.

Derived Economy cloning preserves the logical account map and shared rate and
invariant values. Derived Exchange cloning preserves its typed rate, ordered
bindings, units and parameters. The only newly introduced external interface is
BTreeMap clone, modeled by calling each supplied key/value clone and retaining
entry order. The generated external template diff was reviewed: no other
interface changed. Lawful identifier clones, ordered finite-map models, logical
Arc values, allocation/resources, physical collection storage, reference counts,
clone/COW isolation, the compiler and translation remain explicit trust
boundaries. Rust enforces source borrowing and ownership; Lean is not proving
physical reference-count implementation or machine code.

246 Lean-generated replay fixtures cover empty traces, repeated actions,
changing balances, aliases, zero-unit/missing-rate stops, overflow, conservation,
changing parameters and original-snapshot computed effects. Rust compares exact
ordered receipts, errors, final prefix state and source isolation. All 596
transaction fixtures also check simulate. Production mutation controls require
proof failure when replay ignores errors, discards receipts, or simulation
applies twice. Generic trace extension is covered by the iterator-consumer
contract below; serialization remains a separate public-boundary obligation.

## Goals, views and canonical keys

`GoalRefinement` proves new/default/require/requirements, borrowed asset discovery,
and complete goal matching. Matching is an account-existence and natural-number
sufficiency predicate, including empty required baskets. `Observations.goal_monotone`
requires both preserved account existence and nondecreasing balances. Sorted map
insertion retains the stored key and replaces the required basket. Logical map
storage and allocation remain trusted; supplied identifier operations must obey
their independent equality, clone and order laws.

`ViewConstructionRefinement` proves finite-vector view construction as sorted,
deduplicated registration. `ObservationRefinement` proves the actual model/view
lookups, including zero balances and hidden/missing accounts. The sorted visibility
precondition follows from construction. Rust's generic `IntoIterator` API is
preserved. The iterator-consumer contract below extends construction to finite
streams under primitive callback laws; arbitrary callback implementations still
need their own evidence.
Explicit loops and the borrowed goal-asset helper avoid Aeneas nested-borrow
limitations while preserving borrowed return values and every identifier type.

`KeyRefinement` proves both nested key builders, their capacity obligations,
sorting calls, and public accessors. `CanonicalSort` connects the dependency sort
adapter to a stable insertion-sort specification and proves permutation, length,
sortedness and canonical order under independent order laws. `KeyOrdering` derives
lexicographic triple laws from account/asset comparisons and exact u64 ordering.
Physical std slice sorting remains trusted; the generic adapter's behavior for
unlawful comparators is not a claim about Rust's implementation.

Produced observation keys contain requested visibility, existing visible accounts
(including empty accounts), and visible balance triples. `view_key_sound` composes
the key and lookup refinements: equal generated keys imply equal actual public
balance-query calls, including Some(0) versus None, across different account domains.
It requires valid sorted visibility sets, unique asset entries, lawful operations
and output capacity bounds. Key generation and the query proof do not assume that
missing accounts exist. Noninterference excludes hidden accounts. Reordering
balances preserves keys. State keys omit empty-account existence and model metadata;
use a fixed account domain when account-existence behavior matters.

The new required serialized `existing_accounts` field intentionally rejects old
keys. It cannot be reconstructed from a balance list. This is covered by Rust
regressions and schema checks; a general validation contract for arbitrary
serialized key fields remains item 17. Derived key PartialEq/Hash methods are covered by the derived-public-traits
extension below; the earlier key-generation contract proves equality of produced
typed fields and its relationship to actual visible queries.
Unused tuple PartialOrd dictionary fields remain fail-closed in the external
model; sorting uses Ord::cmp. The derived-public-traits extension below supplies
the tuple equality model used by the actual ObservationKey PartialEq method.

`FingerprintRefinement` proves the concrete new/write/finish implementation and
complete state_fingerprint caller against mathematical FNV-1a. Every byte performs
xor then multiplication modulo 2^64. The stream contains the vector length followed
by each sorted triple's account, asset and u64 quantity encodings. Identifier Hash
implementations have explicit deterministic byte-encoding laws; the numeric law is
proved for the reviewed adapter. This establishes a deterministic fingerprint for
the declared encoding, with no collision-freedom claim.

The hashing boundary is native 64-bit little-endian Rust with the default Hasher
convenience methods and tuple Hash::hash_slice. The pinned Rust sources show Vec
hash delegates to slice hashing, which prefixes its usize length and invokes the
item hash_slice; tuples inherit the default per-element hash_slice. Hasher's default
write_usize/write_u64 use native-endian bytes. Aeneas's trait dictionary retains
only Hasher::write/finish and Hash::hash, erasing those convenience overrides. The
adapter therefore does not claim arbitrary override support. The extraction script
forces its native target and rejects other word widths or byte orders. Current
pinned binaries support macOS arm64 and Linux x86_64/aarch64. Reviewed source paths:
`library/core/src/hash/mod.rs` and `library/alloc/src/vec/mod.rs` in the pinned
nightly-2026-09-17 rust-src component. Physical hashing/storage, allocation, standard
primitives, compiler/MIR/Charon/Aeneas and backend semantics remain dependencies.

504 executable Lean fixtures exercise empty/absent accounts, hidden data, duplicate
visibility requests, account/asset insertion order, overwritten goals, zeros,
u64 maximum identifiers/quantities, query results, both key forms and fingerprints.
The Rust bridge uses separate Holder/Asset/RuleId/Role types. Production mutation
controls target missing-goal acceptance, reversed visibility, omitted empty-account
presence, skipped state sorting and an incorrect FNV prime. These tests supplement
the general proofs. All five controls passed, alongside fresh extraction, the full
proof gate (281 registered contracts and 3,083 audited Axionomy theorems), all 298
workspace tests, 80 boundary/gate controls, formatting, Clippy and documentation
checks. Items 14–15 are validated within the scopes above; items 16–18 remain open
or partial.

## Typed public data and expression construction

`TypedDataRefinement` proves the extracted `AssetAmount` constructor, accessors
and both tuple conversions, including round trips in both directions. Quantity
new/get, u64 conversion, lossless u32 widening, scalar borrow/consume and default
are explicit contracts. `quantity_validation` and `checked_validation` preserve
the independent nonnegative predicate supplied by a numeric backend, including
rejection of invalid checked results. The predicate law is a premise about the
primitive backend method, not an assumption about the constructor's result.
It does not establish the full algebraic requirements still tracked in item 18.

The account constructor/default/borrow/consume/conversion contracts preserve the
entire basket. u64 account equality composes the basket equality proof; it ignores
entry order for unique entries and requires representable map lengths. Typed
amount insertion composes the proved store operation, including the old-value
result, zero removal and the unchanged remaining entries.

Proposal construction preserves the supplied rate identity and units. Binding
and parameter updates replace only their selected map entry and preserve other
proposal fields. The sorted-map dependency model retains the stored key on
overwrite; identifier order must agree with identity. Parameter conversion has an
independent `Into<String>` encoding law. The composed expression constructor uses
the concrete String identity conversion. `CallbackRefinement` supplies the separate
whole-function contracts for arbitrary modeled conversion results, including
failure or divergence, described below.

Direct contracts cover proposal, receipt, delta, account assessment, shortfall,
computed amount, rate condition and model-build-error accessors. Assessment
variants return the exact account/shortfall/issue lists and distinguish absent
projected deltas from an empty successful list. Borrowed condition names use the
reviewed UTF-8 model with a representable byte-length bound. Rust's borrowed
lifetimes and distinct identifier domains remain intact.

`ExpressionConstructionRefinement` covers all public expression constructors and
the Add operator. `construct_refines` recursively composes the actual extracted
constructors; `evaluate_constructed` then composes their result with the existing
evaluator refinement. This proves exact operators, operand order, typed names,
values and evaluation failures for u64 expression trees. Nonconstant constructors
and ordinary value accessors remain generic in the numeric representation.
Allocation, physical references/storage, stack/resource behavior, compilation,
translation and the existing primitive models remain dependencies.

The extraction adds 65 checked forwarding roots without a new external model.
Two Rust parameter names change from `amount` to `value` to avoid an Aeneas module
name collision; types and executable behavior are unchanged. The Lean generator
adds 18 typed-value/update cases, 120 proposal cases and five signed validation
cases. Rust also rebuilds both trees in each of the existing 2,210 evaluation
scenarios through public constructors. The existing transaction conformance bridge
checks the diagnostic and receipt accessors. Three production mutation controls
target discarded bindings, a wrong delta accessor and reversed division operands.
The batch passed the full proof gate with 331 registered contracts (50 new) and
3,244 audited Axionomy theorems, fresh extraction, all 304 workspace tests,
83 boundary/gate controls and all three new production mutation controls.
Formatting, Clippy with warnings denied, documentation and compile-fail doctests
also passed. The prior 40 production mutation controls remain in CI.

This closes a batch within item 16. It does not close the remaining collection,
iterator, lookup, rate/invariant/model inspection, derived-trait or
convenience-constructor contracts, nor serialization/schema or other numeric
backend arithmetic.

## Generic iterator consumers

`IteratorContract.Yields` describes only primitive `next` calls, up to the first
`None`. It neither assumes a consumer result nor requires fused behavior after
that `None`. `yields_unique` proves determinism of the observed sequence, and
`vector_yields` and `scripted_yields` supply concrete, satisfiable witnesses. A
separate `FailsAfter` protocol describes a finite successful prefix followed by
an iterator panic. Each public consumer also has an explicit `into_iter` failure
contract. Nonterminating iterators are outside these termination claims.

`TraceExtensionRefinement` connects the actual extracted `Trace::extend` to list
concatenation for generic typed proposals and numeric payloads. It preserves the
entire original trace, proposal order and duplicates. The capacity premise covers
the old length plus the yielded length. The implementation now uses an explicit
`for`/`push` loop instead of delegating to `Vec::extend`; this avoids adding a
trusted model for standard-library specializations or erased iterator overrides.
The generic public signature is unchanged. Logical results are preserved for
lawful iterators; allocation strategy and calls to custom `size_hint`/`fold`
overrides can differ from the previous delegation. No performance equivalence is
claimed.

`IteratorViewRefinement` proves the generic production view loop and public view
constructor. The model stays unchanged and visibility equals ordered registration
of exactly the yielded identifiers. Independent order laws establish sorted,
unique IDs; an input-length bound establishes the output bound. This operation is
generic in the numeric dictionary and does not establish its arithmetic laws.

`IteratorFilteringRefinement` extends candidate filtering beyond vectors. Its
public theorem composes complete u64 transaction analysis for every yielded
proposal, deriving the predicate result from the existing analysis proof. It
preserves order and duplicates and uses the same original snapshot throughout.
Capacity, metadata readiness and lawful identifier operations remain explicit.
The successful and failure-prefix loop theorems support the public contracts;
consumer correctness is not a premise about itself.

All three consumers propagate iterator failures, subject to successful lawful
operations on the preceding values. These are Aeneas failure results, not proofs
of recoverable state after physical Rust unwinding. Destructor effects, foreign
side effects, allocator behavior, primitive collection models, compilation and
translation retain their documented dependency boundaries. No new external model
or axiom is introduced by this batch.

Lean generates 24 scripted trace/view cases, including empty streams, duplicates,
nonempty starting traces and `Some` values after an initial or intermediate
`None`. Rust checks these cases with a separately implemented iterator whose
`fold` and `size_hint` overrides panic and which rejects polling after `None`.
All 596 transaction scenarios also exercise the generic candidate source,
including repeated proposals and an unused tail. Additional Rust checks exercise
panic propagation during `into_iter` and after prefixes of lengths zero to three.
A production mutation that drops trace entries must compile through extraction
and then fail the implementation proof. Remaining item 16 work includes basket
collection conversions/iteration and inspection/lookup APIs (covered by subsequent
batches), other callback laws, derived traits and convenience-constructor
panic contracts.

This batch passed fresh extraction and the full proof gate: 350 registered
contracts, 3,292 audited Axionomy theorems, kernel checking, fixture equality and
compile-fail doctests. All 307 workspace tests, 86 boundary/gate controls and the
new production mutation control passed, as did formatting, Clippy and documentation
with warnings denied. The mutation keeps the extracted loop interface intact:
appending only when the trace is empty compiles through extraction and then fails
the refinement proof. The prior 43 production mutation controls remain in CI.

## Core inspection and complete asset collection

`Inspection*Refinement` connects 20 public inspection APIs and seven concrete
iterator-consumption probes to the production Rust through Charon/Aeneas. The
proofs remain generic in numeric payloads. Inspection transports these values;
it does not establish their arithmetic laws or close the BigUint/BigInt backlog.

`ExchangeAssessment::account` and `shortfall` return the first matching record
or missing basket, including assessments assembled by callers with repeated
account IDs. Whole-function monadic refinements cover all three variants. Pure
successful comparison laws yield the `Inspection.first` specification; separate
composed contracts propagate comparison failure after a nonmatching prefix and
return a first match without requiring any later comparison to terminate. Lookup
retains its original `PartialEq`-only bound and borrowed result. The explicit
production loops replace slice `find` only to avoid a nested-lifetime extraction
limitation. They do not collect or clone the input.

`LinearInvariant::new` propagates arbitrary `Into<String>` success or failure.
`weight` inserts or replaces a nonzero signed coefficient and removes zero
coefficients while preserving the order of the other keys. The integer projection
refines `Inspection.weight`, preserves the invariant name, and preserves unique
nonzero weights from a valid input. The empty constructor establishes that input
representation. The name accessor borrows the original UTF-8 contents; its logical
string conversion requires a representable byte length. Asset equality and the
IndexMap storage/hash contract remain explicit dependencies.

Rate inspection returns the stored registered roles, distinct pairs,
static baskets, computed amounts and conditions. Static lookup requires independent
successful role comparisons. Model inspection returns accounts, rate IDs and
invariants in insertion order, and selects a stored rate under lawful equality.
The named private `AccountEntries` and `WeightEntries` adapters expose their
primitive `next` and exact `size_hint` behavior to extraction. Factory contracts
compose with finite-yield proofs and the public next/hint probes. The iterator
proofs apply to every remaining suffix; concrete size hints equal its length.
Logical IndexMap lengths have explicit `usize` bounds; Vec/Slice bounds follow
from their representations. Public types, lifetimes and trait bounds are retained.

The complete `Economy::asset_keys` implementation now returns exactly the sorted,
duplicate-free union of account balance keys, static baskets reached through
registered rate roles, computed targets, recursive expression/condition balance
references and invariant weight keys. `model_assets_contract` proves exact
membership, strict order, uniqueness and the output-size bound. Independent asset
order laws and successful role comparisons are premises; each rate's computed
reference vector must fit `usize`. No premise assumes the collection call succeeds.
The theorem describes the stored metadata inspected by this API; validating
arbitrarily deserialized role registration belongs to the serialization contract.
Static collection now uses helpers that keep each borrowed field local to avoid
an Aeneas nested-borrow mapping limitation. Numeric values are never copied by
these helpers, and no new cloning or ordering bound is imposed on quantities.

The external interface adds reviewed logical models for `indexmap` 2.14.0 `Keys`
iteration and exact remaining lengths for its iterators and concrete std
Slice/Vec/array iterators. Their physical storage, allocation, hashing, drops and
iterator internals remain trusted. Aeneas' pinned generic `Iterator` dictionary
does not contain `size_hint`. Account extraction uses its supported
`-filter-trait-methods` mode while retaining concrete methods and all seven public
caller probes. An exact diagnostic guard accepts only the reviewed missing-field
warning; different, missing or duplicate warnings and all reported errors fail.
Generated-definition guards require every public method, adapter and collection
loop. Unsupported generic calls cannot typecheck against a missing dictionary
field. External template hashes are updated only for the reviewed additions;
the translator binary, version pins and existing backend patch are unchanged.

Lean generates 537 scenarios: 144 assessment lookups, 153 invariant-update
sequences and 240 model inspection/asset-collection cases. Rust checks signed
extrema, Unicode, all recursive expression forms and comparison variants,
duplicate IDs, empty inputs, metadata-only assets, every iterator suffix and
borrowed pointer identity. A `PartialEq`-only identifier records comparison calls
and panics at a selected entry, checking first-match stopping and failure
propagation. Serde is only setup for private assessment records in these tests;
this is not serialization proof evidence. The model tests retain separate account,
asset, role and rate types and also inspect BigUint payloads beyond `u64`.
These finite checks supplement the general implementation proofs. Three mutation
controls reverse lookup acceptance, discard invariant assets or reverse zero-weight
handling; each regenerated implementation must compile before its semantic proof
is required to fail. Physical unwinding and foreign callback side effects remain
outside these logical contracts.

This inspection batch passed fresh extraction, the full proof gate, kernel checking,
fixture equality and axiom auditing: 461 registered contracts and 3,654 audited
Axionomy theorems. All 319 Rust 1.89 workspace tests, 97 boundary/gate controls and
three new production mutation controls passed, as did formatting, Clippy,
documentation and workspace doctests. The previous 47 production controls remain
in CI. These counts describe verification evidence, not a percentage of core
coverage; remaining typed APIs, serialization/schema and numeric backend work
are tracked in the completion backlog.

## Convenience constructors and string-conversion callbacks

`ConvenienceRefinement` connects the public u64 `Rate::consume`, `produce` and
`preserve` shortcuts to the existing checked-builder specifications. Each returns
exactly the successful builder result, with all other fields preserved, or panics
exactly when bounded natural-number credit overflows. The failure criterion is an
independent specification over the selected role basket and input entries, not a
premise that the extracted builder fails. Role clone/order and asset clone/equality
laws remain explicit, successful primitive-operation assumptions. These are u64
contracts; generic numeric arithmetic still belongs to item 18.

The fixed-array `basket` helper composes the extracted mapping closure with the
proved array-to-basket constructor. Literal asset/value pairs remain unchanged by
the closure. The result has insertion order and zero filtering, and duplicate
asset keys are exactly the panic condition under lawful equality, including zero
entries. Its public `Eq + Hash` asset bound stays intact, without adding `Clone`
or `Ord`. A source-local variable changes from `quantity` to `value` to avoid a
Lean namespace collision; this does not change the Rust behavior or signature.

The only added external interface is `core.array.Array.map`. The reviewed model
calls the supplied `FnMut` from left to right, threads its state, retains length
and propagates the first failure. The pinned Rust source at
`library/core/src/array/mod.rs` implements `map` through `try_map`, a draining
iterator and `try_from_fn_erased`, whose loop invokes the generator at increasing
indices. Physical `MaybeUninit` storage, moves, drops, unwinding and resource
limits remain trusted. The extracted basket closure is proved against this
model; proving helpers about the model does not verify the standard library.
The external template hash and required concrete definitions are guarded, with
no translator patch or new axiom.

`CallbackRefinement` covers generic `Into<String>` inputs to expression parameters,
proposal parameters, invariant names and condition labels. Whole-function monadic
equalities preserve any modeled conversion result, including failure or
divergence; point laws for conversion success/failure yield the corresponding
constructor contracts. They do not assume that the constructor already succeeds.
Expression/proposal numeric payloads stay generic. Proposal updates overwrite
only the converted parameter key and retain rate, units and bindings. Invariant
construction composes its existing name and empty-weight contracts.

Condition construction is extracted for u64 rates. Under independent successful
role comparison/clone laws, it registers exactly the expression roles and appends
the condition with its converted label, preserving all remaining fields. Success
requires vector capacity for one element. A conversion failure precedes the
append and therefore propagates without that capacity assumption. Foreign side
effects, arbitrary role callback failures, destructors and state observable after
physical unwinding are outside this contract.

Lean generates 781 literal arrays and 72 callback scenarios. Rust additionally
runs all 56 existing bounded-merge scenarios through each of the three rate
shortcuts. It checks exact results and metadata frames, duplicate/overflow panic
messages, exactly-once conversion, Unicode names, overwriting and non-Clone,
non-Ord literal assets. Two controls change overflow rejection to success and
replace literal values with zero; each must re-extract and compile, then fail its
semantic proof. Panic messages and callback side-effect counts are finite Rust
evidence; the Lean contracts establish the logical results and panic class.

## Derived public traits

The `Derived*Refinement` modules connect 62 forwarding probes to production
methods: all 29 public derived Clone implementations, 17 derived PartialEq
implementations, quantity/fingerprint cmp/partial_cmp and their four comparison
predicates, and Quantity/AssetAmount/StateFingerprint/ObservationKey hashing.
A generic clone forwarding helper keeps the recursive expression clone dictionary
explicit for the pinned translator. The harness calls the actual Rust methods;
production types, fields, ownership and numeric bounds are unchanged. Extraction
guards require all 63 roots and 54 distinct production method definitions.

`DerivedValueRefinement` preserves arbitrary modeled primitive callback results
for quantity operations and the field-ordered amount comparisons, cloning and
hashing. PartialOrd predicates use the derived defaults through partial_cmp,
including None and failures, rather than invoking payload predicate overrides.
`DerivedExpressionRefinement` proves structural equality and identity cloning by
induction over every finite expression. `DerivedRecordRefinement` covers the
error/status variants, computed amounts and conditions. `DerivedCompositeRefinement`
composes map/vector behavior into proposal, trace and model-build-error equality
and cloning, and proves early termination on unequal or failing rate comparisons.

Whole-value equality requires independent total primitive equality laws against
logical equality. Generic basket/account/account-error equality instead compares
stored amounts by asset lookup, independently of insertion order, with explicit
usize length bounds. Its specification retains explicit zero entries and also
describes noncanonical logical inputs; it does not silently assume uniqueness or
replace map equality with equality of the representation list. No arithmetic law
is needed for these generic numeric payload contracts.

`DerivedCloneRefinement` composes all remaining public clones. Primitive payload
clones have independent total identity laws. Vec cloning uses the reviewed backend
model; the contracts do not assert callback traversal order or foreign side
effects. Tuple clones in rates and observation keys become Aeneas BuiltinClone,
whose identity result requires compatible payload clone behavior when interpreting
it in Rust. Economy cloning shares Arc payloads and clones account keys under the
existing logical Arc model. Physical allocation, reference counts, copy-on-write,
destructors, stack resources and storage representation remain dependencies.

Two equality adapters are now live. Pinned Rust `alloc/src/collections/btree/map.rs`
checks equal lengths and compares zipped entries with key equality before value
equality. The model preserves this order and stops at the first false result or
failure. Pinned `core/src/tuple.rs` expands tuple equality as left-to-right field
comparisons joined by `&&`; the triple adapter follows that sequence. Sorting's
unused tuple PartialOrd chaining hooks, including gt, still fail closed.
These adapters do not verify physical trees or the dependency implementations.

`DerivedKeyRefinement` proves equality of all three observation-key fields and
fingerprint equality/order. Quantity and amount hash composition retains the
supplied generic Hash/Hasher effects. Fingerprint and observation-key hashes use
the existing native 64-bit little-endian adapter and default Hasher integer and
length-prefix behavior. Observation account identifiers additionally require
compatibility with default per-element Hash::hash_slice; tuples inherit that
default. Arbitrary erased overrides are not covered. The composed observation
hash theorem emits three separate length-prefixed sequences in declaration order
and applies the proved FNV writer under independent identifier and numeric byte
encoding laws. Determinism does not establish collision freedom.

The Lean generator supplies 900 expression comparisons, 100 proposal comparisons,
64 ordered trace comparisons, 121 observation-key comparisons, 36 numeric
comparison/encoding cases and 49 basket comparisons. Rust exercises these through
distinct identifier types and the public API, also checking computed amounts and
conditions, all clone families, every error variant and large non-Copy BigUint
payload transport. Serialization is used only to supply some public test inputs
or observe records without PartialEq; this is not a serialization proof. That
milestone, numeric arithmetic laws, remaining iterator adapters
and arbitrary callback/destructor behavior remain separate completion obligations.
The inherited trait conveniences are covered by the next contract.

## Invariant input validation

`LinearInvariant` deserialization previously checked duplicates against weights
that had already been normalized. A zero first weight disappeared before the
next check, so `[(asset, 0), (asset, 2)]` was accepted while nonzero duplicates
were rejected. Repeated assets now fail before zero normalization.

The production `from_serialized_weights` helper retains all input entries during
validation, then builds the sparse weight map. Aeneas extracts both loops.
`InvariantInputRefinement.construct_refines` connects their complete result to
`InvariantInput.construct` over mathematical signed integers. It proves
termination and first-duplicate rejection for all finite represented vectors
under lawful asset equality and the existing logical map model. Success retains
the name and exactly the nonzero input coefficients in order, with unique keys.
The proof covers the full i64 range, including negative weights. No asset Clone
or Ord bound is added; public weight-update semantics remain unchanged.

The Lean generator supplies 262 input cases covering duplicate placement, zero,
negative weights, ordering and both i64 extremes. Rust exercises the actual
serde loader with a non-Clone/non-Ord asset type and checks that economy loading
propagates duplicate rejection. A production mutation that drops zero entries
before validation must compile after re-extraction and fail the refinement.

The serde-to-helper handoff is source-guarded and tested. Serde parsing,
serialization round trips, error conversion and text rendering are not proved
by this contract. The existing IndexMap/Vec models, lawful Eq/Hash behavior,
allocation, resource exhaustion, destruction, compilation and translation remain
explicit dependencies. The implementation uses a temporary map during validation;
no allocation or performance equivalence to the old loader is claimed.

## Inherited public traits

The 58 forwarding roots call the public Rust methods without changing production
source, generic bounds, visibility, ownership or borrowing. Four proof modules
register 78 contracts and compose the previously covered derived implementations:

- `InheritedCloneRefinement`: 29 `clone_from` methods return exactly the result
  of cloning the source, independent of the previous target. This includes modeled
  callback failure/divergence. Successful field preservation follows from the
  derived clone contracts and their primitive laws. Physical drops, allocation
  reuse, Arc counts and mutable state after Rust unwinding remain dependencies.
- `InheritedEqualityRefinement`: 19 `ne` methods negate the core type's `eq`
  result. They propagate its failures, and do not call a payload's `ne` override.
  Basket/account equality keeps its existing key-lookup, length and storage laws.
- `InheritedOrderRefinement`: quantity/fingerprint `min`, `max` and `clamp`
  preserve the exact comparison protocol. Min selects the first input on a tie;
  max selects the second. Clamp checks the bounds first, returns the lower bound
  without checking the upper bound when below it, and retains the input at equal
  bounds. Invalid or incomparable bounds produce Aeneas `assertionFailure`, the
  model of Rust's assertion panic. Independent Nat-key comparison laws justify
  exact total selectors without erasing distinguishable values with equal keys.
- `InheritedHashRefinement`: four slice-hash entry points perform ordered element
  hashing with no outer length prefix. The contracts prove empty identity, append
  composition, state threading, first-callback failure and finite success under
  primitive callback laws. They compose the existing element-hash contracts.

The pinned Rust default `core::hash::Hash::hash_slice` loops over the slice and
calls each element's `hash`. Aeneas cannot currently translate this std body:
the slice iterator reaches unsupported `PtrMetadata` in its interpreter. We model
this std default explicitly using the existing ordered `hashValues` fold. This
is a reviewed dependency boundary, not a proof of the standard library itself.
All four core Hash implementations inherit the default, so these outer calls do
not dispatch to a payload `hash_slice` override. Internal observation-key vector
hashing still has the native LE64/default-method restrictions documented above.
Arbitrary erased overrides and portability outside that scope remain open.

Six new external interfaces were reviewed against the pinned Rust sources:
the default hash-slice loop; the separate primitive u64 bulk-byte hash-slice
override; pair/triple `ne` overrides; Box `ne`; and String `clone_from`. Tuple
inequality calls payload `ne` left-to-right with short-circuiting, and Box delegates
to its payload. String replacement models contents, with allocation and drops
outside scope. The unused u64 slice override preserves its distinct single-write
behavior, even for an empty slice, under a representable byte-length bound.
Generic adapter universes were widened to match newly retained dictionaries;
their storage and callback semantics are unchanged.

The backend omits the Hash `hash_slice` and Iterator `size_hint` dictionary fields.
The diagnostic guard permits exactly one of each reviewed warning; missing,
duplicated or other diagnostics fail. All 58 new roots and the existing concrete
core methods are mandatory. Each slice root resolves to the named default adapter
with its concrete extracted core dictionary, rather than an erased override.

Lean generates 36 numeric pairs, 216 clamp cases and eight slice sequences with
exact hasher write chunks, plus nine tagged order pairs and 27 tagged clamp cases.
Rust reuses structural fixtures for replacement and
inequality, checks all clone families, uses distinct typed identifiers and tests
tags that distinguish equal-key scalar values. Deliberately panicking payload
convenience overrides check that core defaults use the specified dispatch.
Serialization only supplies some inputs and observations; item 17 remains open.
Production mutation controls omit clone replacement or slice traversal and require
successful extraction/compilation before the semantic proofs reject the change.
