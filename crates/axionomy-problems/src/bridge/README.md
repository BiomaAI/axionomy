# Single-lane bridge negotiation

Multi-agent negotiation for a capacity-one bridge.

Showcase and Stress use `timed.rs`: a shared bounded clock, staggered
arrivals, deadlines, a two-tick reservation lease, and crossings that hold the
lane for two or three ticks. A paid three-tick detour can overlap bridge traffic.
Later traffic waves change deadline pressure while retaining credit. Fixed
priority, slack-responsive auctions, and adaptive detours run against identical
traffic. Completion records on-time and late trips; a late trip still completes.
Larger comparison fronts cover evaluated policies, not all possible mechanisms.
The routing BFS resolves ready completions and excludes bidding/lease-expiry
branches. The compact direct-settlement semantics below describe Micro.

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

Every participant and the bridge preserve identity assets. Agent names in
rate identifiers therefore cannot be used to impersonate another account or
swap the encoded auction winner. Finish consumes the bridge's active lifecycle
asset, making the terminal economy quiescent.

## Required results

- First-come and auction proposals both replay to the same goal.
- A bid of A=2 and B=1 resolves to A.
- A second claim while capacity is held is rejected.
- Bidder impersonation and winner/loser rebinding are rejected.
- Rejection leaves every account unchanged.
- Exact Pareto search retains both possible priority allocations with both
  agents' credits intact; auction outcomes with the same priority but spent
  credit are correctly dominated under the encoded objectives.

## API pressure

- Multi-agent proposals and atomic joint resolution.
- Escrow and resource accounting.
- Alternative mechanisms over one state vocabulary.
- Capacity as a conserved asset.
- Dominance can reject an economically wasteful mechanism without rejecting
  its core validity.

## In this folder

- `mod.rs` is the model: the Micro bridge, its first-come and auction
  mechanisms, and the searches and proposals over every instance.
- `timed.rs` builds the Showcase and Stress traffic: the shared clock,
  leases, occupied crossings, deadlines and the paid detour.
- `study.rs` is `Bridge`, the bridge's `axionomy_study::Problem`: its catalog
  entry, strategies and the documents a study holds, each derived by replay.
- `view.rs` presents it: labels, units, scene and leaderboards.

[`examples/bridge.rs`](https://github.com/BiomaAI/axionomy/blob/main/crates/axionomy-problems/examples/bridge.rs) runs it:

```console
cargo run -p axionomy-problems --example bridge
```

The bridge is one of the [conformance problems](https://github.com/BiomaAI/axionomy/blob/main/PROBLEMS.md).
