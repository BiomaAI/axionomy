# Simulation: policies, runs and studies

Status: planned, in the order the steps below give. Built: rollouts and
Monte Carlo episodes recording where they run, steps 1, 3, 4 and 5, and step
2 for MCP.

The economy owns commitments and their consequences. A policy reads what an
actor may see and proposes exchanges. Whoever calls the rollout (a problem, a
study, an MCP client, an outside harness) decides when execution advances.
Where an action has got to lives in assets, not in policy state: a worker
holding a job, travelling, busy until a tick or awaiting an outcome is a
balance, so every simulation resumes, replays and forks for free.

Agents, models, prompts and learning stay outside, in the Autonomy Harness
that [CONTINUOUS_AGENT_SYSTEMS.md](CONTINUOUS_AGENT_SYSTEMS.md) describes.
This plan only changes Axionomy's side of that boundary, and adds as little
as it can: each step reuses what exists.

## What exists

| Need | Already there |
| --- | --- |
| Possible actions | Rates, `Economy::applicable`, `ActionSource` |
| Whether an action applies, and why not | `Economy::assess`, with its shortfalls |
| What an actor may see | Actor views (`agent_view`, `responder_view`), ISMCTS information states |
| Choosing | Plain Rust: `Option::or_else` for priorities, `Iterator::min_by_key` for ranking by an ordered key, ties broken by id |
| Stepping, in memory or recorded | `rollout::run` over `&mut impl Apply`; `Episodes` for repeated experiments, with `Unrecorded` or a `GroupRecorder` |
| Bounded computation | Search sessions: `advance(WorkBudget)`, interrupted and resumed |
| Several exchanges, all or nothing | `Economy::replay` and `replayed`, Lean's `Execution.replay`, MCP's `axionomy_trace_replay` |

Ordered keys, not floating-point scores, rank choices: today's policies rank
by tuples such as `(time + travel, urgency, u64::MAX - value)`, and a scalar
utility is the special case of a one-element key.

## Names

Each term means one thing. Axionomy's machinery prefers words a user's
ontology is unlikely to need, but an occasional overlap with a domain word is
fine (problems call their state a world; search has sessions); what matters
is that overlaps stay few and no Axionomy term means two things.

| Term | Means |
| --- | --- |
| Economy (`World` in a problem) | The authoritative state; the only thing that changes |
| Exchange | One firing of a rate |
| Proposal | One exchange offered for applying; never a trace |
| Trace | Exchanges in order |
| Replay | Applying a trace, all of it or none of it: `replayed` gives the economy after it, `replay` applies it in place |
| Run | A trajectory: a trace from a state, kept in memory or recorded in the store |
| Rollout | A run a policy drives, to a goal, a stop or a horizon |
| Episode | One rollout of a repeated experiment, such as one Monte Carlo sample |
| Group | The runs produced together, such as one Monte Carlo estimate's episodes |
| Policy | How an actor chooses; a problem's `Policy` enum names its policies |
| Decision | One choice: a problem's `decision(view, actor, policy)` |
| Thinking | What a policy does when it simulates before it chooses; a study keeps its search's observations as each document's `thinking` |
| Nature response | What happens without anyone choosing: an outcome resolves, a vehicle arrives, a deadline expires; a problem's `nature_response(world)` |
| Tick | The clock advancing |
| Horizon | The most steps a rollout may take |
| Study | One request to Studio, MCP or the CLI: a problem, an instance, strategies, a seed and a budget. A study produces runs and the evidence of how they were found |
| Path | An identifier's one written form, such as `edge/key-room/gate`, and its order. Typed identifiers derive `Identifier`, so they are written and ordered as their paths, and a typed run written down reads as a `paths` run that MCP can explore (D-044) |

A study is not called a job, session, experiment or scenario because each
already means something else here: work-league's workers claim jobs, search
has sessions, Monte Carlo runs experiments, and Nature encodes scenarios.

## Step 1: simulations run through `rollout::run` (done)

Five simulations hand-wrote their loop. Three shared one shape, one proposal
per step taken from a priority order:

```rust
// logistics::fleet, mission::expedition and rescue::incident, as controllers
let exchange = nature_response(world)
    .or_else(|| close_if_applicable(world))
    .or_else(|| actors.find_map(|actor| decision(&view(world, actor), actor, policy)))
    .unwrap_or_else(|| tick(world));
```

That body is now a rollout controller, and the goal its terminal condition:
`fleet::run`, `expedition::run` and `incident::run` take
`&mut impl Apply` and return the rollout's result, so a horizon or a
rejection reaches the service as an error instead of a panic, and every
Studio replay stayed byte for byte the same. `work_league`'s continuous
season (due events, then every worker once in a rotation, then the tick) and
`cold_chain`'s day plan keep their own loops until a second problem shares
their shape.

