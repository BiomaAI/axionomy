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
retained. Three policies share each scenario and receive only their own view,
public team positions and explicitly learned objective accounts. Studio exposes
those views at every frame. Their outcome frontier compares evaluated policies,
not an exhaustive optimum or a success-probability estimate. The Micro
information-set search fixture below remains available and tested.

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

The coordinated policy scans, shares, and follows the Medic's visible
intelligence. A direct policy always sends both agents north. Replayed traces
are also projected into observation, outcome, and termination transitions for
learning.

Observation-scoped ISMCTS receives only the Scout's canonical information
identity when generating decisions or choosing rollout actions. The caller
supplies complete encoded belief worlds; the planner retains only worlds
consistent with the live observation and samples among them. After a public
action and required Nature response, caller-owned belief worlds are advanced,
filtered to the new observation, and passed back to the same planner. Tree
statistics merge by information state rather than hidden world state. Nature
resolution may inspect the sampled world, but only to propose concrete encoded
outcome exchanges. The selected live action is core-revalidated.

## Required results

- Agent views hide Nature and the other agent.
- Two worlds with different hidden truths have equal Scout observation keys,
  equal public proposal sets, and the same seeded ISMCTS decision.
- The initial ISMCTS decision is the public `BeginScan` exchange in either
  hidden world.
- Hidden instantiation, scan resolution, and encounter exchanges never appear
  in the Scout's decision source.
- Intelligence changes ownership only through a share exchange.
- Role identities reject scan results, injuries, or intelligence bound to the
  wrong agent.
- After observing, the caller filters 16 initial belief worlds to a posterior
  and replans; the next selected action is the now-causal share exchange.
- Joint movement, encounter, treatment, and rescue are multi-account rates.
- A coordinated successful trace contains instantiation, scan intent, scan
  resolution, share, encounter, and finish exchanges and replays exactly.
- Across all 16 weighted scenarios, coordination succeeds 12 times and direct
  north succeeds 8.
- The complete trace becomes one RL transition per exchange, ending with an
  encoded terminal outcome.
- A sampled Pareto front retains the reliability/time/medical-use tradeoff and
  remains explicitly approximate.

## API pressure

- Agent-specific observations and information boundaries.
- Information-state identities and observation-safe tree transpositions.
- Belief sampling from encoded priors without giving policies hidden state.
- Lazy public proposal generation followed by core applicability filtering.
- Multi-agent communication as economic state change.
- Hidden truth, belief, hazard, and deadline in one model.
- Exact 16-scenario counterfactual comparison plus genuine seeded Monte Carlo
  over partially observed policies.
- Caller-owned belief evolution and repeated information-set planning.
- Assessment-derived masks and replay-derived learning trajectories.
- Approximate fronts provide set-valued policy supervision without claiming
  exact hidden-world performance.

## In this folder

- `mod.rs` is the model: the Micro mission, its Nature, agent views, beliefs,
  ISMCTS planning, scenario evaluation and Monte Carlo.
- `expedition/` is the Showcase and Stress expedition, an ontology of its own
  laid out the same way: its model in `mod.rs`, the documents of its studies
  in `study.rs`, and its presentation in `view.rs`.
- `study.rs` is `Mission`, the mission's `axionomy_study::Problem`: its
  catalog entry, strategies and the documents a study holds, each derived by
  replay.
- `view.rs` presents it: labels, units, scene and leaderboards.

[`examples/mission.rs`](https://github.com/BiomaAI/axionomy/blob/main/crates/axionomy-problems/examples/mission.rs) runs it:

```console
cargo run -p axionomy-problems --example mission
```

The mission is one of the [conformance problems](https://github.com/BiomaAI/axionomy/blob/main/PROBLEMS.md).
