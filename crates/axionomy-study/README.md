# axionomy-study

The interface-neutral study runtime for Axionomy. It describes capabilities,
accepts reproducible study requests, and returns replay-derived studies without
depending on HTTP, CLI, MCP, async runtimes, databases, a browser, or any
particular problem.

A study is what the runtime computes for a request: a problem's instance under
its strategies, with a seed and a budget. Each document of a study holds a run,
one trajectory replayed through the core, and the evidence of how it was found.
"Run" means only that trajectory.

A problem is a type implementing `Problem`: its catalog entry, a
`ProblemDescriptor`, and how a study's documents are computed. A `Registry`
holds the problems a front end offers, each under its own key. The canonical
problems live in `axionomy-problems`, whose `registry()` registers them all; an
ontology of your own is studied in Studio, MCP and the CLI by registering its
`Problem` beside them or instead of them, as "Study your own problem" in
[`axionomy-problems`](../axionomy-problems/README.md) shows. External
interfaces take a `Registry`, call the same methods and are tested for
semantic parity.

Its deliberately small vocabulary is `Problem`, `Registry`,
`ProblemDescriptor`, `StudyRequest`, `StudyControl`, `StudyProgress`,
`Progress`, `SearchObservationView`, `Study` and `StudioEvent`.
Problems, strategies and instances are paths (`work-league`, `a-star`,
`showcase`). A problem's catalog entry declares its own instances, family and
capabilities, each a path with a label, so front ends show what it declares and
keep no table of their own; the solver work it reports labels its phases and
algorithms the same way.
`Registry::resolve` decides a request's instance and strategy and returns the
`StudyKey` the study records: the same key always gives the same study.
`Registry::study` computes it: the problem computes the documents, and the
registry checks their scenes and cues and assembles them. A study includes all
replayable alternatives, the selected document, model and observation
projections, decision evidence, and assessed proposals. Pause, resume, and
cancel are cooperative runtime signals; they cannot mutate an economy or
authorize an exchange.

Progress, solver observations, and verified replay frames share one observer
boundary. Observations are streamed at work checkpoints as `thinking` events,
and a bounded history is retained as each view document's `thinking`, giving
live HTTP and Worker clients the same evidence as offline consumers of a saved
study. Thinking is trying exchanges at a point and playing them out before
choosing, with the same apply as the step finally chosen.

A problem reports through `StudyProgress`: progress and the thinking it makes,
frames, and each document's trajectory, with `StudyControl` checked at every
report and at `StudyProgress::checkpoint`. Problems with resumable algorithms
advance Monte Carlo, MCTS, and ISMCTS in small deterministic work chunks, and
`Progress` reports phase-local samples, iterations, nodes, and game
moves through one ordered observer. One-shot problems retain the same contract
and check control between their indivisible phases.

A problem builds its documents with the runtime's helpers, which derive every
frame by core replay: `document` replays a trace from its initial state into a
view document and reports its trajectory, `proposal` assesses an exchange
against a snapshot, `telemetry` counts an algorithm's work, `simulate` and
`frontier_trace` find the traces documents show, and `strategy` reads a
study's strategy as the problem's own type. How a problem's values are
labeled, formatted and drawn (`Label`, `Format`, scenes and Pareto fronts) is
`axionomy-view`'s.
