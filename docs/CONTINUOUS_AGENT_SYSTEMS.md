# Continuous Agent Systems: Autonomy Harness and Work Utility System

## Proposal

A Work Utility System can serve as the deterministic mechanics, consequence,
and replay layer underneath continuously operating agents. It does not train
models, own agents, manage policy populations, or promote deployments. Those
responsibilities belong to the Autonomy Harness.

The intended relationship is:

> The Autonomy Harness proposes decisions. The Work Utility System
> determines what those decisions mean, whether they are permitted, and what
> consequences they produce.

This separation allows an Autonomy Harness to orchestrate rule-based agents,
language-model agents, reinforcement learners, planners, human operators, and
multi-agent frameworks against one authoritative definition of the
environment without forcing the Work Utility System to become an AI framework.

## Central architectural rule

The Work Utility System owns:

- Authoritative state and game mechanics.
- Assets, accounts, rates, exchanges, goals, and invariants.
- Atomic transition validation and application.
- Feasibility assessment and structured shortfalls.
- Deterministic snapshots, forks, receipts, and replay.
- Actor-scoped observations derived from authoritative state.
- Generic, non-authoritative search and simulation over forks.

The Autonomy Harness owns:

- Agent implementations and model providers.
- Prompts, memory, tools, and orchestration.
- Policy training and inference.
- Experience stores, feature pipelines, and datasets.
- Policy populations, opponent pools, and league scheduling.
- Model and policy registries.
- Candidate qualification, promotion, deployment, and rollback.
- Operational monitoring and human oversight.

The boundary can be tested with a simple question:

> If the Autonomy Harness disappeared, would the environment still have a
> complete and unambiguous definition of valid state and valid change?

The answer must be yes. Conversely, replacing one Autonomy Harness with
another must not require rewriting the environment's mechanics.

## System relationship

```text
                         AUTONOMY HARNESS

 observations ──→ agents and policies ──→ proposed exchanges
      ↑                    │                       │
      │                    ├── training            │
      │                    ├── policy populations  │
      │                    ├── deployment gates    │
      │                    └── monitoring          │
      │                                            ↓
 ┌──────────────────────────────────────────────────────────┐
 │                   WORK UTILITY SYSTEM                    │
 │                                                          │
 │ actor view → assess → fork/search/simulate → apply       │
 │                                             │            │
 │ accounts + assets + rates + exchanges ──────┘            │
 │                                                          │
 │ output: receipts, shortfalls, objectives, traces         │
 └──────────────────────────────────────────────────────────┘
      │
      └──→ verified outcomes and trajectories return to Autonomy Harness
```

The Work Utility System may provide generic algorithms, but they remain
disposable reasoning processes over authoritative snapshots. They may propose
an exchange; they cannot make one valid or mutate state outside the transition
contract.

## Continuous decision loop

A continuously operating agent does not need a continuously mutating model.
It needs a repeated decision protocol:

1. External events enter the environment through explicit exchanges.
2. The Autonomy Harness requests the observation permitted for an actor.
3. One or more agents or policies propose exchanges.
4. The Work Utility System assesses each proposal.
5. The Autonomy Harness may search or simulate from a fork before choosing.
6. The selected applicable exchange is applied atomically.
7. A receipt and replay step are returned to the Autonomy Harness.
8. The Autonomy Harness observes the new state and begins the next decision.

This is compatible with receding-horizon or model-predictive control: plan
from the latest snapshot, execute a bounded part of the plan, observe the
result, and plan again. The planner may be replaced at any time without
changing transition semantics.

Long-running decisions should use resumable work sessions. The Autonomy
Harness supplies a budget, observes progress, and may pause, resume, or cancel.
A completed session returns proposals and evidence rather than committing
automatically.

## Continuous learning remains outside

The Work Utility System should make verified learning material easy to obtain,
but it should not perform learning. For each decision it can return:

