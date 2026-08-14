# axionomy-service

The interface-neutral application boundary for Axionomy. It describes
capabilities, accepts reproducible run requests, and returns replay-derived
artifacts without depending on HTTP, CLI, MCP, async runtimes, databases, or a
browser.

`ReferenceService` adapts the fourteen canonical conformance problems. External
interfaces call the same service and are tested for semantic parity.

Its deliberately small vocabulary is `ProblemDescriptor`, `RunRequest`,
`RunControl`, `ServiceProgress`, `SearchObservationView`, and `RunArtifact`. A run artifact includes all
replayable alternatives, the selected document, model and observation
projections, decision evidence, and assessed proposals. Pause, resume, and
cancel are cooperative runtime signals; they cannot mutate an economy or
authorize an exchange.

Progress, solver observations, and verified replay frames share one observer boundary. Observations are
streamed at work checkpoints and a bounded history is retained in each view
document, giving live HTTP and Worker clients the same evidence as offline
artifact consumers.

Adapters with resumable algorithms advance Monte Carlo, MCTS, and ISMCTS in
small deterministic work chunks. `ServiceProgress` reports phase-local samples,
iterations, nodes, and game moves through one ordered observer, while
`RunControl` is checked at every chunk boundary. One-shot adapters retain the
same contract and check control between their indivisible phases.

Every problem owns an adapter here—not in `axionomy-problems` and not in a
transport. That adapter lowers user-defined Rust ontologies into the
monomorphic `axionomy-view` contract and derives every frame by core replay.