Names became consistent on the way: `response` and
`required_nature_response` are `nature_response`, and the six
`type Proposal = Trace<Ontology>` aliases are gone, since a proposal is one
exchange.

## Step 2: studies record their runs (built for MCP)

A study records each document's trajectory as a store run and rebuilds its
frames from the stored steps when they are read, instead of keeping the whole
view as JSON. What is not a trajectory (Pareto fronts, telemetry, Monte Carlo
estimates, search observations) is small and stays with the study.

- MCP keeps runs in its store instead of whole studies as JSON.
- The Studio server opens a store (in memory unless a file is given, as MCP
  does), so a study can be reopened and scrubbed from stored roots.
- GitHub Pages keeps its pre-built replays: it has no server.

This step does not depend on step 1: every study already builds its documents
from an initial state and a trace, so it records them the same way.

Done when MCP's store holds no view JSON and the Studio server reopens a
recorded study.

Built for MCP, which needed nothing new in the store for it:

- `StudyObserver::trajectory` reports each document's initial economy and
  trace written as paths (D-044), from the shared helpers every problem
  builds its documents with. Studio and the browser build ignore it, and
  their replays are byte for byte what they were.
- MCP records each document as two economies with the code that records
  every other: its initial state as a run of its own, like an authored
  economy, and its trace as a run forked from it, one step per exchange, as
  `axionomy_trace_replay` would. A study's summary lists both handles for
  every document, so a client steps into any state of a study, tries another
  exchange there, and diffs it against the study's next step.
- MCP's store keeps a study's key and its documents' handles, not its views.
  Rather than rebuilding frames from stored steps, a study missing from memory
  is computed again from its key, which is deterministic, and its documents
  are served only if they replay the trajectories the study recorded; a study
  recorded by another version is refused and studied again. Work-league's
  showcase study took a 135 MB store file as JSON and takes 8.4 MB as runs.

The Studio server's store and reopening a recorded study remain.

## Step 3: studies are named studies (done)

Studio's and MCP's computation is named a study, in one commit with the Python
interop suite. The study runtime computes a `Study` from a `StudyRequest`, keyed by
its `StudyKey` and controlled and observed through `StudyControl` and
`StudyObserver`. The Studio server serves `/api/studies`, mints
`StudioStudyId`s (`study-7`) and streams `study_started` … `study_failed`
events; MCP's `axionomy_problem_study` returns a `StudySummary` whose `stu-`
handle is read under `axionomy://study/{study_id}`; the CLI's command is
`axionomy study`. "Run" means a trajectory everywhere.

## Step 4: replay is all or nothing

`replayed` kept nothing when an exchange failed, but `replay` kept the
exchanges before the failure, so two names that read as one operation, in
place and on a copy, meant different things. Both now apply a whole trace or
none of it:

- A failed `replay` leaves the economy unchanged, and `ReplayError` names the
  exchange that failed, counted from zero.
- Lean's `Execution.replay` states it; `replay_rejection_atomic`,
  `replay_success` and `replay_error_index` prove it, and the conformance
  fixtures are regenerated from Lean.
- Several exchanges committed together are a trace given to `replay`. `Apply`
  gains `replay` when a recorder first needs to commit several exchanges at
  once.

A commitment that fits one rate stays one rate: a rate is already an atomic
transition across accounts.

## Step 5: outside work, as one example

Work done outside the economy (a robot, a model, a person) is modelled, not
orchestrated:

- Asking is an exchange that produces a request asset, such as
  `Requested(id)`.
- The result is a rate that consumes it, so a duplicate report is infeasible
  and the kernel rejects it.
- A timeout is a Nature response that turns it into `Lapsed(id)`: the outcome
  is unknown, not failed.
- A crash between asking and doing loses nothing: the request is in the
  state, and recording made the state durable.

`test_outside_work_is_a_request_the_economy_holds` in
`crates/axionomy-mcp/interop/test_mcp.py` shows the pattern with a stand-in
for the outside party, through MCP's existing tools: a duplicate report and a
report after the lapse are refused, and the lapse cannot fire before its
deadline. MCP needed nothing new for it.

## Not planned

Each of these waits for a problem or a client that needs it:

- A separate behavior crate, or scorer, picker, thinker and action types.
- Floating-point utility scores.
- Action state machines and behavior checkpoints.
- Typed behavior identifiers, versioned definitions, or definitions as data.
- Durable workflow, retry or compensation machinery.
- MCP operations for behaviors.
- MuJoCo or language-model executors; those belong to the Autonomy Harness.
