# Axionomy development contract

Axionomy core features must carry Lean proofs. This is a product requirement,
alongside Rust's ownership and strong typing guarantees.

Commit and push each completed, validated milestone to `origin/main` unless the
user directs otherwise. Keep commits focused on the milestone's changes.

For changes to the root crate (`src/**`, its dependencies, or semantics promoted
into it), read `verification/README.md` and `verification/coverage.json` first.

- State the behavior as a typed Lean specification and prove the relevant
  property before treating a new or changed core feature as complete. Extend
  existing theorems when applicable. Tests alone do not satisfy this requirement.
- Keep `AccountId`, `Asset`, `Role`, `RateId`, and numeric backends distinct in
  Rust. Keep semantic state private and expose mutation only through the core.
  Do not replace typed APIs with strings, JSON, unchecked casts, or runtime-only
  checks to make extraction easier. Lean supplements the Rust type checker.
- Connect the specification to Rust with conformance/regression tests. For the
  covered executable domain, regenerate fixtures from Lean, never by hand.
- Register the feature, theorem names, Rust evidence, precise proof boundary,
  and reviewed source hashes in `verification/coverage.json`. A source hash is
  a review acknowledgement, not a proof; never update it merely to silence CI.
- Do not introduce `sorry`, `admit`, new axioms, or assumptions that restate the
  desired conclusion. The checker audits elaborated dependencies. Review
  theorem premises for satisfiability and avoid vacuous or circular contracts.
- Run `python3 scripts/check_proofs.py` and the appropriate Rust checks before
  completing core work. Preserve the workspace's existing checks.
- Report verification precisely. The current baseline proves Aeneas-extracted
  u64 checked arithmetic, basket construction/operations, account deposit/withdrawal,
  model/rate construction and metadata discovery, complete transaction assessment/application
  and finite-vector candidate filtering, fork/simulation/ordered replay, snapshot expressions/conditions,
  bounded effect aggregation, availability/preservation, preparation, atomic commit,
  receipts, runtime signed conservation and whole-state diagnostic measurement
  against Lean specifications. Collections use
  explicit trusted models; other contracts have design proofs and finite Rust
  conformance tests. This is not a proof of the
  entire Rust implementation. Preserve fresh extraction and its explicit trust
  boundary when extending implementation proofs.
  Existing refinement gaps are recorded in `verification/README.md`; new features
  must not silently expand those gaps or claim a stronger verification level.

Search, problem encodings, transport, Studio, and authoring adapters are not the
execution kernel. Moving authority into them is not a way around this contract.
Public type and denomination guarantees in those adapters must remain intact.
