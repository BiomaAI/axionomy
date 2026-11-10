# Simulation: policies, runs and studies

Status: planned, in the order the steps below give. Step 0, rollouts and
Monte Carlo episodes recording where they run, is built.

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
| Several exchanges, all or nothing | `Economy::replayed`, Lean's `Replay.run`, MCP's `axionomy_trace_replay` |

Ordered keys, not floating-point scores, rank choices: today's policies rank
by tuples such as `(time + travel, urgency, u64::MAX - value)`, and a scalar
utility is the special case of a one-element key.

## Names

Each term means one thing. Where a word is already a domain word inside a
problem (work-league's jobs and contracts, logistics' and marketplace's
orders, amm's obligations, marketplace's and bridge's settlement), Axionomy
does not reuse it for its own machinery.

| Term | Means |
| --- | --- |
| Economy (`World` in a problem) | The authoritative state; the only thing that changes |
| Exchange | One firing of a rate |
| Proposal | One exchange offered for applying; never a trace |
| Trace | Exchanges in order |
| Replay | Applying a trace, all of it or none of it: `replayed` gives the economy after it, `replay` applies it in place (step 4 makes `replay` match) |
| Run | A trajectory: a trace from a state, kept in memory or recorded in the store |
| Rollout | A run a policy drives, to a goal, a stop or a horizon |
| Episode | One rollout of a repeated experiment, such as one Monte Carlo sample |
| Group | The runs produced together, such as one Monte Carlo estimate's episodes |
| Policy | How an actor chooses; a problem's `Policy` enum names its policies |
| Decision | One choice: a problem's `decision(view, actor, policy)` |
| Nature response | What happens without anyone choosing: an outcome resolves, a vehicle arrives, a deadline expires; a problem's `nature_response(world)` |
| Tick | The clock advancing |
| Horizon | The most steps a rollout may take |
| Study | One request to Studio or MCP: a problem, an instance, strategies, a seed and a budget. A study produces runs and the evidence of how they were found |

"Job" is not used for studies: work-league's workers claim jobs, and the
store's writer runs jobs. "Session" belongs to search, "experiment" to Monte
Carlo, and "scenario" to Nature's encoded scenarios.

## Step 1: simulations run through `rollout::run`

Five simulations hand-write their loop. Three share one shape, one proposal
per step taken from a priority order:

```rust
// the shape of logistics::fleet, mission::expedition and rescue::incident
let exchange = nature_response(&world)
    .or_else(|| close_if_applicable(&world))
    .or_else(|| actors.find_map(|actor| decision(&view(&world, actor), actor, policy)))
    .unwrap_or_else(|| tick(&world));
```

That body is a rollout controller, and the goal is its terminal condition.
These three move onto `rollout::run`, which already applies, stops at the
goal and reports a horizon or a rejection as a result instead of a panic.
`work_league`'s continuous season (due events, then every worker once in a
rotation, then the tick) and `cold_chain`'s day plan step `&mut impl Apply`
so they can be recorded, and keep their own loops until a second problem
shares their shape.

Names become consistent on the way: `response` and
`required_nature_response` become `nature_response`, and the six
`type Proposal = Trace<Ontology>` aliases go, since a proposal is one
exchange.

Done when every Studio replay is byte for byte the same, the three loops are
gone, and their horizons and rejections reach the service as errors.

## Step 2: studies record their runs

A study records each document's trajectory as a store run and rebuilds its
frames from the stored steps when they are read, instead of keeping the whole
view as JSON. What is not a trajectory (Pareto fronts, telemetry, Monte Carlo
estimates, search observations) is small and stays with the study.

- MCP keeps runs in its store instead of artifact JSON.
- The Studio server opens a store (in memory unless a file is given, as MCP
  does), so a study can be reopened and scrubbed from stored roots.
- GitHub Pages keeps its pre-built replays: it has no server.

This step does not depend on step 1: every study already builds its documents
from an initial state and a trace, so it records them the same way.

Done when MCP's store holds no view JSON and the Studio server reopens a
recorded study.

## Step 3: studies are named studies

Studio's and MCP's computation is renamed together, in one commit, with the
Python interop suite: `/api/runs` becomes `/api/studies`, `StudioRunId`
(`run-7`) becomes `StudyId` (`study-7`), `RunRequest`, `RunSummary`,
`RunControl` and `RunObserver` become `Study…`, `RunArtifact` becomes the
study's record, and `axionomy_problem_run` becomes `axionomy_problem_study`.
After it, "run" means a trajectory everywhere.

## Step 4: replay is all or nothing

Today `replayed` keeps nothing when an exchange fails, but `replay` keeps the
exchanges before the failure. The pair reads as one operation, in place and
on a copy, and should be one. No caller depends on the kept prefix; only the
specification and its tests state it.

- `replay` leaves the economy unchanged when any exchange fails, and the
  error names the exchange that failed.
- The Lean model states and proves it, replacing the prefix theorem, and the
  conformance fixtures are regenerated from Lean.
- `Apply` gains `replay`, so a recorder records a trace only when all of it
  applies. This is the all-or-nothing commit of several exchanges, under the
  name the kernel, the store and MCP already use.

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

One problem shows the pattern with a stand-in for the outside party, driven
through the existing MCP tools. Whether MCP needs anything new is decided
after that example, not before.

## Not planned

Each of these waits for a problem or a client that needs it:

- A separate behavior crate, or scorer, picker, thinker and action types.
- Floating-point utility scores.
- Action state machines and behavior checkpoints.
- Typed behavior identifiers, versioned definitions, or definitions as data.
- Durable workflow, retry or compensation machinery.
- MCP operations for behaviors.
- MuJoCo or language-model executors; those belong to the Autonomy Harness.
