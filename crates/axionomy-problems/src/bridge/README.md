# Single-lane bridge negotiation

Multi-agent negotiation for a capacity-one bridge.

Showcase and Stress use `timed.rs`: a shared bounded clock, staggered
arrivals, deadlines, a two-tick reservation lease, and crossings that hold the
lane for two or three ticks. A paid three-tick detour can overlap bridge traffic.
Later traffic waves change deadline pressure while retaining credit. The
package's fixed-priority, slack-responsive auction and adaptive-detour policies
run against identical traffic. Completion records on-time and late trips; a
late trip still completes. Larger comparison fronts cover evaluated policies,
not all possible mechanisms. The package's routing BFS resolves ready
completions and excludes bidding/lease-expiry branches. The compact
direct-settlement semantics below describe Micro.

## Specification

Two agents begin west of a bridge with energy, credits, and bidding status.
The bridge owns one `CapacityFree` token and an encoded first/second turn.

Two mechanisms are encoded:

- First-come consumes first-turn capacity and gives one agent
  `CrossingRight` plus `PriorityBenefit`.
- Auction submission escrows credits; one atomic resolution consumes both
  bids and capacity, charges the winner, refunds the loser, and produces
  `CrossingRight` plus `Waiting`.

Crossing returns the bridge's capacity token and enables the second turn. A
waiting or first-come agent can then receive the second right. Finish requires
both agents to hold `Crossed`.

The rates have variables: one law serves both travellers, `cross/{agent}`,
and a bid or a withdrawal of either amount, `submit-bid/{agent}/{amount}`.
A resolution is a law for each winner, `resolve/a/{winning-bid}/{losing-bid}`,
whose loser is the other traveller and whose bids a relation keeps to the
higher winning, A on a tie. Timed laws split only where their shape differs:
a completion by detour and lateness, a detour by whether the traveller was
waiting. A crossing holds the lane for the traveller's `CrossingTime`, a fact
of its account, and the clock advances by one law, `advance/{to}`. Every role
is fixed to the account its identifier names, so an agent's name in an
identifier cannot be used to impersonate another account or swap the auction
winner, and no identity asset pins a binding. Finish consumes the bridge's
active lifecycle asset, making the terminal economy quiescent. Micro has 8
rates as written, for 17 instances; Showcase and Stress 18, for 100.

## Required results

The package's experiments find these, pinned in `tests/outcomes`:

- First-come and auction policies both reach the goal.
- Exact Pareto search, ranked by either traveller, finds both possible
  priority allocations with both agents' credits intact; auction outcomes
  with the same priority but spent credit are dominated under the encoded
  objectives.

The model's tests apply the rules directly:

- First-come and auction plans both replay to the same goal.
- A bid of A=2 and B=1 resolves to A.
- A second claim while capacity is held is rejected.
- Bidder impersonation and winner/loser rebinding are rejected, and no
  identifier names a winner who bid less.
- Rejection leaves every account unchanged.

## API pressure

- Multi-agent proposals and atomic joint resolution.
- Escrow and resource accounting.
- Alternative mechanisms over one state vocabulary.
- Capacity as a conserved asset.
- Dominance can reject an economically wasteful mechanism without rejecting
  its core validity.

## In this folder

- `mod.rs` is the model: the Micro bridge and its first-come and auction
  mechanisms.
- `timed.rs` builds the Showcase and Stress traffic: the shared clock,
  leases, occupied crossings, deadlines and the paid detour.
- `package.rs` writes its package, the bridge's only implementation of its
  searches and policies: its economies under refs, goals, strategies,
  experiments and view, whose scripts are `scripts/bridge.rhai` and
  `scripts/bridge-view.rhai`.

The bridge is one of the [conformance problems](https://github.com/BiomaAI/axionomy/blob/main/PROBLEMS.md).
