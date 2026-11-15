# Cohort-indexed perishables

Cohort-indexed perishable inventory with explicit time and storage effects.

Ten thousand physical units are represented as fungible claims over two
cohorts. Each cohort has one unique condition fact, so spoilage changes a
shared fact rather than rewriting every claim holder. The package's outage
policy proposes each due exchange, but the economy remains the only authority
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
The cold chain's rates have variables: one law serves every batch and
place, `deliver/{batch}/true/{at}`, and every pair of batches a split takes
from and starts, `split/{source}/{destination}/{at}`, a relation over the
places giving the cold slots a crate there takes, none in the warehouse and
one in cold storage, which ages a batch warm only while the cooling is off.
A move between the two, `move/{batch}/{from}/{to}`, takes the cold space
where the batch goes and frees it where it was. Laws split only where
their shape differs: fresh or preserved crates, warm aging that spoils or
not. A day opens as `open-day/{day}`, a relation over the days giving the
grid's supply, three units of electricity but none on a day of the outage,
so the seed changes only those numbers, not the law; the policies read
them with `state.values`. Each role is fixed to its account, the world,
the buyer or the batch its identifier names, so no identity token pins a
binding, and a plan binds each exchange's roles to the accounts its rate
fixes.

The package's four daily policies share the same seeded outage schedule; their
front is explicitly the evaluated policy set. Micro is the storage fixture
described below.

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
effects. One law advances the clock, `advance/{from}/{to}`, a relation over
the moments pairing each with the one after it. Eating and spoiling stay
one rate per cohort and exposure: the storage facts, the moment and the
condition each requires are identifiers, not numbers a relation could give.

At ambient expiry, one spoil exchange changes the ambient cohort's unique
fresh condition into `Rotten`; its six thousand claims are untouched. A power
loss then atomically changes the fridge from cold/powered to
ambient/unpowered and reclassifies the refrigerated cohort from cold expiry to
an earlier warmed-expiry condition, which spoils at its own moment. A
cold-expiry spoil proposed later is rejected as infeasible because the
authoritative condition and environment no longer exist.

## Required results

The package's experiments find these on Micro's seventy warehouse and thirty
fridge claims, pinned in `tests/outcomes`:

- The outage, ten claims moved first, resolves in eight exchanges: every claim
  kept, none usable, ten cooling energy spent.
- The exact front of storage plans runs from all seventy claims moved, a
  hundred usable for seventy energy, to none moved, thirty usable for none.
- Every cold-chain policy closes Showcase and Stress.

The model's tests check its rules:

- Ten thousand claims occupy the same number of state entries as ten claims.
- A 1,000-unit move is one scaled multi-account exchange.
- Each cohort condition has unit supply; multi-unit or repeated spoilage is
  infeasible.
- Before/reached facts reject early decay and late consumption.
- Power loss touches no claim balance and changes one shared cohort fact; the
  replaced cold condition can no longer spoil.
- Incorrect location, cohort, or World bindings are rejected by encoded
  identities and conditions.
- The outage replays to the encoded goal and becomes quiescent, and over 128
  generated inventories and transfers it agrees with a plain independent
  inventory oracle.
- A sealed storage plan keeps what it moved cold, for its cooling energy.

## API pressure

- Structural fungibility through shared asset identity and quantity.
- Non-fungible facts through unique identity, unit supply, and invariants.
- Event-driven temporal effects without mutation-on-read or an external clock.
- Stale proposal rejection by core assessment.
- Work proportional to shared-fate cohorts rather than physical units.
- Incremental touched-account commit and receipt-delta invariant validation
  without weakening atomicity.
- Bounded policy discretization can coexist with an arbitrary-unit fungible
  exchange rate.

## In this folder

- `mod.rs` is the model: the Micro storage fixture, its clock, spoilage,
  outage and storage-plan seal.
- `cold_chain/` is the Showcase and Stress cold chain, an ontology of its own
  laid out the same way: its model in `mod.rs`, and its scripts
  `scripts/cold-chain.rhai` and `scripts/cold-chain-view.rhai`.
- `package.rs` writes its package: its economies under refs, goals,
  strategies, experiments and view, whose scripts are `scripts/perishables.rhai`
  and `scripts/perishables-view.rhai`.

Perishables is one of the [conformance problems](https://github.com/BiomaAI/axionomy/blob/main/PROBLEMS.md).
