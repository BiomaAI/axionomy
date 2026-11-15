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

The fleet's rates have variables: one law serves every van, order and road,
`load/{van}/{order}/hub`, and laws split only where their shape differs,
loading at the depot or the hub, delivering on time or late, each trip
outcome, repairing a broken-down or a worn van. A trip goes along a road, a
relation over the map's directed roads; the fuel it burns and whether it
takes the trunk road's one lane are `Road(from, to)` and `Trunk(from, to)`
facts of the world's map. An order's payload space, refrigerated slot,
staging space and payment come from its own account's weight, cold-cargo
flag and price, a delivery is made where the van stands at the order's
destination, and an order expires at its `Expiry`. Each role is fixed to its
account, the world, the station, Nature, the van or the order its
identifier names, so no identity token pins a binding. Nature takes, order by
order, a release or an expiry, then, van by van, a trip's resolution or
arrival or a service's end (`state.moves`); a van's decision is its rate's
exchange (`state.exchange_of`).

One fuel pump, one repair bay and one tow truck remain occupied for their whole
service duration. Fuel and repair parts are finite, maintenance clears actual
wear, and breakdown recovery consumes time and operating funds. The package's
three fleet policies run identical initial orders and route tickets; their
comparison is an evaluated policy frontier. Micro keeps the original
stochastic model, which the package's planner replans by Monte Carlo tree
search. The model's tests apply exchanges directly: cargo compatibility, trunk
occupancy, paid service occupancy, handoff conservation, and a rate catalog
that does not disclose unrealized weather tickets.

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

Nature draws each resolution by its event, `weather/<route>/<elapsed time>`:
the weather of that leg, met at the time the vehicle has elapsed when the
leg begins or, after a repair, which takes time, when it resumes. A run
seeded alike that meets the same leg at the same time meets the same weather,
whichever policy brought it there.


## Required results

The package's experiments find these, pinned in `tests/outcomes`:

- In Micro, with one order left, the direct policy delivers in five exchanges
  and two time units; the reliable policy and the planner both deliver in ten
  exchanges and four time units.
- Every fleet policy closes Showcase and Stress.

The model's tests check its rules: a delivery through a breakdown replays with
its weather as explicit Nature exchanges, a broken-down vehicle arrives only
after its repair and a fresh draw, failed travel changes nothing, and a
package loads only from its own order. Fuel, money, time, cargo,
order-lifecycle and position invariants are declared, so every exchange keeps
them.

## API pressure

- Long-horizon stochastic trajectories rather than one-step decisions.
- Recurrent Nature outcomes and retry loops.
- Disposable MCTS route planning over the same transition system.
- Efficient forks sharing immutable laws and untouched account data.
- Candidate generation from current route, custody, and resource assets.

## In this folder

- `mod.rs` is the model: the Micro vehicle, its routes and Nature.
- `fleet/` is the Showcase and Stress fleet, an ontology of its own laid out
  the same way: its model in `mod.rs`, and its scripts `scripts/fleet.rhai`
  and `scripts/fleet-view.rhai`.
- `package.rs` writes its package: its economies under refs, goals,
  strategies, experiments and view, whose scripts are `scripts/logistics.rhai`
  and `scripts/logistics-view.rhai`.

Logistics is one of the [conformance problems](https://github.com/BiomaAI/axionomy/blob/main/PROBLEMS.md).