- The actor-visible observation.
- Applicable and rejected proposals.
- Valid-action masks.
- Structured feasibility shortfalls.
- Projected and realized account deltas.
- Consumed, preserved, and produced assets.
- Intermediate goal progress.
- Per-participant objective vectors.
- Search observations and counterfactual outcomes.
- The accepted exchange and resulting receipt.
- A replay-verifiable trajectory.

The Autonomy Harness may transform this evidence into trajectories, replay
buffers, preference data, supervised examples, or reinforcement-learning
episodes. It may train new candidate policies using imitation learning,
offline learning, policy gradients, actor-critic methods, or any future
approach.

The trained policy returns only as another policy managed by the Autonomy
Harness. It receives observations and proposes exchanges through the same
interface as every other policy.

### Policy lifecycle

```text
verified traces
      ↓
Autonomy Harness-owned dataset or replay buffer
      ↓
Autonomy Harness-owned training
      ↓
candidate policy
      ↓
evaluation through Work Utility System forks
      ↓
Autonomy Harness-owned assurance and promotion decision
      ↓
approved, shadowed, rejected, or retired policy
```

Continuous learning should therefore mean continuous evidence collection and
candidate improvement, not unrestricted self-modification in production.

## Continuous multi-agent game

The same boundary supports persistent cooperative, competitive, and
mixed-motive environments.

The Work Utility System represents:

- Actor identities and action authority.
- Private and shared information.
- Resources, territory, permissions, commitments, and treaties.
- Faction-specific goals and utility-producing consequences.
- Simultaneous intent and atomic resolution.
- Explicit Nature and chance outcomes.
- Communication as state-changing exchanges when it matters mechanically.

The Autonomy Harness assigns policies to actors, schedules matches, maintains
populations, trains responses, and chooses deployment mixtures.

### Different faction objectives

Outcomes should retain a profile rather than one global reward:

```text
Faction A → [survival, territory, wealth, influence]
Faction B → [safety, resources, ecological stability]
Faction C → [expansion, information, readiness]
```

Each value must be derived from terminal or intermediate authoritative state.
The Autonomy Harness may scalarize a faction's vector for one training run,
use lexicographic priorities, train a preference-conditioned policy, or
preserve a Pareto set. The original vector remains available so that
evaluation does not erase tradeoffs.

If weights or priorities are part of the environment—for example, a faction's
survival need rising during a famine—they belong in authoritative state. If
they are merely part of decision policy, they may remain Autonomy
Harness-owned.

### Emergent relationships

Enemies and allies need not be assigned in advance. They can emerge from
repeated consequences involving scarcity, incompatible goals, externalities,
trade, protection, and shared threats.

The Autonomy Harness may derive a contextual relationship estimate:

```text
impact(i, j) = expected change in faction j's utility after faction i acts
```

Consistently negative impact suggests antagonism; positive impact suggests
cooperation. This relationship graph is analysis, not environment truth. If a
declared alliance, treaty, or war changes which actions are legal, that status
and its lifecycle must be represented by assets and exchanges.

## Algorithm portfolio and ownership

No single algorithm covers continuous decisions, learning, strategic response,
and assurance. The system should expose a narrow execution contract that
allows the Autonomy Harness to compose an algorithm portfolio.

| Capability | Suitable algorithms | Primary owner |
| --- | --- | --- |
| Exact deterministic planning | BFS, Dijkstra, A*, branch-and-bound | Either Autonomy Harness or generic search library |
| Multi-objective planning | Exact or bounded Pareto search | Generic search library |
| Stochastic consequence estimation | Monte Carlo and stratified sampling | Either |
| Long-horizon online planning | MCTS and receding-horizon search | Either |
| Hidden-information planning | ISMCTS | Either |
| Failure and counterexample discovery | Adversarial MCTS, best-first search, branch-and-bound | Autonomy Harness using utility forks |
| Cooperative policy learning | PPO, MAPPO, value decomposition | Autonomy Harness |
| Mixed-motive policy learning | Separate-critic or attention-based actor-critic | Autonomy Harness |
| Strategic population learning | PSRO, league training, population-based training | Autonomy Harness |
| Approved-policy selection | Contextual bandits or explicit routing rules | Autonomy Harness |
| Population evaluation | Payoff matrices, regret, exploitability, alpha-rank | Autonomy Harness |
| Drift and operational monitoring | Sequential tests, change detection, incident rules | Autonomy Harness |

