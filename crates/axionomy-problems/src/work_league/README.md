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
has one occupied slot, released only when its service is due.

The league's rates have variables: one law serves every worker, `{agent}`,
and every contract, `{job}`, such as `claim/{agent}/{job}` or
`move/{agent}/{from}/{to}`, whose roads are a relation. Laws split only where
their shape differs: each mode's begin, a finish's outcome, each facility's
service, and an expiry by what the contract was doing, `expire/{job}/none/none`,
`expire/{job}/some/{agent}/none` or `expire/{job}/some/{agent}/some/{mode}`.
A contract's terms never change, so they are law structure: a relation's
row for each contract gives where its work is done, `{place}`, a word a
begin writes into `at/{place}`, what it pays, `{reward}`, and the energy,
time and material its work takes, `{energy}`, `{time}` and `{material}`,
from which each mode's law computes its costs, and in a season when it
opens and its deadline, `{release}` and `{deadline}`. A begin names only
the worker, the contract and the mode, `begin/{agent}/{job}/{mode}`, and
the scripts read a contract's place from its terms (`state.values`). A
season's length is one for the whole world and never changes, so it is a
constant, `{season-length}`, which the tick and every activity read.
Nature's weight of each outcome of an attempt at a contract in a mode is a
relation's too, `{weight}`, and an outcome happens only with some weight,
so Nature holds no account. A wave's fourth contract
depends on its first, a value the claim derives from `{job}`. Every role is fixed to the account its identifier
names, so no identity token pins a binding. Micro has 11 rates as written,
for 246 instances; Showcase and Stress 23, for 1,030 and 1,954.

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

All policy fields use the same scenario seed. Nature draws each attempt's
ticket by its event, `attempt/<job>/<n>`, the nth attempt at the job, in
Micro and the seasons alike, independently of worker, mode, or proposal
ordering. Modes apply their encoded failure weights to that common latent
ticket. Rotating the first worker each tick reduces fixed-seat access
advantages. Role/policy permutations and broader held-out evaluation remain
future benchmark work.

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

The unit tests check the laws by applying exchanges:

- Each contract has at most one live claim; arrivals, dependencies, leases
  and deadlines are the economy's. Early outcomes, occupied service bays,
  duplicate handoffs, and late payments are rejected without mutation.
  Worker time advances on the shared clock, and travel follows the declared
  roads one hop a turn.
- Every role is fixed to the account its rate's identifier names, so a claim
  cannot be rebound to another participant, and work is done only where the
  contract is, at its mode's cost.
- Activities overlap; a joint handoff moves the claim and payment once and
  spends both workers' turns; recycling recovers material with a conserved
  loss.

The package's experiments find the rest, pinned in `tests/outcomes`: Micro's
serial match completes every job, and every field's Showcase and Stress
seasons end with each contract completed or expired, Stress materially
longer than Showcase. Failure, repair, retry, recycling, and shared-facility
effects replay through ordinary atomic exchanges, under every invariant.

## API pressure

- Competitive multi-agent allocation without a privileged scheduler.
- Several honest definitions of winning instead of one universal utility.
- Eligibility, exact ratios, ties, rank changes, and Pareto comparison as a
  generic view contract.
- Live frame publication without moving transport state into the economy.
- Shared URLs that restore a precise problem, outcome, replay step, and
  leaderboard on both a server and static GitHub Pages.
- Rates with variables over workers, contracts and modes, with an `Option`
  in their identifiers, a relation for the roads and a derived dependency,
  their per-contract places and numbers given by relations rather than
  written into thousands of concrete rates, and the season's length a
  constant rather than a balance.

## In this folder

- `mod.rs` is the model: the league's workers, jobs, locations, facilities
  and policies, and the Micro serial fixture.
- `continuous.rs` is the Showcase and Stress season on the shared clock:
  leases, deadlines, activity durations and facility occupancy as laws.
- `package.rs` writes its package: its economies under refs, goals,
  strategies, experiments and view, whose scripts are `scripts/work-league.rhai`
  and `scripts/work-league-view.rhai`.

The Work League is one of the [conformance problems](https://github.com/BiomaAI/axionomy/blob/main/PROBLEMS.md).
