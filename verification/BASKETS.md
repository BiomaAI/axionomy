# Implementation proof: basket construction

The constructor contract below is complemented by the production operation
proofs in `Axionomy/BasketOperationsRefinement.lean`. These use the shared-source
account extraction so account and exchange proofs can reuse the same basket
types and methods.

## Addition, scaling, containment, and equality

For u64 quantities, `add_refines_naturals` and `scale_refines_naturals` prove
termination and exact natural-number results of `Basket::checked_add` and
`checked_scale`, including first-overflow asset order. Addition preserves its
original ordered state on failure. Scaling returns no partial basket and removes
zero products. Neither theorem assumes success. They use lawful asset equality
and cloning, and the existing ordered-map abstraction.

`contains_refines` proves total execution and the exact shortfall-based Boolean;
`contains_refines_naturals` identifies success with sufficient balance for every
requested asset. Required asset identifiers must be distinct, as for the earlier
shortfall proof. The equality theorem proves total execution for baskets whose
lengths fit `usize`; with unique identifiers, equality holds exactly when their
entry lists are permutations. It therefore ignores insertion order while still
checking asset identities, quantities, and additional entries.

`BasketOperations.Valid` means unique asset identifiers and no zero entries.
The store, credit, debit, addition, scaling, shortfall, and exchange-staging
theorems preserve this representation invariant under their stated premises.
They complement the constructor invariant and establish facts needed by later
calls; they do not assume the result of the operation being verified.

The production addition/scaling loops only rename local `quantity` bindings to
`amount`, avoiding generated namespace shadowing. Basket equality replaces
`Iterator::all` with an equivalent short-circuiting loop because the pinned
Aeneas iterator trait lacks `all`. It still uses the original `PartialEq::eq`
comparison (not a possibly user-overridden `ne`) and retains all public bounds.
No generated Lean or intermediate LLBC is patched.

The account extraction's reviewed external boundary adds two explicit definitions:
Option equality invokes the supplied comparison and propagates its failures;
map length returns the logical entry count when it fits `usize`, otherwise fails
closed. Physical collection correctness/allocation remains trusted. Equality's
length premise excludes impossible Rust collections; uniqueness excludes invalid
duplicate-key map representations. The external-template checksum pins these
interfaces. No axioms are added.

The Lean specification generates 654 cases for the public Rust operations,
covering both entry orders, empty/different-key baskets, zero scaling, u64
boundaries, and late overflow. Isolated mutations that partially commit addition
or ignore the scaling factor must compile after extraction and then fail the
operation refinement proof. Generic iterator construction is covered below.
Sorted views, serialization and numeric backend arithmetic remain separate
obligations; the complete u64 exchange evaluator is covered in [ACCOUNTS.md](ACCOUNTS.md).

`Basket::try_from_entries` used to discard a zero entry before checking the next
asset. As a result, `[(A, 0), (A, 1)]` and `[(A, 0), (A, 0)]` incorrectly succeeded
despite the constructor's duplicate-rejection contract. Construction now records
every entry, rejects the first repeated asset, and only then removes zeros.
Successful output keeps the original nonzero quantities and their insertion order.
The temporary map includes zeros, so construction can use more space than the
final sparse basket. Public signatures, trait bounds, and private state are intact.

## Contract and theorem

`Axionomy/Basket.lean` specifies construction over a typed list of asset/quantity
pairs. It proves that construction succeeds exactly when asset identifiers are
unique, zeros cannot hide duplicates, and canonicalization preserves nonzero
entries, uniqueness, and order. Asset identities remain an arbitrary type,
independent of the quantity type.

`Axionomy/BasketRefinement.lean` proves that the Aeneas-extracted constructor
returns exactly this specification for every `Vec<(A, Quantity<u64>)>` represented
by the backend, including empty input and all u64 values. Its explicit equality
premise requires the supplied asset equality operation to terminate and agree
with identity. `exactEquality_lawful` gives a concrete satisfying dictionary;
the premise does not assume the constructor's correctness. The first repeated
asset is returned even when either occurrence has quantity zero.