Generic search can be shipped alongside the Work Utility System because it
operates directly over forks and produces replayable traces. Neural training,
model storage, and policy deployment should not be shipped there.

### Best-response composition

For strategic multi-agent learning, the Autonomy Harness can use Policy-Space
Response Oracles:

1. Maintain a population of policies for each faction.
2. Run policy combinations through the Work Utility System.
3. Record each faction's replay-derived payoff vector.
4. Compute a meta-strategy over the current populations.
5. Ask a best-response oracle for a policy against that mixture.
6. Add the result to the population and repeat.

The best-response oracle may itself be MCTS, ISMCTS, a heuristic planner, a
learned policy, a language-model agent, or a hybrid. This is precisely why the
utility interface should not depend on a particular Autonomy Harness
implementation.

## Continuous assurance

Continuous assurance means continuously refreshed evidence under explicit
assumptions. It does not mean that the Work Utility System certifies the
correctness of sensors, the completeness of the environment model, or the
universal safety of a learned policy.

### Evidence layers

| Layer | Work Utility System contribution | Autonomy Harness contribution |
| --- | --- | --- |
| Proposal | Feasibility, authorization, shortfalls | Decide whether to retry, repair, or abstain |
| Transition | Invariants, atomic application, receipt | Operational deadline and escalation policy |
| Trace | Deterministic replay and terminal outcomes | Retention, audit workflow, and incident linkage |
| Scenario | Reproducible forks and outcomes | Scenario generation and coverage policy |
| Candidate | Comparable evaluation artifacts | Statistical thresholds and promotion decision |
| Deployment | Current decision evidence | Shadowing, canarying, monitoring, rollback, human override |

### Runtime shield

The proposal boundary forms a deterministic shield:

```text
policy proposal
      ↓
assess against current authoritative state
      ├── invalid or infeasible → explain, repair, abstain, or fallback
      └── applicable           → eligible for selection and application
```

This does not guarantee that every applicable exchange is desirable. It
guarantees that accepted changes obey the encoded mechanics. The Autonomy
Harness may add risk limits, approval rules, or a fallback policy before
choosing among valid actions.

### Counterexample search

Ordinary evaluation estimates typical performance. Assurance should also
search deliberately for failure:

- Make Nature or an adversarial policy maximize loss, shortfall, or hazard.
- Search for the minimum-cost trace reaching an unsafe goal condition.
- Mutate initial resources, timing, permissions, and observation histories.
- Compose individually harmless events into adverse sequences.
- Preserve every discovered failure as a replayable regression scenario.

The failure search uses the same authoritative mechanics as normal planning.
It cannot manufacture an invalid transition merely to create a counterexample.

### Statistical and operational gates

The Autonomy Harness may qualify a candidate using:

- Mean and confidence or credible intervals.
- Worst-decile performance and conditional value at risk.
- Catastrophic-failure probability.
- Regret against the active policy.
- Performance across opponents, teammates, roles, and hidden seeds.
- Pareto-front quality and coverage.
- Shield intervention, abstention, and fallback frequency.
- Decision work, latency, and resource cost.

Planning seeds and held-out evaluation seeds should remain separate. Competing
policies should face common evaluation scenarios so stochastic variation does
not masquerade as improvement.

