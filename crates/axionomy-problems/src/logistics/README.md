# Stochastic delivery logistics

Long-horizon stochastic logistics over core-validated rollouts.

Showcase and Stress use `fleet/`: a two-slot refrigerated van and
a three-slot freight van serve three customers. Four/six arriving orders include
one-slot cold goods, three-slot heavy loads and transferable standard freight.
Cold loads expire after their window; late standard/heavy deliveries earn half
payment. All customer payments and service fees move existing money.

Trips overlap on a common clock. The depot–hub trunk holds one vehicle until
arrival. Express travel consumes extra fuel and can break down; Nature outcomes
are fixed by seed, vehicle, trip number and directed road, independently of policy
evaluation order. Arrival time is based on the recorded departure time, including
when an environmental response is applied later. The dispatcher never selects
the weather result. A finite two-slot hub supports asynchronous cargo handoffs;
direct transfers require actual co-location and free compatible capacity.

One fuel pump, one repair bay and one tow truck remain occupied for their whole
service duration. Fuel and repair parts are finite, maintenance clears actual
wear, and breakdown recovery consumes time and operating funds. Three responsive
fleet policies run identical initial orders and route tickets; the larger-profile
comparison is an evaluated policy frontier. Micro retains the original stochastic
model and full repeated MCTS execution. Tests cover 24 fleet/scenario combinations,
replay, cargo compatibility, paid service occupancy, handoff conservation, and
dispatch independence from unrealized weather tickets.

## Specification

One vehicle must deliver four identified packages. Orders encode waiting,
in-transit, and delivered state. The vehicle encodes its location, cargo
capacity, package custody, fuel, money, repair tool, elapsed time, and
remaining deadline. A fuel station holds replenishment stock.

Two route policies are available:

- Direct routes use fewer exchanges but have encoded clear, delay, and
  breakdown weights of 2:1:1.
- Reliable routes use two legs each way with weights 8:1:1 per leg.

Departure converts an encoded location into a traveling-route state. Nature
then resolves one weighted outcome exchange. Clear or delayed arrival consumes
route-specific fuel and time; breakdown consumes time through a repair
exchange and returns the route to Nature for another resolution. Loading,
delivery, refueling, travel, repair, and order completion are all rates.

## Required results

- A reliable-policy trajectory delivers all four orders and exceeds forty
  exchanges.
- Every sampled travel outcome appears as a Nature exchange.
- The complete long trajectory replays from the initial economy.
- Fuel, money, time, cargo, order lifecycle, and position invariants hold.
- Generic Monte Carlo compares both policies over 64 seeded rollouts.
- Encoded reliability makes the reliable policy's mean utility higher.
- Mean and lower-decile selection both use caller-chosen projections of the
  same sampled encoded outcomes.
- MCTS repeatedly selects and executes actions through deliveries and
  disruptions, retaining each searched decision at its actual trace index.
  Completion, no feasible action, and horizon exhaustion are distinct outcomes.
  Search randomness is independent of realized Nature sampling; cancellation
  discards the fork. Fleet coordination is not implemented yet.
- A vector Monte Carlo evaluation exposes completion probability, mean
  deliveries, and mean elapsed time as an explicitly approximate policy front.
- A repeatable workload reports rollouts and exchanges per second.

## API pressure

- Long-horizon stochastic trajectories rather than one-step decisions.
- Recurrent Nature outcomes and retry loops.
- Outcome distributions, mean values, and risk statistics.
- Disposable MCTS route planning over the same transition system.
- Efficient forks sharing immutable laws and untouched account data.
- Candidate generation from current route, custody, and resource assets.
- Sampled dominance may remove a policy while preserving its underlying
  encoded rollouts for audit and alternative statistics.

## In this folder

- `mod.rs` is the model: the Micro vehicle and its routes, Nature, rollouts,
  Monte Carlo and MCTS planning.
- `fleet/` is the Showcase and Stress fleet, an ontology of its own laid out
  the same way: its model in `mod.rs`, and its scripts `scripts/fleet.rhai`
  and `scripts/fleet-view.rhai`.
- `package.rs` writes its package: its economies under refs, goals,
  strategies, experiments and view, whose scripts are `scripts/logistics.rhai`
  and `scripts/logistics-view.rhai`.

[`examples/logistics.rs`](https://github.com/BiomaAI/axionomy/blob/main/crates/axionomy-problems/examples/logistics.rs) runs it, and the
`rollout_throughput` bench measures its long-horizon workload:

```console
cargo run -p axionomy-problems --example logistics
cargo bench -p axionomy-problems --bench rollout_throughput
```

Logistics is one of the [conformance problems](https://github.com/BiomaAI/axionomy/blob/main/PROBLEMS.md).
