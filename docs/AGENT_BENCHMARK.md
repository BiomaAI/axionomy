# Agent Benchmark and Rating Design

## Status and purpose

This document describes a future Axionomy enhancement. It is not part of the
implemented benchmark suite and does not add agent, benchmark, or rating
concepts to the kernel.

The benchmark should measure how effectively agents use Axionomy to understand
and act within closed economies. It covers individual problem solving,
cooperative teams, direct competition, and mixed-motive coordination without
prematurely claiming one universal measure of intelligence.

The work has two purposes:

1. Produce reproducible, explainable comparisons of agent outcomes,
   robustness, generalization, and resource use.
2. Pressure the public engine, search, service, CLI, HTTP, MCP, and Studio APIs
   until they support realistic agent callers without granting an evaluator or
   interface semantic authority.

Autonomous robot fleets are one useful future benchmark family because they
combine logistics, scheduling, shared resources, uncertainty, and
coordination. They are not the identity or limit of this design.

## Terminology

- **Agent** means an evaluated policy implementation that observes through and
  acts through a declared Axionomy interface.
- **Actor** means an identity or authority represented by accounts and exchange
  roles inside one economy.
- **Team** means a declared composition of agents evaluated together.
- **Instance** means one closed initial economy plus its declared evaluation
  contract and hidden scenario identity.

An agent may control one actor, several actors, or only a restricted proposal
surface. Multiple accounts do not by themselves make a benchmark multi-agent;
the distinction is whether independently evaluated policies have separate
observation and action authority.

## Authority boundary

Every benchmark instance is an ordinary closed economy. Authoritative state,
goals, constraints, information, chance, utility, and semantic time remain in
accounts and assets. Every accepted action or environmental effect remains a
rate applied as an atomic exchange. An evaluator may generate an initial
economy and later inspect it, but it cannot maintain a parallel world model or
inject hidden mutations during execution.

Leaderboards, baselines, normalized scores, Pareto indicators, confidence
intervals, and latent ratings are disposable comparisons over replay-verified
economic outcomes. Operational costs may be measured outside the economy
because removing the measurement cannot change a transition or result. No
score can make an invalid trace valid, create terminal truth, or silently
replace encoded multi-objective outcomes with benchmark-owned utility.

The future implementation should live in an outer `axionomy-eval` or
`axionomy-benchmarks` crate that consumes public engine APIs and portable
artifacts. Evaluation pressure may improve generic counters, observation and
proposal boundaries, policy artifacts, or service contracts. An agent- or
domain-specific concept moves inward only when independent domains demonstrate
that it is universal economic machinery.

## Evaluation designs

### Single-agent

One agent has the declared authority and observation needed to solve a problem.
Tracks should cover:

- Deterministic planning, search, allocation, and optimization.
- Long-horizon execution and replanning.
- Stochastic decisions with encoded Nature outcomes.
- Partial observation and belief-conditioned action.
- Multi-objective outcome discovery or preference-conditioned selection.
- Correct recognition of infeasible or unsatisfiable instances.

The agent may submit a final trace, an anytime sequence of improving traces, a
contingent policy, or a bounded Pareto set according to the track contract.

### Cooperative multi-agent

Several agents act from declared, potentially different observations toward a
shared encoded goal. Evaluation must preserve the distinction between:

- Centralized control of several actor accounts.
- Decentralized policies with private observations.
- Explicit communication and information transfer.
- Simultaneous proposals and encoded conflict resolution.
- Stable teams and teams assembled from independently rated agents.

Team performance is primary. Individual contribution is reported only when it
can be estimated by controlled team rotation, replacement, or ablation; it is
not inferred from arbitrary external reward shaping.

### Competitive multi-agent

Agents have opposed or partially opposed encoded objectives. Games, auctions,
resource contention, and adversarial missions belong here. Evaluation should
alternate roles, starting positions, and hidden seeds and should separate
absolute rule competence from performance against the current opponent pool.

Pairwise or multiplayer ratings may be meaningful in these tracks, but every
match remains replayable and every participant outcome remains readable from
the terminal economy.

### Mixed-motive multi-agent

Agents share some goals while competing over other outcomes. Negotiation,
markets, coalition formation, shared infrastructure, and allocation problems
belong here. Reports should retain:

- Goal and constraint satisfaction.
- Each participant's encoded outcome vector.
- Aggregate and distributional outcomes.
- Pareto efficiency and avoidable domination.
- Agreement, deadlock, defection, and recovery outcomes.

Social welfare, fairness, or inequality measures are evaluation projections
unless the model explicitly encodes them as economic facts or goals. An outer
metric may compare outcomes but cannot retroactively define what an exchange
meant to its participants.