The Autonomy Harness should promote candidates through offline evaluation,
adversarial testing, shadow execution, limited canaries, and explicit rollback
criteria. The Work Utility System supplies the evidence for those gates but
does not own the gates.

## Minimal integration contract

An implementation-neutral interface between the Autonomy Harness and Work
Utility System needs operations equivalent to:

```text
describe_model()                         → mechanics and identifiers
snapshot()                               → authoritative state reference
actor_view(snapshot, actor)              → permitted observation
assess(snapshot, proposed_exchange)      → applicability or explanation
fork(snapshot)                           → isolated counterfactual state
apply(fork_or_live, exchange)            → receipt or rejection
replay(source_snapshot, trace)           → verified terminal state
objectives(snapshot, objective_schema)   → ordered outcome vector
```

Optional generic search accepts Autonomy Harness-supplied action sources,
objective projections, work budgets, and progress observers:

```text
search(snapshot, action_source, objective, budget, observer)
    → proposals + replayable evidence
```

This contract should work synchronously in process and asynchronously through
CLI, HTTP, task, or tool protocols. Transport changes must not change
transition meaning.

## Non-goals for the Work Utility System

The Work Utility System should not add:

- A neural-network runtime as a core dependency.
- PPO, MAPPO, or other training loops.
- Replay-buffer or feature-store infrastructure.
- Prompt, memory, or language-model orchestration.
- A model or policy registry.
- Automated policy promotion.
- Production canary and rollback orchestration.
- A universal global reward across independent participants.
- A permanent enemy or ally classification inferred outside mechanics.

It should remain equally usable by learning and non-learning Autonomy Harness
implementations.

## Delivery sequence

### Phase 1: Stable decision boundary

- Actor-scoped observations.
- Proposal assessment and application.
- Forks, receipts, and deterministic replay.
- Resumable generic search with progress and interruption.
- Portable decision and trace artifacts.

### Phase 2: Evaluation support

- Reproducible instance and scenario generation.
- Ordered per-participant objective profiles.
- Common work counters and seed identity.
- Batch match execution and outcome artifacts.
- Adversarial goal and counterexample search.

### Phase 3: Multi-agent pressure

- Explicit actor action authority and information boundaries.
- Simultaneous proposal and resolution patterns.
- Teammate, opponent, role, and seed permutations.
- Payoff tensors and replayable policy-profile matches.
- Best-response interfaces usable by Autonomy Harness-owned PSRO and league
  systems.

### Phase 4: Assurance integration

- Evidence bundles suitable for external qualification gates.
- Shadow-decision comparison without state mutation.
- Stable policy and Autonomy Harness metadata in decision artifacts.
- Incident-to-trace correlation and reproducible regression scenarios.
- Streaming progress and evidence for operational monitoring.

None of these phases requires the Work Utility System to train or deploy a
model. They make it a better environment, reasoning, and evidence dependency
for frameworks that do.

## Client-facing summary

> The proposed architecture supports continuous agent operation, continuous
> learning, persistent multi-agent games, and continuous assurance without
> embedding learning inside the Work Utility System. The Autonomy Harness owns
> agents, policies, training, populations, deployment, and monitoring. It calls
> the Work Utility System to validate actions, simulate counterfactuals,
> evaluate consequences, and produce replayable evidence. This keeps
> environment mechanics stable while allowing algorithms and models to evolve
> independently.

## References

- [Policy-Space Response Oracles](https://arxiv.org/abs/1711.00832)
- [Safe Reinforcement Learning via Shielding](https://ojs.aaai.org/index.php/AAAI/article/view/11797)
- [The Surprising Effectiveness of PPO in Cooperative Multi-Agent Games](https://arxiv.org/abs/2103.01955)
- [Alpha-Rank: Multi-Agent Evaluation by Evolution](https://www.nature.com/articles/s41598-019-45619-9)
- [NIST AI Risk Management Framework Core](https://airc.nist.gov/airmf-resources/airmf/5-sec-core/)
