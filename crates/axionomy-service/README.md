# axionomy-service

The interface-neutral application boundary for Axionomy. It describes
capabilities, accepts reproducible run requests, and returns replay-derived
artifacts without depending on HTTP, CLI, MCP, async runtimes, databases, or a
browser.

`ReferenceService` adapts the twelve canonical conformance problems. External
interfaces call the same service and are tested for semantic parity.

Its deliberately small vocabulary is `ProblemDescriptor`, `RunRequest`,
`RunControl`, `ServiceProgress`, and `RunArtifact`. A run artifact includes all
replayable alternatives, the selected document, model and observation
projections, decision evidence, and assessed proposals. Pause, resume, and
cancel are cooperative runtime signals; they cannot mutate an economy or
authorize an exchange.

Adapters with resumable algorithms advance Monte Carlo, MCTS, and ISMCTS in
small deterministic work chunks. `ServiceProgress` reports phase-local samples,
iterations, nodes, and game moves through one ordered observer, while
`RunControl` is checked at every chunk boundary. One-shot adapters retain the
same contract and check control between their indivisible phases.

Every problem owns an adapter here—not in `axionomy-problems` and not in a
transport. That adapter lowers user-defined Rust ontologies into the
monomorphic `axionomy-view` contract and derives every frame by core replay.
