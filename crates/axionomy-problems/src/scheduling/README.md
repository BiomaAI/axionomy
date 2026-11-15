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

## Specification

Two jobs each have two ordered operations:

```text
Job 1: M1 for 2 slots → M2 for 1 slot
Job 2: M1 for 2 slots → M2 for 1 slot
```

Every `(machine,time)` pair is an account preserving
`SlotIdentity(machine,time)` and initially holding `Available`.
Scheduling consumes a job's `ReadyAt` token and every required slot's
availability, then produces `Reserved(operation)` and the successor readiness
or final completion token.

Finish preserves both completion tokens and produces `Makespan(n)` and
`Solved`.

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
- Rebinding an operation to the wrong machine or time slot is rejected.

## API pressure

- Atomic reservation of variable numbers of accounts.
- Precedence represented as tokens, not solver-only constraints.
- Optimization objective encoded in final state.
- Concrete-rate explosion across ready/start times.
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
