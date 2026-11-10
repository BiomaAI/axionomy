# axionomy-service

The interface-neutral application boundary for Axionomy. It describes
capabilities, accepts reproducible study requests, and returns replay-derived
studies without depending on HTTP, CLI, MCP, async runtimes, databases, or a
browser.

A study is what the service computes for a request: a problem's instance under
its strategies, with a seed and a budget. Each document of a study holds a run,
one trajectory replayed through the core, and the evidence of how it was found.
"Run" means only that trajectory.

`ReferenceService` adapts every canonical conformance problem. External
interfaces call the same service and are tested for semantic parity.

Its deliberately small vocabulary is `ProblemDescriptor`, `StudyRequest`,
`StudyControl`, `ServiceProgress`, `SearchObservationView`, `Study` and
`StudioEvent`. Problems and strategies are paths (`work-league`, `a-star`),
and an instance is named by its profile: `micro`, `showcase` or `stress`.
`ReferenceService::resolve` decides a request's instance and strategy and
returns the `StudyKey` the study records: the same key always gives the same
study. `ReferenceService::study` computes it. A study includes all replayable
alternatives, the selected document, model and observation projections,
decision evidence, and assessed proposals. Pause, resume, and
cancel are cooperative runtime signals; they cannot mutate an economy or
authorize an exchange.

Progress, solver observations, and verified replay frames share one observer
boundary. Observations are streamed at work checkpoints as `thinking` events,
and a bounded history is retained as each view document's `thinking`, giving
live HTTP and Worker clients the same evidence as offline consumers of a saved
study. Thinking is what a policy does when it simulates before it chooses.

Adapters with resumable algorithms advance Monte Carlo, MCTS, and ISMCTS in
small deterministic work chunks. `ServiceProgress` reports phase-local samples,
iterations, nodes, and game moves through one ordered observer, while
`StudyControl` is checked at every chunk boundary. One-shot adapters retain the
same contract and check control between their indivisible phases.

Every problem owns an adapter here—not in `axionomy-problems` and not in a
transport. That adapter lowers user-defined Rust ontologies into the
monomorphic `axionomy-view` contract and derives every frame by core replay.
