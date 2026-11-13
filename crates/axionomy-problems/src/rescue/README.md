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

Three responsive policies run the same seeded hidden scenario and read only
their own account, public incident state and discovered patients. Their
replayed outcome comparison is approximate, not an estimated success
probability. Tests cover 24 policy/scenario combinations, equal-observation
decisions, finite treatment, required clinical updates, one-seat capacity,
closed resource invariants and deterministic replay. The original Micro
Monte Carlo specification below remains intact.

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

- An agent view cannot inspect the Nature account.
- Equal agent views under different truths choose the same public observation
  intent.
- Public intent and Nature's chosen resolution are separate recorded exchanges.
- A successful sampled rollout, including Nature instantiation, replays from
  the unresolved weighted model.
- Across all eight encoded scenarios, observe-then-follow succeeds six times while
  a north-only policy succeeds four.
- Exact scenario evaluation selects observe-then-follow.
- A separate seeded Monte Carlo entry point draws randomly from the same
  encoded weighted prior and is reproducible.
- A sampled Pareto front retains the higher-success sensor policy and the
  sensor-free direct policy, and is explicitly labeled approximate.

## API pressure

- Restricted observation without a second state model.
- Hidden truth and belief as different account holdings.
- Reproducible priors and chance through an encoded Nature participant.
- Exact finite-support evaluation and genuine seeded Monte Carlo over isolated
  forks.
- Counterfactual policy comparison over identical encoded scenarios rather
  than independent stochastic runs.
- Approximate multi-objective estimates keep sampling uncertainty visible.

## In this folder

- `mod.rs` is the model: the Micro rescue, its Nature, policies, scenario
  evaluation and Monte Carlo.
- `incident/` is the Showcase and Stress incident, an ontology of its own
  laid out the same way: its model in `mod.rs`, and its scripts
  `scripts/incident.rhai` and `scripts/incident-view.rhai`.
- `package.rs` writes its package: its economies under refs, goals,
  strategies, experiments and view, whose scripts are `scripts/rescue.rhai`
  and `scripts/rescue-view.rhai`.

[`examples/rescue.rs`](https://github.com/BiomaAI/axionomy/blob/main/crates/axionomy-problems/examples/rescue.rs) runs it:

```console
cargo run -p axionomy-problems --example rescue
```

The rescue is one of the [conformance problems](https://github.com/BiomaAI/axionomy/blob/main/PROBLEMS.md).
