# Making the existing examples demonstrate Axionomy

Axionomy's strongest claim is that different decision makers can reason over
one authoritative environment: propose an action, inspect its feasibility,
explore a fork, commit through the same rules, and independently replay the
consequences. Assets include information, permissions, time, and obligations
as well as material resources. The examples should make those relationships
observable.

The suite already has fourteen domains. The next investment should be depth
and explanation within those domains. Bigger account counts and longer traces
are workload measurements; the persuasive evidence is a decision whose
consequences a visitor can understand and verify.

## What the review found

- **The Mission search and displayed policy were disconnected.** The service
  called ISMCTS once, then built both replays with fixed policies. It attached
  that search's statistics to both. The coordinated replay now executes
  repeated ISMCTS choices and conditions beliefs between decisions. The direct
  baseline retains its own policy-evaluation evidence.
- **Private observations appeared frozen.** The adapter populated opening
  observations and the generic viewer always read them. Mission now derives
  observations after each replayed exchange, and the viewer follows the
  selected frame. The Scout's sighting and its transfer to the Medic are
  inspectable as actual balances.
- **A malformed API call is a weak demonstration of a domain constraint.**
  Mission now includes a fully bound proposal to move together before sharing.
  Its assessment exposes the Scout's missing shared sighting and the Medic's
  missing intelligence. The existing missing-role probe remains available.
- **Comparison was hidden behind a disclosure.** Studio now opens the outcome
  table and includes the first three terminal scene metrics alongside their
  initial values. These are existing Rust projections, not new browser scores.
  For the Living Market this makes the price and liquidity differences visible
  across all three existing realities.
- **The Living Market's console example omitted its comparative story.** It now
  replays the no-whale and thin-liquidity alternatives, prints their price
  differences from the market day, and attempts a funded but over-demanding
  trade. The failed apply is checked against the complete original state.

These changes do not increase the number of examples. They expose and connect
capabilities that the same reference models already support.

## Lead with three existing demonstrations

| Example | Question to make concrete | Evidence the visitor should inspect |
| --- | --- | --- |
| Living Market | How much of the outcome depends on one participant or liquidity depth? | Initial and closing reserves and price, three replayed realities, direct price effects, exact coalition attribution, rejected slippage protection |
| Hidden-information Mission | What changes when an agent learns something, and who is allowed to know it? | Private sighting, 16-to-8 belief conditioning, rejected premature movement, explicit information transfer, replanned actions, costs and failures |
| Autonomous Work League | What does winning cost, and who wins under a different objective? | A shared finite job market, claims and facility use, resource consumption, failures and repair, different leaders at the same replay step |

Keep the Maze as the shortest onboarding route into the four primitives. Use
these three as the deeper demonstration of why the architecture matters.

## Demonstration sequence

1. State a concrete goal and constraint before naming an algorithm.
2. Show an attractive alternative and its cost or missing prerequisite.
3. Inspect the assessment or observation that changes the decision.
4. Replay the accepted exchange and its effects on every affected participant.
5. Compare the resulting outcomes; retain failure and uncertainty explicitly.
6. End with evidence a visitor can inspect: a receipt, an invariant, a private
   observation, a counterfactual trace, or a non-dominated alternative.

For Mission, run **seed 11, budget 128**: the planner follows the South sighting,
shares it, responds to injury, and succeeds while the direct North baseline
fails in the same instantiated scenario. Then run **seed 17**: the misleading
sighting sends the planner South while the objective is North. Valid execution
does not guarantee a successful decision under uncertainty. The separate
sampled frontier evaluates the two fixed-policy baselines; it does not estimate
the new planner's success probability. Links select controls; press **Run** to
compute a different seed.

For the Showcase Living Market, the existing market-day replay closes at
12.966 credits per energy, the no-whale replay at 8.612, and thin liquidity at
10.080. These are endogenous reserve ratios. Final price alone does not prove
that every thin-liquidity trade has a larger effect; inspect the path and
individual receipts as well. Direct per-actor changes and coalition-based
Shapley contributions answer different attribution questions.

## Next depth improvements, in priority order

These are proposed follow-up changes, not capabilities added by this pass.

| Existing example | Next substantive improvement | Acceptance evidence |
| --- | --- | --- |
| Work League | Put a contested job or facility, a failure, and the recovery decision at the center of the replay. Compare policies under a documented common scenario protocol. | Exactly one valid claim; failed contenders cannot debit resources; repair changes future feasible work; leaders differ at the same snapshot. Current alternative fields use different seed offsets, so they are not isolated causal comparisons. |
| Logistics | Execute the route planner repeatedly through delivery and disruption. The current MCTS artifact stops after loading and its first route decision. | Chosen route is actually executed; a breakdown causes a new assessment and decision; full delivery or explicit failure replays; compare completion risk and fuel/time costs. |
| Marketplace | Make a blocked competing order and its successful alternative the main story. | One settlement moves goods, payment, carrier fee, commission, and tax atomically; an infeasible settlement changes none of them; participant utility tradeoffs remain visible. |
| Perishables | Follow a power-loss decision through a deadline and its inventory consequences. | Cooling spends encoded energy; the condition changes once at cohort level; claims and conserved quantities reconcile; show which alternative saves inventory and what it costs. |
| Maze | Explain why the shortest-looking route loses under another objective. | Key acquisition and gate opening are causal; BFS/A*/Dijkstra retain their own work; each exact Pareto route replays with its energy/time balance. |
| Sokoban | Explain a tempting push that creates an irreversible deadlock. | A push is atomic, deadlock evidence identifies the lost continuation, and a successful alternative avoids it. |
| Job shop | Make a bottleneck and competing job completion priorities visible. | Machine capacity is never double-booked; precedence is preserved; different schedules expose makespan versus individual completion tradeoffs. |
| Workshop | Put a catalyst and a material/waste/time tradeoff into one short chain. | Catalyst survives reuse, consumed inputs reconcile with output and waste, and non-dominated recipes retain their traces. |
| Bridge | Follow a losing bid through escrow and resolution. | Priority allocation differs from first-come; credits and capacity reconcile; losing bids cannot create a hidden debit. |
| Rescue | Show the value and cost of sensing against direct commitment. | Exact scenario comparison explains the decision; noisy observations remain uncertain; belief-conditioned actions do not read hidden truth. |
| Connect Four | Explain a forced defensive move rather than showing a long game. | Root alternatives expose the threat; the selected move is replayed; the existing board/minimax oracle checks the result. |
| Exact Cover | Follow one attractive subset to a contradiction and another to a cover. | Each element is covered exactly once; Algorithm X and generic search agree on feasibility and replay through the same core. |

Mission should next compare the executed planner across all sixteen encoded
scenarios, including its budget sensitivity. Living Market should next make a
single actor's direct effect and its coalition contribution inspectable together.
Neither requires another domain.

The architectural boundary remains the one in
[CONTINUOUS_AGENT_SYSTEMS.md](CONTINUOUS_AGENT_SYSTEMS.md): the harness owns
agents, learning, and deployment; Axionomy owns the rules, consequences, and
replay. These bounded reference policies demonstrate that boundary. They do
not yet demonstrate a continuously learning agent population.
