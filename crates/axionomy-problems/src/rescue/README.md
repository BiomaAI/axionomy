# Stochastic rescue

A partially observed rescue decision with an explicitly encoded Nature.

Showcase and Stress use `incident/`: three/four hidden casualties across
four/five sectors, a bounded shared incident clock, clinical decline, hard
evacuation deadlines, two noisy sensor reports, finite medicine and base
resupply, and a single evacuation seat. Search reveals a patient; a hint alone
does not. Stabilization restores vitality and delays decline, but consumes
time while other patients worsen. Emergency driving uses twice the fuel per
road hop. Patients continue to deteriorate during transport and only count as
saved after hospital admission. Closing an incident records every outcome,
including deaths and people left behind; closure is not universal success.

The incident's rates have variables: one law serves every patient and site,
`stabilize/{patient}/{location}/true`, and laws split only where their shape
differs, restoring vitality or not, each cause and phase of a loss. One law
drives between any two places, fast or not, `drive/{from}/{to}/{fast}`: the
world's map is its relation over every two places, giving how many roads
apart they are, and another gives each pace's ticks and fuel per road, from
which it derives the drive's `{time}` and `{fuel}`. The policies read the
same map (`state.values`), and the view finds the sites and roads among the
rates' instances (`state.instances`). Each role is fixed to its account,
the world, the crew, Nature or the patient its identifier names, so no
identity token pins a binding. Nature takes, patient by patient, the first
loss or decline that applies, then a discovery or a report
(`state.moves`); the responder's choice is its rate's exchange
(`state.exchange_of`).

The package's three incident policies play the same seeded hidden scenario
and read only the responder's account, public incident state and discovered
patients. Their outcome comparison is approximate, not an estimated success
probability. The model's tests apply exchanges directly: hidden scenarios
differ only where the responder cannot see, and clinical updates, finite
treatment and the one seat hold. The Micro specification follows.

## Specification

The agent begins at base with energy and one sensor. An unresolved Nature
account owns `Unresolved` and a user-provided
`ScenarioWeight(truth,seed)` prior. A sampling exchange preserves the selected
weight, consumes `Unresolved`, and produces private `Truth(North|South)` and
`Seed(0..3)` assets. The policy first proposes a truth-independent public
`BeginObserve` exchange, which consumes `Planning` and the sensor and produces
`AwaitingObservation`. Nature then fires the uniquely applicable hidden
`ResolveObservation` exchange, which:

- Preserves the agent at base.
- Consumes `AwaitingObservation` and restores `Planning`.
- Preserves Nature's matching truth.
- Advances the seed.
- Produces an agent belief.

Seed zero reports the wrong location; the other three seeds report correctly.
A rescue exchange succeeds only when the agent's location and Nature's truth
match.

Micro's rates have variables: `instantiate/{truth}/{seed}` draws a scenario,
`resolve-observation/{truth}/{seed}/{report}` answers an observation over a
relation of what the sensor reports of each site at each seed, and derives
the seed after it (`{next}`, the first again after the last), and
`move/base/{to}`, `rescue/{location}` and `evacuate/{location}` go to each
site. Each role is fixed to the agent's or Nature's account. Nature draws
among the scenarios `state.moves` offers, and answers with the resolution
it offers. A reading may point where the agent has no road, as seed zero
does of a survivor south in Micro; the move it proposes names no rate, and
is refused.

## Required results

The package's experiments find these, pinned in `tests/outcomes`:

- In Micro's scenario, the survivor south and the sensor's seed 1,
  observe-then-follow rescues with the sensor and two energy; direct north is
  refused.
- Over 128 scenarios drawn from the prior, each by the event `scenario` with
  its own seed, observe-then-follow rescues 97 times and direct north 63.
- Every incident policy closes Showcase and Stress.

The model's tests check its rules: Nature resolves an observation only after
the public intent, the actor and Nature cannot swap roles, the agent holds
the same whatever the truth, and a sampled rescue replays from the weighted
model.

## API pressure

- Restricted observation without a second state model.
- Hidden truth and belief as different account holdings.
- Reproducible priors and chance through an encoded Nature participant.
- Seeded Monte Carlo over the encoded prior.
- Policies compared over identical encoded scenarios rather than independent
  stochastic runs.
- Rates with variables whose accounts their identifiers name, and a derived
  value (`report/{round}/{location}` advances to round `{next}`).
- Relations that give numbers: the incident's map gives each drive's road
  distance and its pace the ticks and fuel per road, never balances an
  exchange could change.

## In this folder

- `mod.rs` is the model: the Micro rescue and its Nature.
- `incident/` is the Showcase and Stress incident, an ontology of its own
  laid out the same way: its model in `mod.rs`, and its scripts
  `scripts/incident.rhai` and `scripts/incident-view.rhai`.
- `package.rs` writes its package: its economies under refs, goals,
  strategies, experiments and view, whose scripts are `scripts/rescue.rhai`
  and `scripts/rescue-view.rhai`.

The rescue is one of the [conformance problems](https://github.com/BiomaAI/axionomy/blob/main/PROBLEMS.md).
