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
differs, restoring vitality or not, each cause and phase of a loss, driving
fast or not. A drive between any two places costs their road distance,
which the world's map holds as `Distance(from, to)` facts; the policies read
the same map. Each role is fixed to its account, the world, the crew,
Nature or the patient its identifier names, so no identity token pins a
binding. Nature takes, patient by patient, the first loss or decline that
applies, then a discovery or a report (`state.moves`); the responder's
choice is its rate's exchange (`state.exchange_of`).

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

## In this folder

- `mod.rs` is the model: the Micro rescue and its Nature.
- `incident/` is the Showcase and Stress incident, an ontology of its own
  laid out the same way: its model in `mod.rs`, and its scripts
  `scripts/incident.rhai` and `scripts/incident-view.rhai`.
- `package.rs` writes its package: its economies under refs, goals,
  strategies, experiments and view, whose scripts are `scripts/rescue.rhai`
  and `scripts/rescue-view.rhai`.

The rescue is one of the [conformance problems](https://github.com/BiomaAI/axionomy/blob/main/PROBLEMS.md).
