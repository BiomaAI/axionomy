# Autonomous Work League

A competitive multi-agent work league encoded entirely as economic state.

Agents contend for a finite pool of jobs. Every claim, movement, attempt,
chance resolution, repair, recharge, and recycling operation is an atomic
exchange. Policies only propose those exchanges; they cannot bypass the
economy or manufacture a score.

## Specification

Micro retains the compact serial correctness fixture. Showcase and Stress use
`continuous.rs`: four workers share a world clock and make one
joint or individual decision per tick. Work, repair, charging, and recycling
can overlap; busy workers cannot move or start another activity. Each facility
has one identity-checked occupied slot, released only when its service is due.

Twelve Showcase contracts arrive in three waves; Stress has six waves and 24
contracts. Claims are exclusive, expire after six unstarted ticks, and can be
released voluntarily. Dependent contracts require another job's encoded
completion. Work must fit a delivery deadline. Expiry releases any claim and
activity without paying for late output. The season goal means every contract
has resolved and every activity has settled; it does not assert every job was
delivered. Completed and expired counts remain separately visible.

Rush, Lean, and Safe modes trade time, materials, energy, wear, and failure
risk. Workers reconsider modes and recovery from their latest state. Repairs
and charging spend earned settlement credit. Recycling converts two waste
units into one usable material unit and one explicit loss. A joint handoff
moves a pending claim and two credits atomically, spends both workers' current
decision permissions, and can occur only once per contract.

All policy fields use the same scenario seed. Nature tickets are keyed by job
and attempt, independently of worker, mode, or proposal ordering. Modes apply
their encoded failure weights to that common latent ticket. Rotating the first
worker each tick reduces fixed-seat access advantages. Role/policy permutations
and broader held-out evaluation remain future benchmark work.

The Showcase match produces at least sixty replay-verified exchanges. At every
snapshot, the presentation ontology independently derives six standings:

- Contract value.
- Jobs completed per elapsed tick.
- Value per energy-plus-material unit.
- Least residual waste, with no-work agents ineligible.
- Successful jobs per attempt.
- Non-dominance over value, completions, waste, and elapsed time.

Those rankings are intentionally allowed to disagree. Earned value, elapsed
time, resource expenditure, waste, attempts, and completions are authoritative
assets because they affect and result from mechanics. Rank, eligibility,
trend, and Pareto membership are disposable comparisons over the selected
replayed snapshot.

## Required results

- Showcase contains four agents, twelve finite jobs, eight locations, and at
  least fifty accepted exchanges; Stress doubles the job pool and materially
  lengthens the replay.
- Each contract has at most one live claim and ends completed or expired.
  Early outcomes, occupied service bays, duplicate handoffs, and late payments
  are rejected without mutation. Worker time advances on the shared clock.
- Agent and job identity assets reject rebinding a valid-looking claim to the
  wrong participant.
- Failure, repair, retry, recycling, and shared-facility effects replay through
  ordinary atomic exchanges.
- Energy, material, time, worker condition, and repair supply invariants hold
  across the complete match.
- Every snapshot carries all six leaderboards, and at least two different
  agents lead the final set of objectives.

## API pressure

- Competitive multi-agent allocation without a privileged scheduler.
- Several honest definitions of winning instead of one universal utility.
- Eligibility, exact ratios, ties, rank changes, and Pareto comparison as a
  generic view contract.
- Live frame publication without moving transport state into the economy.
- Shared URLs that restore a precise problem, outcome, replay step, and
  leaderboard on both a server and static GitHub Pages.
- Concrete-rate expansion pressure that keeps the new exact scalar
  parameter/law schema intentionally narrower than a general callback system.

## In this folder

- `mod.rs` is the model: the league's workers, jobs, locations, facilities
  and policies, the Micro serial fixture, and the runner.
- `continuous.rs` is the Showcase and Stress season on the shared clock:
  leases, deadlines, activity durations and facility occupancy as laws.
- `package.rs` writes its package: its economies under refs, goals,
  strategies, experiments and view, whose scripts are `scripts/work-league.rhai`
  and `scripts/work-league-view.rhai`.

[`examples/work_league.rs`](https://github.com/BiomaAI/axionomy/blob/main/crates/axionomy-problems/examples/work_league.rs) runs it:

```console
cargo run -p axionomy-problems --example work_league
```

The Work League is one of the [conformance problems](https://github.com/BiomaAI/axionomy/blob/main/PROBLEMS.md).
