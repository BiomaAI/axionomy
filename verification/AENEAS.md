# Implementation proof: checked u64 quantities

This first implementation proof covers `Quantity<u64>::checked_add`,
`checked_sub`, `checked_mul`, and `checked_div`. Each theorem quantifies over all
pairs of u64 values, proves successful termination of the generated program,
and equates its `Option` result to the existing mathematical specification.
Overflow, underflow, and a zero divisor produce `None`; division rounds down.
There are no input bounds beyond the u64 type itself.

The separate [basket construction proof](BASKETS.md) extends this pipeline with
shared-source extraction and an explicit collection abstraction. The arithmetic
proof's intrinsic boundary described here remains unchanged.
The [account update proof](ACCOUNTS.md) extracts deposit, withdrawal, shortfall,
and the production per-account exchange staging helper using the production
generic trait dictionary and the collection abstraction. That shared pipeline
also proves static rate builders and the runtime signed conservation predicate;
see its extended trust boundary in [ACCOUNTS.md](ACCOUNTS.md).

`Axionomy/QuantityRefinement.lean` contains the four refinement theorems. Its
`Refines` predicate requires an actual `Result.ok` witness as well as equality
of the decoded quantity and the specification. A panic or divergence cannot
satisfy this predicate. Successful `None` is distinguished from an execution
failure. The kernel audit permits only Lean's standard logical axioms.

## Production code and extraction

`extract.rs` contains four forwarding entry points calling the public API. It
is a Cargo example in the root package, so it shares production sources and
`Cargo.lock`. It does not implement an alternative arithmetic algorithm.

Charon reads the optimized MIR of these entry points with MIR inlining enabled
and pinned thresholds. This specializes the real generic call paths to u64,
including `num_traits` dispatch and non-negativity validation. It avoids pulling
the entire generic trait dictionary (formatting, hashing, signed measures, etc.)
into this small proof. Rust still type-checks the original generic API with all
its trait bounds and ownership rules. No bounds are removed from Rust.

The only production refactoring is an explicit `from_checked` conversion in
place of `and_then(...Result::ok)`. It retains scalar validation and the existing
public signatures. The generic standard-library combinator's drop handling
otherwise triggers a translation limitation in this Aeneas version.

The generated `QuantityExtraction/Types.lean`, `QuantityExtraction/Funs.lean`,
and `QuantityExtraction.lean` are committed and regenerated before checking
proofs. Do not edit them by hand. Fresh LLBC is created in a temporary directory;
reusing a previous extraction cannot make the gate pass.

## Explicit trust boundary

The theorem checks the translated program against the specification. It relies on:

- Rust's type checker and MIR transformations, Charon, and Aeneas translating the
  selected production source faithfully. These tools are pinned, not proved by
  this project. LLVM, machine code, the operating system, and hardware are outside
  the theorem. Ordinary builds continue to support the existing Rust MSRV.
- Aeneas's models of Rust primitive integer operations and execution effects.
- Three explicit adapters in `QuantityExtraction/FunsExternal.lean`:
  `add_with_overflow` and `mul_with_overflow` reuse Aeneas's u64 bitvector
  overflow models; `cold_path` is a compiler hint modeled as a no-op. The
  generic adapters support only u64 and fail for other types. The extraction
  checker rejects non-u64 instantiations and any changed external template.
- Three definitional aliases for `U64.add/sub/div`: the pinned generator emits
  these names, whereas its backend exports the generic `UScalar` operations.
  The aliases add no new arithmetic semantics.
- The specification accurately expressing the intended contract. Review remains
  necessary; a proof cannot establish that a chosen contract is the right one.

The intrinsic adapters are explicit semantic models, not extracted intrinsic
implementations. They introduce no axioms or assumptions that restate the desired
correctness theorem. All Axionomy declarations, including the generated code and
these adapters, have their transitive axiom dependencies audited.

This proof is for the default-feature extraction configuration and the u64
backend. It does not certify BigUint, arbitrary `QuantityScalar` implementations,
signed weighting, serialization, collections, accounts, rates, or exchanges.
Their existing coverage and gaps remain listed in `coverage.json`.

## Reproduce and maintain

Requires Python 3.12+, Git, rustup, elan, and a native compiler/linker. The pinned
binary release supports Linux x86_64/aarch64 and macOS arm64.

```sh
python3 scripts/setup_aeneas.py
(cd verification/lean && lake update)
python3 scripts/check_proofs.py
python3 -m unittest discover -s scripts -p 'test_*.py'
```

Setup verifies the Aeneas release archive checksum and builds Charon from
Aeneas's exact pinned commit. It installs the extraction Rust nightly without
changing the default Rust toolchain. The release's Lean backend and the project
use the same Lean version. `lean/lake-manifest.json` pins transitive Lean packages.

Setup also applies `patches/aeneas-namespaced-discriminant.patch` to the Lean
support code. It gives generated enum discriminant instances fully qualified
names, allowing multiple extractions of the same Rust enum to coexist in the
combined audit. `aeneas-toolchain.json` pins the patch and the before/after source
hashes; unknown source changes are rejected. The translation binary is unchanged,
and the patch changes names only. All resulting terms still undergo kernel checks.

After intentionally changing production arithmetic:

```sh
python3 scripts/extract_quantity.py --write
# Review the generated diff, repair the refinement proof, and review coverage.
python3 scripts/check_proofs.py
```

The gate rejects a stale generated model even if its old proofs still compile.
It also rejects new external declarations until their semantic boundary is
explicitly reviewed. Regeneration never overwrites the handwritten proofs or
intrinsic adapters. Tests remain supplementary evidence for the source refactor
and the wider core.

An end-to-end mutation test reverses subtraction in an isolated copy of the
production source. It verifies both that the old extraction is rejected and that
regenerating the changed implementation causes its refinement proof to fail.

Upstream documentation: [Aeneas workflow and limitations](https://github.com/AeneasVerif/aeneas/tree/4acbb6d73abe9082e45cf49ba6d6229a2c7d5836).