The refinement theorem provides an execution `Result.ok` witness as well as
equality with the specification. Thus the translated program terminates without
panic or divergence under the models below. A Rust `Err(DuplicateAsset)` is a
successful execution with a rejected input, distinct from an execution failure.

## Connection to production Rust

`extract_basket.rs` compiles the exact `src/basket.rs`, `src/quantity.rs`, and
`src/amount.rs` files using `#[path]`. It forwards a vector to the same
`construct_entries` function called by the public constructor. There is no copied
constructor algorithm. The source checker verifies both forwarding bodies and
the production paths. Coverage hashes record review of the complete sources.

This harness lets Charon read promoted MIR before generic drop elaboration,
which the pinned backend cannot translate for this loop from dependency MIR.
The constructor uses a private `N: Zero` helper and `Quantity::scalar_is_zero`
to avoid extracting unrelated formatting and signed-measure dictionaries.
Public APIs still require the original `QuantityScalar`, `Eq`, and `Hash` bounds.

The pinned backend also cannot directly translate `IndexMap::retain`'s mutable
value callback. A thin production adapter exposes a shared quantity reference.
Its exact body, `quantities.retain(|_, quantity| keep(quantity));`, is checked
before extraction. The adapter is modeled; the actual zero-checking predicate,
duplicate checks, iteration, insertion calls, and constructor control flow are
extracted. Neither generated Lean nor intermediate LLBC is manually rewritten.

## Explicit trust boundary

This is an implementation refinement under a collection abstraction, not a proof
of the physical hash table. In addition to the compiler and translation boundary
documented in [AENEAS.md](AENEAS.md), it relies on:

- Aeneas's primitive u64, vector, iterator, and execution-effect models. Inputs
  are finite vectors whose length fits `usize`; arbitrary Rust iterators and
  their effects are not covered by the refinement theorem.
- `BasketExtraction/TypesExternal.lean` representing `IndexMap<K,V,S>` as an
  ordered list of typed key/value pairs. Hash seeds and hasher state are opaque
  units in this abstraction.
- `BasketExtraction/FunsExternal.lean` modeling map creation, lookup, insertion,
  and the retain adapter. Lookup invokes supplied equality; insertion preserves
  order; retention invokes the extracted callback in order and preserves values.
  These are executable definitions, not axioms asserting the desired result.
- The actual collection implementing these operations with lawful, stable,
  terminating `Eq`/`Hash` behavior and successful allocation. The logical map
  does not execute hashing, allocation, destructor effects, or table resizing.
  Rust's trait bounds alone do not prove these laws. Panicking or inconsistent
  custom hash/equality implementations are not certified.

Unused hasher internals are deliberately modeled as execution failures, not
invented hashing semantics. The constructor proof succeeds because the logical
map never calls them. Any future extracted path that observes these internals
needs a new model and proof. The u64 addition dictionary inherited by `Zero`
uses Aeneas's primitive addition and is likewise unused by construction.

The public forwarding call is checked syntactically. The theorem specializes its
shared implementation to vector inputs and u64 quantities. The public-collection contracts below now connect generic entry/asset-qualified
construction and convenience conversions directly to the AccountExtraction model.
JSON deserialization still has finite conformance evidence and remains item 17.
Remaining serialization/schema, derived/convenience API and
numeric-backend gaps remain explicit in `coverage.json`.
The subsequent [account proof](ACCOUNTS.md) covers deposit/withdrawal and their
basket lookup, insertion, and shortfall paths under the same storage abstraction.

## Regenerate and check

Use the tool setup in [README.md](README.md), then:

```sh
python3 scripts/extract_basket.py --write
# Review generated changes, reprove the contract, and review source coverage.
python3 scripts/check_proofs.py
python3 -m unittest discover -s scripts -p 'test_*.py'
```

The gate regenerates both arithmetic and basket models from current Rust,
rejects changed external interfaces or stale generated files, rechecks proof
terms, and audits all Axionomy declarations for forbidden axioms. It also checks
260 Lean-generated basket cases through direct construction, asset-qualified
construction, and JSON deserialization. Those cases supplement the universal
constructor theorem; they are not its basis.

