# Cohort-indexed perishables

Cohort-indexed perishable inventory with explicit time and storage effects.

Ten thousand physical units are represented as fungible claims over two
cohorts. Each cohort has one unique condition fact, so spoilage changes a
shared fact rather than rewriting every claim holder. A disposable event
agenda proposes due exchanges, but the economy remains the only authority
on timing, environment, identity, and effects.

The larger profiles use `cold_chain/`. Cohorts hold fresh,
preserved, and spoiled crates. Splitting copies accumulated heat and lineage;
moving into refrigeration never resets prior exposure. Cold capacity is a
conserved physical resource, including space occupied by spoiled food until
disposal. Daily cooling costs electricity. Processing consumes electricity
and labor; deliveries consume transport capacity and fuel. Backup generation
converts that same fuel into stored electricity, so recovery can reduce later
delivery capacity. Buyers fund fresh/preserved food at different prices.
Every day settles each cohort's exposure before advancing the clock and
records unfilled food obligations. Showcase spans six days with a two-day
outage; Stress tightens storage/fuel over eight days with a three-day outage.
Four responsive policies share the same seeded outage schedule. Their front
is explicitly the evaluated policy set, while Micro retains the exact storage
fixture described below.

## Specification

Warehouse and fridge accounts hold ten thousand fungible fruit claims in two
cohorts. The claim quantity is stable ownership or custody state; it does not
duplicate freshness on every unit. Each cohort account instead holds one
unique condition fact:

```text
Warehouse:       Claim(Ambient) = 7,000, Ambient
Fridge:          Claim(Refrigerated) = 3,000, Cold, Powered, CoolingEnergy = 7,000
Ambient cohort:  Fresh(AmbientExpiry) = 1
Cold cohort:     Fresh(ColdExpiry) = 1
```

A single exchange with multiplicity 1,000 moves claims from the ambient cohort
to the refrigerated cohort while preserving both location identities, both
cohort identities, cold power, freshness, and the open transfer window. It
also converts `CoolingEnergy` into `SpentCoolingEnergy`; it does not fire one
thousand transitions.

Time is explicit economic state. The World account owns exactly one `Now`
fact, plus complementary `Before(deadline)` and `Reached(deadline)` facts.
Advancing the clock consumes the applicable `Before` fact and produces its
`Reached` fact. Consequently fruit cannot spoil early, but it also cannot be
eaten or moved after expiry while a simulator is still materializing due
effects.

At ambient expiry, one spoil exchange changes the ambient cohort's unique
fresh condition into `Rotten`; its six thousand claims are untouched. A power
loss then atomically changes the fridge from cold/powered to
ambient/unpowered and reclassifies the refrigerated cohort from cold expiry to
an earlier warmed-expiry condition. The event agenda schedules that newly
produced condition. Its old cold-expiry candidate remains in the disposable
queue but is later rejected as infeasible because the authoritative condition
and environment no longer exist.

The `ClaimIndex` is a receipt-maintained `cohort → account → quantity`
projection. It provides holder and aggregate-supply queries without becoming
truth: it can be rebuilt from the economy, and tests compare the incremental
index to a full rebuild. The `EffectAgenda` similarly indexes fresh condition
facts and only proposes ordinary spoil exchanges.

## Required results

- Ten thousand claims occupy the same number of state entries as ten claims.
- A 1,000-unit move is one scaled multi-account exchange.
- Each cohort condition has unit supply; multi-unit or repeated spoilage is
  infeasible.
- Before/reached facts reject early decay and late consumption independently
  of event-loop discipline.
- Power loss touches no claim balance and changes one shared cohort fact.
- The newly warmed cohort spoils at its earlier deadline.
- The queued cold event becomes stale and is rejected by core assessment.
- Incorrect location, cohort, or World bindings are rejected by encoded
  identities and conditions.
- The complete outage trace replays to the encoded goal and becomes quiescent.
- 128 generated inventory splits and transfers agree with a plain independent
  inventory oracle.
- The receipt-maintained holdings index always equals a complete rebuild.
- Bounded exact planning evaluates zero through seven 1,000-claim transfers and
  retains all eight preservation/cooling-energy tradeoffs as replayable traces.

## API pressure

- Structural fungibility through shared asset identity and quantity.
- Non-fungible facts through unique identity, unit supply, and invariants.
- Semantic cohort indirection versus disposable physical indexes.
- Event-driven temporal effects without mutation-on-read or an external clock.
- Trigger indexes, stale proposal rejection, and receipt-driven projections.
- Work proportional to shared-fate cohorts rather than physical units.
- Incremental touched-account commit and receipt-delta invariant validation
  without weakening atomicity.
- Bounded policy discretization can coexist with an arbitrary-unit fungible
  exchange rate.

## In this folder

- `mod.rs` is the model: the Micro storage fixture, its clock, spoilage and
  outage, the `ClaimIndex` and `EffectAgenda`, and the storage plans.
- `cold_chain/` is the Showcase and Stress cold chain, an ontology of its own
  laid out the same way: its model in `mod.rs`, and its scripts
  `scripts/cold-chain.rhai` and `scripts/cold-chain-view.rhai`.
- `package.rs` writes its package: its economies under refs, goals,
  strategies, experiments and view, whose scripts are `scripts/perishables.rhai`
  and `scripts/perishables-view.rhai`.

[`examples/perishables.rs`](https://github.com/BiomaAI/axionomy/blob/main/crates/axionomy-problems/examples/perishables.rs) runs it:

```console
cargo run -p axionomy-problems --example perishables
```

Perishables is one of the [conformance problems](https://github.com/BiomaAI/axionomy/blob/main/PROBLEMS.md).
