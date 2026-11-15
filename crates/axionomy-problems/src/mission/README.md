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

The expedition's rates have variables: one law serves every objective, site
and pair of places, `share/{objective}/{location}/true`, and laws split only
where their shape differs: each teammate's journeys, arrivals and
treatment, since each plays a role of its own, sharing by radio or in
person, each phase and cause of a loss, a clinical pulse that harms or
passes over. A journey takes the road distance between two places: the
world's map is the departure's relation over every two places, giving how
many roads apart they are, and the policies read the same map
(`state.values`). A pulse falls on the days a relation lists, and a report
advances the scan to the round it derives. One law steps the clock,
`tick/{day}`, its relation over the days giving whether the weather closes
in or clears that day, and one ends a pulse, `finish-clinical/{day}`, its
relation giving how many days on the next pulse falls, the last scheduling
none. A delivery and a loss in transit are each objective's own, since an
archive's carrier and a casualty's differ. The mission's horizon, how many
objectives it has and the deadline every objective shares are the whole
expedition's and never change: constants, `{horizon}`, `{objective-count}`
and `{deadline}`, which the laws and the policies read by name
(`state.constant`). Each role is fixed to its account, so no identity token
pins a binding. Nature takes the weather or a pulse's end, then, objective
by objective, a decline, a loss, finished work or a discovery, then each
teammate's arrival and a report (`state.moves`);
a teammate's decision is its rate's exchange (`state.exchange_of`).

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

Scout and Medic accounts begin at base while Nature holds the unresolved
truth, sensor seed, and hazard state, which it draws by weights its law
gives. Neither agent view can inspect Nature or the other agent's private
account.

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

Micro's rates have variables: `instantiate/{truth}/{seed}/{hazard}` draws a
scenario, a relation over the three giving the weight Nature draws it by,
`{weight}`, which never changes and so is law, not Nature's to hold;
`resolve-scan/{truth}/{seed}/{report}` answers a scan over a relation of
what the sensor reports of each site at each seed, and derives the seed
after it (`{next}`, the first again after the last), and sharing,
moving, the encounter and the rescue serve each site. Each role is fixed to
its account, so no identity token pins a binding. Nature draws among the
scenarios `state.moves` offers, by their weights (`state.values`), and
answers with the response it offers.

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