An isolated mutation test restores zero filtering before duplicate validation.
It verifies that stale extraction is rejected and that freshly extracting the
buggy Rust makes the basket refinement proof fail. Other negative controls
reject changed forwarding bodies, copied proof-only sources, altered storage
interfaces, missing generated definitions, and fabricated axioms.

## Public collection boundary

`BasketCollection` generalizes the independent constructor specification to typed
values and an explicit zero predicate. It proves success exactly for distinct
asset identifiers, nonzero contents, insertion-order preservation and canonical
uniqueness. The predicate describes a backend operation; it is not an assumption
about the desired constructor result. The Nat specialization is proved equal to
the existing `Basket.construct` specification.

`BasketCollectionRefinement` connects the actual generic production constructor
to that specification. `entries_contract` derives the successful representation
and its `usize` size bound from distinct, bounded input. The helper refinements
operate under the existing logical map model; actual allocation and physical map
capacity remain dependency assumptions. Generic Zero laws are independent of the
arithmetic laws still tracked in item 18. `BasketPublicRefinement.u64_entries_refines`
discharges the concrete u64 zero predicate and returns the exact existing
natural-number construction result.

The independent `IteratorContract.Prefix` protocol records only successful
primitive `next` calls, without any premise about the remaining iterator.
`entries_reject_prefix` and `amounts_reject_prefix` therefore prove duplicate
rejection even when the unused tail would panic or never terminate. Zero values
still establish an asset's presence. These rejection paths do not invoke the zero
predicate, so they need no zero-callback success premise. `FailsAfter` contracts
propagate input failures if no earlier duplicate has already ended construction;
separate public contracts handle conversion-to-iterator failures.

`BasketAmountsRefinement` extracts the private `AmountEntries` iterator and proves
that it converts precisely one asset-qualified amount per yielded value. The
production implementation replaces the generic `.map` call with this explicit
`next`/`into_parts` adapter. It does not precollect the input or impose new public
trait bounds. This avoids assuming correctness of custom `map` overrides that the
pinned Aeneas Iterator dictionary does not represent. Lawful default map behavior
has the same result; custom override invocation can differ.

FromIterator and owned-array conversion return exactly the canonical basket, or
panic exactly on duplicate input under successful iterator/equality/zero laws.
Early duplicate panic also needs only an observed prefix. Aeneas represents the
panic class; the current message is checked by Rust conformance, not proved as a
formatting contract. Array conversion now directly calls `Self::from_iter`,
preserving its typed signature and duplicate policy.

The only added dependency interfaces are the std owned-array iterator's type,
`into_iter` and `next`, plus reuse of the source-guarded `retain_quantities` adapter
already modeled in BasketExtraction. The array model is an executable ordered
list of remaining initialized values. Review of the pinned nightly-2026-09-17
`core/src/array/iter.rs` and `iter/iter_inner.rs` confirms initialization of the
alive range to `0..N` and forward consumption of its first element. Physical
MaybeUninit storage, unsafe moves, destructor effects, allocation and std internals
remain trusted. The source guard checks retention in both extraction pipelines;
its extracted zero predicate is proved. Compiler/translation and existing
primitive/hash/collection boundaries remain unchanged.

Basic new/default, len/is_empty, get and insertion-order borrowed iteration now
have explicit production refinements. Length has a representable-size premise.
Sorted borrowed iteration is covered by the subsequent contract below.

The Lean generator adds 85 collection scenarios: all lists through length three
from zero/nonzero/maximum-u64 values and overlapping typed asset identifiers.
Rust exercises entry construction, typed amounts, FromIterator, fixed arrays,
queries and borrowed iteration. Scripted non-fused inputs reject polling beyond
None and override map/fold/size_hint with panics. Each case also runs with a
panicking tail to check first-error precedence; non-Copy BigUint values exercise
the typed-amount adapter without claiming BigUint arithmetic verification.
Two production mutation controls skip a typed amount or silently accept duplicate
collection input; both must translate successfully and then break the relevant
implementation proof.