## Simultaneous action and communication

Multi-agent execution must not become an external batch mutation. Agents may
commit proposals, intentions, reservations, bids, messages, or capability
facts through exchanges. A declared resolution law then accepts compatible
actions or atomically settles contention through another encoded exchange.

Centralized and decentralized tracks remain distinct because they grant
different information and authority. Communication is not free ambient state:
when message delivery, delay, bandwidth, trust, or cost affects the problem,
those effects belong in accounts, assets, and rates. Operational transport may
carry an already-authorized observation without becoming mission truth.

## Benchmark families and instances

The existing conformance problems are design proofs, not a sufficient rating
set. A credible benchmark needs versioned families with many generated public
and hidden instances.

Candidate families include:

- Pathfinding and resource-constrained navigation.
- Constraint satisfaction and exact cover.
- Production, scheduling, and logistics.
- Autonomous robot or vehicle missions.
- Stochastic rescue and partially observed missions.
- Markets, negotiation, and allocation.
- Cooperative and adversarial games.
- Perishable inventory and event-driven operations.

Difficulty should vary along independently inspectable dimensions:

- State-space size, branching, depth, and horizon.
- Resource scarcity and constraint tightness.
- Number and heterogeneity of actors.
- Task count, dependencies, deadlines, and arrival pattern.
- Information asymmetry and communication cost.
- Chance support, uncertainty, and disruption frequency.
- Coordination density and contention for shared resources.
- Number and conflict of objective dimensions.

Public practice instances teach the contract. Hidden instances and scenario
seeds test generalization. Small instances provide exact oracle calibration;
larger instances use frozen high-budget best-known references. Benchmark
versions freeze generators, instance sets, evaluation rules, and baselines so
ratings remain reproducible.

## Evaluation run contract

An evaluation run is identified by:

```text
agent implementation and version
+ benchmark, family, instance, and role identity
+ team composition or opponent identities when applicable
+ hidden scenario seed
+ permitted observations, interface, and tools
+ deterministic work budget
+ benchmark and evaluator version
```

Depending on the track, a submission may be:

- A final replayable trace.
- An anytime sequence of improving replayable traces.
- A policy evaluated against held-out encoded scenarios.
- A bounded set of non-dominated alternatives.
- One participant policy combined with independently supplied peer policies.
- A complete team or opponent policy set.

The evaluation record distinguishes:

- Solved or goal reached.
- Correctly infeasible or unsatisfiable.
- Feasible but incomplete.
- Valid terminal outcome without the requested goal.
- Invalid final trace.
- Exhausted without a solution.
- Budget exceeded.
- Cancelled.
- Tool or transport failure.
- Agent crash.

Rejected proposals explored through `assess` are legitimate deliberation, not
failed runs. They affect optional tool-efficiency and safety-proposal
diagnostics but not result validity unless the agent attempts to commit them as
its submitted outcome.

## Outcome measures

Safety, replay validity, and mandatory encoded constraints are hard gates.
Beyond those gates, authoritative outcome dimensions are read from terminal
accounts and traces. Examples include:

- Goal attainment and priority-weighted task value.
- Completion time, lateness, distance, energy, waste, and wear.
- Reliability, risk, and catastrophic failure.
- Participant utility, retained resources, and allocation.
- Shared-resource utilization and congestion.
- Information acquired, revealed, transferred, or preserved.

Conflicting objectives remain ordered Pareto vectors. Dominance,
fixed-reference hypervolume for bounded result sets, distance to a reference
front, confidence intervals, and final ranking are evaluation policy.

Incomplete outcomes may expose encoded goal progress and feasibility
shortfalls as dense diagnostics. They become official score components only
when the benchmark declares a stable, non-gameable projection. Distance to one
proposed exchange is not automatically distance to solving the complete
problem.

Multi-agent reports should separate:

- Team or system outcome.
- Per-participant encoded outcomes.
- Outcome distribution across participants.
- Coordination failures and contention resolutions.
- Results across different teammate and opponent combinations.

## Work, cost, and anytime performance

The primary implementation-independent cost should be the number of concrete
core transition evaluations: exchanges assessed or applied. Algorithm-specific
expanded-state, rollout, sample, iteration, and information-set counters remain
useful diagnostics.

Wall-clock time, model tokens, tool calls, network requests, and monetary cost
are reported separately because they measure deployment behavior rather than
economic work. A track may impose limits on them, but it must not pretend that
one token, one MCTS iteration, and one expanded graph state are intrinsically
the same unit.

Every resumable run should retain an anytime quality-versus-budget curve. The
leaderboard can therefore compare agents at several fixed budgets and measure
how quickly they find usable partial and final outcomes instead of observing
only the last answer.

## Stochastic evaluation

