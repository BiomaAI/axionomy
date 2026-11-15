# Discrete job-shop scheduling

A two-job, two-machine scheduling problem with discrete capacity assets.

Showcase and Stress use `execution.rs`. A queue reservation
does not complete work or unlock a successor. Starting requires an idle,
working machine and a free crew; changing the machine's job configuration
adds one tick. Every operation has a faster preferred machine and a slower
alternative. The shared clock cannot skip the scheduled Machine Two failure.
Interrupted work keeps its remaining duration, releases its crew, and resumes
only after a finite-kit repair finishes. Stress has one crew instead of two,
a later second job, and a longer repair. Timelines show actual occupancy,
including interruption, with future running completion shown as a projection.
The package's exact searches omit equivalent early queue reservations and
process ready completions before dispatch. The static slot fixture below
remains Micro.

Each operation's laws serve both machines it may run on,
`queue/one-a/{machine}`. How long each machine takes for the operation never
changes, so a start's relation gives it, `{processing-time}`, and the start
computes from it when the work is due; a start splits only by whether the
machine must switch its setup. When a job arrives never changes either: its
operations' queue laws' rows give it, `{arrival}`, the same on either
machine, and the dispatch reads it there (`state.values`). The tick Machine
Two fails at and how long a repair takes are the whole shop's, constants
`{fault-at}` and `{repair-duration}`. The clock advances by one law to any
tick, `advance/{to}`. Every role is fixed to the account its identifier
names.
Showcase has 45 rates as written, for 96 instances, and Stress 45, for 104.
The package's dispatch reads the machines an operation may run on from its
queue law's instances rather than keeping a table of its own.

## Specification

Two jobs each have two ordered operations:

```text
Job 1: M1 for 2 slots → M2 for 1 slot
Job 2: M1 for 2 slots → M2 for 1 slot
```

Every `(machine,time)` pair is an account initially holding `Available`.
Reserving an operation, `schedule/<operation>/{ready}/{start}`, consumes its
job's `ReadyAt(operation, ready)` token and the availability of its
machine's slots from `{start}`, each slot role fixed to the slot it names,
then produces `Reserved(operation)` and the successor's readiness or the
final completion token at the end it derives, `{end}`: the start plus the
operation's duration, a constant of its law. A relation keeps every start no
earlier than its ready time.

Finish, `finish/{job-one-end}/{job-two-end}/{makespan}`, preserves both
completion tokens and produces `Makespan(makespan)` and `Solved`; a relation
holds the makespan to the later end. Micro has 5 rates as written, for 56
instances.

## Required results

The package's experiments find these, pinned in `tests/outcomes`:

- Best-first search finds makespan 5.
- A separate branch-and-bound strategy also finds makespan 5.
- A horizon of two slots is infeasible for both.
- Exact Pareto search, ranked by either job, finds the two completion
  allocations `(3,5)` and `(5,3)`, making the shared machine's stakeholder
  tradeoff explicit.

The model's tests apply the rules directly:

- For horizons zero through five, the economy accepts exactly the schedules a
  direct job-shop brute-force oracle calls feasible, at the makespan it
  computes, the least being 5.
- Rebinding an operation to the wrong machine or time slot is rejected, and
  no reservation starts before its operation is ready.

## API pressure

- Atomic reservation of variable numbers of accounts.
- Precedence represented as tokens, not solver-only constraints.
- Optimization objective encoded in final state.
- Rates with variables across ready and start times, with a derived end and
  relations for what never changes.
- Stakeholder completion allocation without a privileged scalar compromise.

## In this folder

- `mod.rs` is the model: the Micro slot fixture and its infeasible horizon.
- `execution.rs` builds the executed Showcase and Stress job shops: queues,
  setup, shared crews, the machine failure and its repair.
- `package.rs` writes its package, the job shop's only implementation of its
  searches: its economies under refs, goals, strategies, experiments and
  view, whose scripts are `scripts/scheduling.rhai` and
  `scripts/scheduling-view.rhai`.

The job shop is one of the [conformance problems](https://github.com/BiomaAI/axionomy/blob/main/PROBLEMS.md).
