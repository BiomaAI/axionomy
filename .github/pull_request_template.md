Describe the problem and resulting behavior.

## Core proof contract

For core semantic changes, provide the feature ID from
`verification/coverage.json`, the Lean theorem names, their relevant premises,
and the Rust conformance evidence. Explain any changes to the proof boundary.
For changes outside the core, state that scope.

- [ ] New or changed core behavior has a checked Lean specification and proof.
- [ ] Rust ontology types, ownership, and mutation boundaries remain intact.
- [ ] The source-to-spec relationship and source hashes were reviewed.
- [ ] Verification claims distinguish design proofs, finite tests, and Rust refinement.

## Validation

List the proof gate and Rust checks run, plus any checks not run.