Planning seeds and evaluation seeds must be separate. Policies are tested out
of sample on common hidden Nature seeds so competing agents face the same
scenario distribution without learning the evaluation outcomes.

Reports should include:

- Mean outcome.
- Confidence or credible interval.
- Worst-decile or conditional tail outcome.
- Catastrophic failure probability.
- Sensitivity to seed, teammate, opponent, and role assignment.

An agent receives only the observation permitted by its track. Evaluation may
inspect the complete economy after execution, but hidden state cannot enter an
agent's information identity, proposal source, or policy input.

## Frozen baselines

Each benchmark family should provide an appropriate subset of:

- No-op and random-applicable floors.
- Simple greedy or rule-based policies.
- Generic Axionomy BFS, A*, best-first, Pareto, Monte Carlo, MCTS, or ISMCTS.
- A specialized domain algorithm or external solver adapter.
- An exact oracle or high-budget best-known reference where tractable.
- Independent-agent and centralized joint-planning baselines for cooperative
  tasks.
- Self-play, fixed-opponent, and population baselines for competitive tasks.
- A clairvoyant stochastic upper bound labeled unattainable rather than
  presented as an ordinary competitor.

A normalized instance measure may compare an agent with a frozen floor and
reference. Negative values remain visible when an agent is worse than the
floor, and values above one reveal that a non-exact reference was beaten.

## Leaderboards and ratings

The implemented Autonomous Work League is the mechanics-level reference for
this boundary. It derives value, throughput, efficiency, residual-waste,
reliability, and Pareto standings at every verified economic step. It is not
yet a population benchmark or durable rating service; it proves that several
honest leaderboards can be reconstructed from one authoritative replay without
installing a universal score in the engine.

The initial public leaderboard should expose a capability profile rather than
an arbitrary single number:

- Validity and safety.
- Goal and outcome quality.
- Pareto quality.
- Robustness.
- Generalization.
- Coordination or competitive performance where applicable.
- Work and tool efficiency.

Results are compared at fixed transition-evaluation budgets. The leaderboard
publishes per-family and per-design results, macro-averages families so
instance count cannot silently set importance, retains uncertainty, and shows
anytime curves.

An optional fixed-budget Agent Benchmark Score may normalize each instance
between its floor and frozen reference and then macro-average families. It must
always name its benchmark version, evaluation design, observation contract,
and budget. A team result additionally names its complete composition.

After enough agents and instances exist, a multidimensional item-response
model may estimate capabilities and instance difficulty together. Candidate
dimensions include deterministic planning, resource reasoning, uncertainty,
partial observation, recovery, cooperation, competition, and tool use. The
result should be named an Axionomy Agent Rating, carry uncertainty, expose its
capability profile, and never be marketed as IQ.

Elo or a Bayesian competitive rating such as TrueSkill belongs only to
genuinely head-to-head tracks. Cooperative contribution requires cross-team
evaluation or explicit ablation rather than treating teammates as opponents.
A global rating never replaces fixed benchmark outcomes, and the system must
preserve non-transitive matchup evidence instead of hiding it behind one
number.

## API pressure expected from the benchmark

This future program is expected to test whether Axionomy needs better generic
support for:

- Reproducible instance generation and hidden-case execution.
- Uniform transition-work accounting across algorithms and interfaces.
- Policies and contingent plans in addition to completed traces.
- Incremental candidate and artifact publication.
- Actor-scoped observation and proposal authority.
- Multi-agent proposal collection and encoded resolution.
- Team, opponent, role, seed, and budget identity in run artifacts.
- Structured exhaustion, interruption, invalid-result, and operational-error
  outcomes across every search family.
- Evaluation projections derived from exact account balances and receipts.
- Studio comparison of agents, budgets, runs, fronts, and uncertainty.

These requirements begin in the evaluation and service layers. Only
transport-neutral economic validation or replay requirements qualify for the
kernel.

## Staged delivery

1. Define runtime-neutral evaluation records, submission contracts, and
   transition-work accounting.
2. Build deterministic single-agent instance families with exact oracles.
3. Add generated public and hidden cases with frozen baseline policies.
4. Add stochastic and partially observed single-agent policy evaluation.
5. Add cooperative multi-agent missions with centralized and decentralized
   tracks.
6. Add competitive and mixed-motive populations, role rotation, and cross-play.
7. Publish fixed-budget capability profiles, confidence intervals, matchup
   evidence, and anytime curves.
8. Add calibrated agent or competitive ratings only after the result matrix is
   large and diverse enough to support them.

The benchmark succeeds when it distinguishes capable agents, explains why
they differ, remains reproducible under replay, and reveals generic engine
requirements without creating a second source of economic truth.
