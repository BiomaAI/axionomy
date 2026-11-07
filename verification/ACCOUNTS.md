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
handoff in `Economy::analyze`; the caller is not extracted by this theorem.
Role/account validation, effect aggregation, preserved-resource preflight,
invariant checking, prepared-account completeness, multi-account commit, and
the caller's remaining `expect` sites remain separate proof obligations.

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
modules. Its new entrypoints only forward to the three fallible static rate
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
proofs adds an axiom. Whole-state measurements and economy diagnostics remain
separate obligations, as do BigUint/BigInt and custom backend laws.

Fresh extraction, external-interface checksums, the Lean kernel/axiom audit,
464 conservation fixtures and 56 merge fixtures (each run through three builders)
cover the new paths. Negative controls reverse the conservation result and discard
a rate overflow: the regenerated Rust programs compile, but their implementation
proofs must fail.
