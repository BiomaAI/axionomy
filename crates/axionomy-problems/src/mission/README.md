# Partially observed team mission

Partially observed, stochastic two-agent reconnaissance mission.

Showcase and Stress use `expedition/`. Scout and Medic move and work
independently on one bounded clock. Travel and recovery occupy each agent until
their recorded completion time. Scout privately discovers an archive and one
or two casualties. Confirmed information reaches Medic only through a charged
radio exchange or an actual in-person rendezvous. Sensor hints can be wrong.
Archive recovery takes three ticks; casualty extraction needs both teammates,
a stabilized patient and the Medic's single evacuation seat. Carried objectives
must reach base before their deadlines.

A seeded sector hazard closes access at tick five and clears at tick ten;
arrivals already in flight can cause injury that slows future travel. Scheduled
clinical pulses reduce casualty health and invalidate stabilization. The core
blocks further public work until those events settle. Medicine, energy, radio,
sensor charges and base resupply are finite conserved resources. Failures are
retained. The package's three policies share each scenario and see only their
own account, public team positions and explicitly learned objective accounts.
Studio exposes those views at every frame. Their outcomes compare evaluated
policies, not an exhaustive optimum or a success-probability estimate. The
Micro specification follows.

## Specification

Scout and Medic accounts begin at base while Nature holds unresolved weighted
truth, sensor seed, and hazard state. Neither agent view can inspect Nature or
the other agent's private account.

The Scout may begin a scan without naming hidden truth. That public exchange
creates `AwaitingScan`; Nature then resolves the scan through the uniquely
applicable truth-and-seed-specific exchange, producing fallible private
intelligence. A share exchange transfers that intelligence to the Medic while
preserving explicit evidence for the Scout. A scanned team can make the
coordinated move only when the Scout preserves shared intelligence and the
Medic preserves the received intelligence. A direct pre-scan commitment is a
separate rate that requires the Scout's still-unused sensor. At
the true site, Nature resolves the encoded safe or injury hazard. Injury
requires an atomic treatment exchange using the Medic's kit before the team
can rescue the victim. Every public mission action consumes encoded deadline
time except the final goal-marking exchange.

The package's planner is the Scout choosing every public action by
information-set tree search over what it sees, each iteration in a world its
observations since the prior leave possible; Nature answers the team. Tree
statistics merge by information state rather than hidden world state. Nature
resolution may inspect the sampled world, but only to propose concrete encoded
outcome exchanges. The selected live action is core-revalidated. Its fixed
policies are share-and-coordinate, which scans, shares and follows the Medic's
intelligence, and direct north, which sends both agents north without a scan.

## Required results

The package's experiments find these, pinned in `tests/outcomes`:

- In Micro's scenario, the victim north, the sensor's seed 0 (whose report is
  wrong) and the route safe, direct north rescues in four exchanges and the
  planner stops without a rescue.
- Over 16 scenarios drawn from the prior, each by the event `scenario` with
  its own seed, share-and-coordinate rescues 11 times and direct north 7.
- Every expedition policy closes Showcase and Stress.

The model's tests check its rules: the Scout holds the same before scanning
whatever the truth, the Medic follows intelligence only once it is shared, a
coordinated rescue replays exactly, and a scan's result cannot be bound to the
Medic.

## API pressure

- Agent-specific observations and information boundaries.
- Information-state identities and observation-safe tree transpositions.
- Belief sampling from encoded priors without giving policies hidden state.
- Public proposals followed by core applicability filtering.
- Multi-agent communication as economic state change.
- Hidden truth, belief, hazard, and deadline in one model.
- Seeded Monte Carlo over partially observed policies.

## In this folder

- `mod.rs` is the model: the Micro mission and its Nature.
- `expedition/` is the Showcase and Stress expedition, an ontology of its own
  laid out the same way: its model in `mod.rs`, and its scripts
  `scripts/expedition.rhai` and `scripts/expedition-view.rhai`.
- `package.rs` writes its package: its economies under refs, goals,
  strategies, experiments and view, whose scripts are `scripts/mission.rhai`
  and `scripts/mission-view.rhai`.

The mission is one of the [conformance problems](https://github.com/BiomaAI/axionomy/blob/main/PROBLEMS.md).