This milestone passed the full proof gate with 388 registered contracts and
3,378 audited Axionomy theorems, fresh extraction, all 311 workspace tests,
90 boundary/gate controls and both new production mutation controls. Formatting,
Clippy with warnings denied, documentation and compile-fail doctests also passed.
The prior 44 production mutation controls remain in CI.

## Sorted borrowed iteration

`BasketOrdering` specifies stable ordering by asset alone. Its proofs establish
permutation and length preservation, weak asset order, preservation of nonzero
entries and distinct keys, strict order for distinct keys, stable equal-key pairs,
and equal canonical output for permutations of a basket with unique keys.
Quantities remain arbitrary typed payloads; these proofs never compare or clone
them.

`BasketSortedRefinement.sorted_refines` connects the extracted production
`iter_sorted` call to that specification and proves the returned iterator's
complete sequence. `sorted_contract` gives exact contents, order and length.
`canonical_contract` retains the representation invariants. `constructed_sorted`
composes the real generic constructor with sorted iteration: independent primitive
iterator/equality/zero laws, distinct input and a size bound establish the
representation and size premises needed by sorting. No premise assumes the
constructor or sorting call succeeds. Asset comparison must be pure, total,
identity-consistent and satisfy the independent `OrderedEffects.OrderLaws`.

The production refactor moves the temporary mutable slice into the generic
`sorted_entries` helper, then returns the owned vector of borrowed entries. This
avoids the pinned translator's nested-borrow limitation without cloning, changing
the public borrowed return type, imposing a scalar trait bound or replacing the
storage algorithm. `Vec::from_iter(self.iter())` expresses the same collection
call directly, avoiding the erased `Iterator::collect` dictionary method.
`sort_by` compares asset references directly; the former `sort_by_key` also
compared those asset references. Equality of behavior requires lawful Rust
`Ord`/`PartialOrd`, as does the public sorted-view contract.

The only added external interface is `alloc.slice.Slice.sort_by`. Review of the
pinned nightly-2026-09-17 Rust `alloc/src/slice.rs` confirms stable ascending
sorting through the callback; both public sorting methods call `stable_sort`.
The existing logical sorting model now threads callback state explicitly and
propagates callback failures. The public refinement discharges that callback
with the extracted stateless asset comparator and assumes its independent laws.
Calculations over this dependency model do not specify std's comparison order
or count, unlawful/stateful comparisons, interior mutability, physical driftsort,
performance, allocation, destructors or unwind state. Physical collection
storage, Vec collection/iteration, compiler and translation remain trusted.

The Lean generator enumerates all 326 ordered subsets of five typed asset/value
entries, including zero and maximum-u64 quantities. Rust checks ascending and
reverse ontology order, exact quantities, unchanged source insertion order and
pointer identity of borrowed keys and quantities. Assets deliberately omit
`Clone`; an empty basket with an unordered non-cloneable scalar checks the lack
of scalar bounds. The same fixtures exercise non-Copy BigUint payloads without
claiming arithmetic verification. A production mutation reverses the asset
comparison; it must extract and compile successfully, then fail the refinement.

This batch passed the full proof gate (401 registered contracts; 3,412 audited
Axionomy theorems), fresh extraction, all 314 workspace tests, 93 boundary/gate
controls and the reversed-comparator production mutation. Formatting, Clippy
with warnings denied, documentation and compile-fail doctests also passed.
The prior 46 production mutation controls remain in CI.

The public u64 `basket([(asset, value), ...])` helper now has a composed
`ConvenienceRefinement` contract. The extracted closure preserves each literal
value; array mapping followed by collection returns exactly the canonical basket
or panics exactly on duplicate keys under lawful equality. Zero entries still
participate in duplicate detection. The reviewed std array-map boundary and the
781 generated literal cases are documented in
[the convenience-constructor contract](ACCOUNTS.md#convenience-constructors-and-string-conversion-callbacks).
