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
operation refinement proof. Arbitrary iterators, sorted views, serialization,
custom backends, and the complete exchange evaluator remain separate contracts.

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
shared implementation to vector inputs and u64 quantities. Asset-qualified
construction and JSON deserialization also use that implementation, but their
adapters have finite conformance evidence, not separate implementation proofs.
The existing mutation, arithmetic, account, rate, exchange, serialization, and
arbitrary numeric-backend gaps remain explicit in `coverage.json`.
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
