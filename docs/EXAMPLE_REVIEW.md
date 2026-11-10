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
- **Scene labels need the same evidence discipline as the ledger.** Visual
  inspection caught a shared-intelligence metric reading the wrong agent and
  global scenario/goal state anchored to fixed compass locations. Those are
  corrected, and agent statuses now expose their actual sighting or briefing.
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

For the **Micro Mission**, study **seed 11, budget 128**: the planner follows the South sighting,
shares it, responds to injury, and succeeds while the direct North baseline
fails in the same instantiated scenario. Then study **seed 17**: the misleading
sighting sends the planner South while the objective is North. Valid execution
does not guarantee a successful decision under uncertainty. The separate
sampled frontier evaluates the two fixed-policy baselines; it does not estimate
the new planner's success probability. Links select controls; press **Study** to
compute a different seed.

The Living Market now evaluates a multi-day economy. Closing prices depend on
seeded sunlight and responsive consumption, investment, and liquidity choices.
Compare the complete paths and unmet obligations, not only the closing price.
Direct per-actor changes and coalition-based Shapley contributions answer
different attribution questions.

## Game-depth implementation

All fourteen games now have substantive interacting mechanics in Showcase and
Stress. Micro remains the compact correctness and algorithm fixture. The table
separates this implemented redesign from possible subsequent extensions.

| Domain | Implemented mechanics | Further extensions |
| --- | --- | --- |
| Work League | Shared time, overlapping activities, occupied service bays, arriving/dependent contracts, claim leases, deadlines, wear, credit-funded recovery, material recycling, paid handoffs, real road topology and shortest-route costs, common job-attempt chance tickets, replay-derived scene evidence | Interactive decisions/forks; broader role and seat evaluation; larger policy populations |
| Logistics | Two heterogeneous vans, three customers, arriving heavy/cold/standard orders, deadline pricing and spoilage, concurrent trips, one occupied trunk road, finite hub staging and cargo transfers, shared paid fuel/repair/towing, wear, seeded vehicle/attempt/route outcomes and responsive fleet dispatch; Micro retains repeated MCTS | Interactive fleet control; broader traffic and failure distributions |
| Living Market | Eight/twelve-day responsive economy, recurring needs and obligations, finite procurement/income, storage and production limits, two-day investment, adaptive liquidity, shared seeded sunlight, honest default and attribution evidence | Interactive intervention/forks and broader policy sensitivity evaluation |
| Mission | Independent timed Scout/Medic movement and work, hidden archive and casualties, private observations, scarce radio or local briefing, joint extraction, one evacuation seat, sector closure, injury delays, clinical pulses, deadlines and finite resupply; Micro retains ISMCTS | Interactive team control; broader communication and hazard distributions |
| Rescue | Three/four hidden casualties, continuing clinical decline, repeatable sector search, fallible hints, stabilization and restoration, one evacuation seat, speed/fuel tradeoffs, finite base resupply and observation-only policies | Interactive incident control; broader sensor and casualty distributions |
| Perishables | Six-cohort cold chain with physical storage limits, inherited heat on split, daily grid outages, finite electricity/fuel/labor, processing, paid transport, backup generation, spoilage disposal and recurring food obligations; four policies share the outage schedule | Interactive storage/processing decisions and forks; broader outage distributions |
| Marketplace | Standard/concession offers, funded reservations, contract/fulfillment separation, two occupied carriers, arriving orders, known route delays, delivery deadlines, late rebates and atomic reservation refunds; bounded buyer/seller plan comparisons | Interactive contract decisions and forks; additional reliability scenarios |
| Workshop | Competing funded contracts, frame/panel production chains, tool wear and finite repair, upgrade investment, offcut recycling, separate delivery/payment and exact waste/time/revenue frontier | Interactive production decisions and forks; broader scenario evaluation |
| Scheduling | Queue reservations distinct from actual execution, arriving jobs, alternative machines, setup costs, shared crews, paused work, finite repair and resume; a single-crew Stress workload | Interactive machine routing and recovery decisions; broader job populations |
| Bridge | Shared time, staggered traffic waves, occupied 2–3 tick crossings, expiring lane reservations, deadlines, responsive bids and a paid concurrent detour; common traffic across policies | Interactive routing and forks; broader traffic and seat evaluation |
| Maze | Shared vault/transit power circuit, injury delays, finite protective gear, treatment and recovery routes; four exact tradeoffs and scene evidence | Interactive route decisions and forks |
| Sokoban | Crate-weight plates, doors, remote latches, far-side exit, two-circuit Stress warehouse, reverse-push dead-square proofs | Interactive pushes and replay forks; broader generated warehouse evaluation |
| Exact Cover | Connected irregular pairs/triples/quadruples, legal dead ends, different-size exact covers, continuation from partial covers, globally impossible instances with no missing elements | Interactive subset choice and forks |
| Connect Four | Core-enforced post-win terminal truth, tactical MCTS, forced blocks and double threats, positional rollouts, center-first and depth-five opponents, per-frame threat evidence | Human moves/forks and broader paired opening/seat evaluation |

Micro fixtures remain the compact conformance surface. Showcase must demonstrate
interacting mechanics; Stress must vary structure and constraints as well as
quantity. Generic Studio decision control and replay forking are separate future
extensions; this implementation presents computed runs and their consequences.
New mutable mechanics must remain authoritative balances and
exchanges; policies may propose but never bypass them.

The architectural boundary remains the one in
[CONTINUOUS_AGENT_SYSTEMS.md](CONTINUOUS_AGENT_SYSTEMS.md): the harness owns
agents, learning, and deployment; Axionomy owns the rules, consequences, and
replay. These bounded reference policies demonstrate that boundary. They do
not yet demonstrate a continuously learning agent population.

### Validation of the implemented redesign

- Full Rust workspace, all targets and features: 265 tests passed. Focused
  Mission, Rescue and fleet regressions also passed after the final refinements,
  including an additional check against remote medical resupply.
- Studio: 8 unit tests, 32 native-browser tests and 3 browser-only Wasm tests
  passed. The production UI build and workspace check passed.
- All fourteen default artifacts were regenerated and checked against the
  current catalog. Tests exercise deterministic replay, resource conservation,
  actual occupancy, deadlines, private observations and common seeded scenarios.
- Visual checks cover every Maze frame, warehouse cells, graph legibility across
  all graph-domain replays, temporal vehicle movement, and the new domain scenes.

The larger Mission and Logistics profiles explicitly report responsive policies;
their Micro fixtures retain ISMCTS and MCTS. Evaluated policy comparisons are
labeled approximate and are not presented as exhaustive optima or estimates of
an untested planner's success probability.
