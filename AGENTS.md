# Axionomy development contract

The kernel's meaning is written down once, in Lean: `verification/lean` states
what quantities, baskets, rates, exchanges, conservation, atomic commit,
replay and rounding mean, and proves the properties that make the economy
sound. The Rust kernel (`src/**`) is tested against that specification: the
Lean model generates `verification/conformance.json`, and the Rust tests must
reproduce every expected outcome exactly.

Commit and push each completed, validated milestone to `origin/main` unless the
user directs otherwise. Keep commits focused on the milestone's changes.

- Change the specification with the behavior. When kernel semantics change,
  update the Lean model, prove the properties that matter, and regenerate the
  fixtures from Lean, never by hand:
  `cd verification/lean && lake build && lake exe conformance > ../conformance.json`.
- `lake build` treats warnings as errors, so `sorry` cannot land. Do not add
  axioms or premises that restate the conclusion.
- Prove semantics, not plumbing. Theorems are for economic meaning:
  conservation, atomicity, determinism, rounding against the caller, exact
  arithmetic bounds. Do not write proofs or fixtures for accessors, derived
  traits, iterators or formatting.
- Keep `AccountId`, `Asset`, `Role` and `RateId` distinct in Rust. Keep semantic
  state private and expose mutation only through the core. Do not replace typed
  APIs with strings, JSON, unchecked casts or runtime-only checks.
- Write idiomatic Rust. The specification constrains behavior, not code shape.
- Run the Rust checks (`cargo fmt --check`, `cargo clippy --workspace
  --all-targets --all-features -D warnings`, `cargo test --workspace
  --all-targets --all-features`) and the spec check before completing core work.
- Manage any Python with uv and a committed `uv.lock`. The MCP server is checked
  against the official MCP Python SDK: `cargo build -p axionomy-mcp --bin
  axionomy-mcp && cd crates/axionomy-mcp/interop && uv run --locked pytest`.

Search, problem encodings, transport, Studio and authoring adapters are not the
execution kernel. Moving authority into them is not a way around this contract.
Public type and denomination guarantees in those adapters must remain intact.
