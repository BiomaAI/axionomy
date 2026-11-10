# Ontologies

An ontology is the vocabulary of one kind of economy: **who holds** things, **what is
held**, **the laws** by which holdings change, and **the parts** of a law that an
exchange binds to holders. In Axionomy these are four Rust types, the
ontology's accounts, assets, rates and roles. The ontology names the four together,
under a path.

Everything else is written in that vocabulary. An `Economy<O>` holds `O::Account`s
with balances of `O::Asset`s, and it applies the rates it names by `O::Rate`. An
`Exchange<O>` names one of those rates and binds each of its `O::Role`s to an
account. Receipts, traces, goals, stored runs and views are all of an ontology.

The ontology is the problem's model. The kernel owns everything else: balances,
the laws' effects, validation, conservation and history (D-003). Building a
problem in Axionomy therefore means designing its ontology and writing its
rates. This guide covers what the kernel requires, how to build an ontology
step by step, and the modeling choices the canonical problems settled on.

## The four kinds

| Kind | Question | Examples from `axionomy-problems` |
| --- | --- | --- |
| Account | Who or what holds state? | an agent, a vehicle, a warehouse cell, an order, the world, Nature |
| Asset | What is held, in what quantity? | fuel, money, a key, a location (`At(Room)`), an identity, a fact (`Unresolved`), time |
| Rate | What laws change holdings? | move, load, bake, bid, settle, a chance outcome resolving |
| Role | What parts does a law have? | mover, payer, payee, buyer, seller, Nature |

The four stay distinct types, so an account can never stand where an asset or a
role is expected. That holds even when all four are paths, as in the
[`paths`](#identifiers-as-data) ontology.

## The shape in code

```rust
pub trait Ontology: Copy + Debug + Ord + Hash + Send + Sync + 'static {
    /// The ontology's name, a path such as `maze` or `logistics/fleet`.
    const NAME: &'static str;
    type Account: Identifier;
    type Asset: Identifier;
    type Rate: Identifier;
    type Role: Identifier;
}
```

- **An ontology is a marker type**, usually an empty enum, and is never a
  value. It derives the standard traits so that the kernel's types, such as
  `Economy<O>` and `Exchange<O>`, can derive theirs.
- **`Identifier`** is what the kernel needs of an identifier type: `Clone`,
  `Eq`, `Ord`, `Hash` and `Debug`. Ordering makes everything the kernel derives
  deterministic: state keys, receipts, roots and view keys. Derive it with
  `#[derive(Identifier)]`, which also writes the type as its path and orders it
  as that path ([below](#identifiers-are-written-as-paths)).
- **`NAME` is a path** of lowercase kebab-case segments. It is checked when
  compiling: wherever the ontology is used, `OntologyPath::of::<O>()` refuses to
  compile a name that is not a path. Storage records the name with every run,
  and views key every value under it.
- **A kernel type that mentions more than one kind** takes the ontology:
  `Economy<O>`, `EconomyBuilder<O>`, `Exchange<O>`, `Receipt<O>`, `Trace<O>`,
  `Goal<O>`, `Rate<O>`, `ApplyError<O>`. **A value of one kind** keeps that kind:
  `Basket<A>`, `Account<A>`, `AssetAmount<A>`, `LinearInvariant<A>` (D-042).

## Building one

The example is a small bakery. Each step names the decision being made, and
the complete program at the end compiles and runs.

### 1. Accounts: who holds

Every place that state lives is an account: actors, containers, facilities,
orders, the world itself, and Nature. Ask what a balance belongs to. Flour
belongs to the baker, the baking slot to the oven, and coins to a customer.

```rust
use axionomy::Identifier;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Identifier)]
enum Holder {
    Baker,
    Oven,
    Customer(u8),
}
```

Variants with data, such as `Customer(u8)`, give a family of accounts one
variant, written `customer/0`, `customer/1` and so on. Exchanges can open and
close accounts, so a family can grow and shrink at runtime: an order is
placed, a position is opened, a firm enters the market. The exchange names
each new account. The kernel never invents identifiers in your ontology
(D-039).

### 2. Assets: what is held

An asset is anything with a quantity. Quantities are exact non-negative 256-bit
integers, with no floats and nothing that rounds silently.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Identifier)]
enum Good {
    /// Grams of flour.
    Flour,
    Dough,
    Bread,
    Coin,
    /// The oven's one baking slot: held, never used up.
    Slot,
}
```

Assets carry far more than goods, and the canonical problems lean on this:

- **Facts are assets.** Location (`At(Room)`), a phase of play (`Planning`,
  `AwaitingObservation`), an unresolved chance outcome (`Unresolved`), and a
  goal reached (`Solved`) are all balances of 0 or 1. They are state the laws
  can read, never flags kept outside the economy (D-001, D-008).
- **Time is an asset.** Elapsed time, deadlines and windows are balances, so a
  late action is an ordinary infeasible exchange (D-029).
- **Fungibility is structural** (D-030). One asset with quantity 10,000 is ten
  thousand interchangeable units. A unique item is its own asset identity with
  a supply of one, such as `Package(OrderId)`. When many holders share a
  condition, point their claims at one cohort asset, so that one exchange can
  age ten thousand units at once.
- **Identity is an asset.** An account that holds `AgentIdentity(AgentId::A)`
  is agent A as far as any law can tell. A rate that preserves that identity
  on its role cannot be bound to anyone else's account; bridge's impersonated
  bid is rejected this way.
- **Units belong to the asset** (D-025). An asset has one atomic basis: flour
  is counted in grams, and nothing else can count it in kilograms. With
  `axionomy-units`, the ontology's asset type is `UnitAsset<Id>`. Typed inputs
  (`1 kg`, `100 g`) lower into atoms of that one basis when amounts are
  written, so taking 100 g from 1 kg is exact subtraction of atoms: 1,000 less
  100 leaves 900. A value that is not a whole number of atoms is refused rather
  than rounded. Two denominations that are meant to differ are two assets,
  connected by an explicit rate. `decode` turns atoms back into a typed value,
  and a view's `QuantityFormat` says how people read them (`0.9 kg`,
  `12.50 USD`, `5 %`) without becoming part of the asset's identity.

### 3. Rates: the laws

A rate is a law over roles. Per unit of the exchange, each role's account
**consumes** a basket, **produces** a basket, or **preserves** one: it must hold
the preserved basket, which is read once and never used up (D-006).

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Identifier)]
enum Law {
    Knead,
    Bake,
    Sell,
}
```

```rust
Rate::new()
    .consume(Part::Seller, basket([(Good::Bread, 1)]))
    .produce(Part::Buyer, basket([(Good::Bread, 1)]))
    .consume(Part::Buyer, basket([(Good::Coin, 3)]))
    .produce(Part::Seller, basket([(Good::Coin, 3)]))
    .distinct(Part::Seller, Part::Buyer)
```

Rates can also:

- compute amounts from state with exact integer expressions that name their
  rounding (`produce_computed`, `QuantityExpression::mul_div`);
- require conditions over balances (`condition`);
- require two roles to be bound to different accounts (`distinct`);
- open or close the accounts bound to a role (`opens`, `closes`).

**A rate is a law, not an event, and roles say who.** `Sell` is one law for
every customer; each exchange binds `Buyer` to whichever customer is buying.
Put data in a rate's identifier only when the law itself differs. For example,
`Move { from, to }` consumes `At(from)` and produces `At(to)`, so every pair of
rooms needs its own law. Sokoban's `Finish` is the other side: whichever goal
cells the crates rest on, finishing is one law, and each exchange binds every
crate's `Goal` role to the cell that crate is on.

### 4. Roles: the parts of a law

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Identifier)]
enum Part {
    Baker,
    Oven,
    Seller,
    Buyer,
}
```

A role is checked only by what its account holds. If a law needs "the oven",
the `Oven` role preserves the oven's slot, so binding any other account fails.
If a law needs "agent A", the role preserves A's identity. Partial
information works the same way. An actor proposes what it can observe, and
Nature, an account with its own role, resolves the hidden part in a separate
exchange (D-020).

### 5. Name it

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Bakery {}

impl Ontology for Bakery {
    const NAME: &'static str = "bakery";
    type Account = Holder;
    type Asset = Good;
    type Rate = Law;
    type Role = Part;
}
```

### 6. Build, propose, apply

The economy is built once. After that, exchanges are the only way anything
changes (D-002).

```rust
let mut economy = EconomyBuilder::<Bakery>::new()
    .account(Holder::Baker, Account::from(basket([(Good::Flour, 1_000)])))
    .account(Holder::Oven, Account::from(basket([(Good::Slot, 1)])))
    .account(Holder::Customer(0), Account::from(basket([(Good::Coin, 5)])))
    .rate(
        Law::Knead,
        Rate::new()
            .consume(Part::Baker, basket([(Good::Flour, 500)]))
            .produce(Part::Baker, basket([(Good::Dough, 1)])),
    )
    .rate(
        Law::Bake,
        Rate::new()
            .consume(Part::Baker, basket([(Good::Dough, 1)]))
            .produce(Part::Baker, basket([(Good::Bread, 1)]))
            .preserve(Part::Oven, basket([(Good::Slot, 1)])),
    )
    .rate(
        Law::Sell,
        Rate::new()
            .consume(Part::Seller, basket([(Good::Bread, 1)]))
            .produce(Part::Buyer, basket([(Good::Bread, 1)]))
            .consume(Part::Buyer, basket([(Good::Coin, 3)]))
            .produce(Part::Seller, basket([(Good::Coin, 3)]))
            .distinct(Part::Seller, Part::Buyer),
    )
    .invariant(LinearInvariant::new(path!("coins")).weight(Good::Coin, 1))
    .build()?;

// Two units of kneading use 1,000 g; two units of baking read the one slot once.
let knead = Exchange::new(Law::Knead, Quantity::new(2)).bind(Part::Baker, Holder::Baker);
let bake = Exchange::new(Law::Bake, Quantity::new(2))
    .bind(Part::Baker, Holder::Baker)
    .bind(Part::Oven, Holder::Oven);
let sell = Exchange::new(Law::Sell, Quantity::new(1))
    .bind(Part::Seller, Holder::Baker)
    .bind(Part::Buyer, Holder::Customer(0));
for exchange in [knead, bake, sell] {
    economy.apply(exchange)?;
}

let fed = Goal::new().require(Holder::Customer(0), basket([(Good::Bread, 1)]));
assert!(economy.matches(&fed));

// A second sale is infeasible, and the assessment names the exact shortfall.
let again = Exchange::new(Law::Sell, Quantity::new(1))
    .bind(Part::Seller, Holder::Baker)
    .bind(Part::Buyer, Holder::Customer(0));
let assessment = economy.assess(&again);
assert_eq!(
    assessment.shortfall(&Holder::Customer(0)),
    Some(&basket([(Good::Coin, 1)]))
);
```

The invariant declares that coins are conserved. Every exchange that would
create or destroy coins is rejected, including one built from a rate written by
mistake (D-005). The goal is economic state as well: `matches` reads balances
and consults nothing outside the economy.

## Modeling choices

- **If it matters, it is state.** Goals, chance, hidden truth, time, capacity
  and phases of play are balances in accounts. A solver, simulator or agent
  only proposes exchanges; it never decides what one does (D-007, D-021).
- **Conservation is declared.** Say what is conserved with a
  `LinearInvariant`, whose weights may be negative, as in assets minus
  liabilities. The kernel checks every exchange against it exactly.
- **Rounding is named.** A computed amount that divides names its rounding.
  Round amounts a participant pays up and amounts a participant receives down,
  so dust stays with the counterparty and invariants still hold.
- **One vocabulary, one ontology.** When a problem grows a different
  vocabulary, it gets a different ontology under its own name, even if it
  reuses type names. The fleet variant of logistics has vans, permits, dock
  and road capacity and a world account, so it is `logistics/fleet`, not
  `logistics`. Its values are
  keyed `logistics/fleet/account/…` and can never be confused with the
  single-vehicle problem's.
- **Keep the vocabulary closed and small.** Enums with data
  (`Order(OrderId)`, `At(Location)`) scale to large instances without growing
  the type. Instances are values, built from configuration, while the
  vocabulary stays fixed.

## Where the ontology is used

| Where | What it uses |
| --- | --- |
| Kernel | `Economy<O>`, `Exchange<O>`, `Trace<O>`, …: the four kinds, kept distinct |
| Search | Generic over `O`: graph search, MCTS, ISMCTS, Pareto search and rollouts all read and propose through the kernel |
| Storage (`axionomy-store`) | Any ontology whose identifiers serialize (`Recordable`). A run records `O::NAME`, is read back only as that ontology, and every identifier is stored as its JSON: for a derived identifier, its path. |
| Views (`axionomy-view`) | `ViewOntology<O>`: how views show `O`, meaning keys, labels, scenes and leaderboards. `DebugOntology<O>` keys each value by its written path under the ontology's name, for example `bakery/account/customer/0`, and labels it from its `Debug` output. |
| Units and time | `axionomy-units` and `axionomy-time` author typed quantities and lower them into atoms of an ontology's assets |
| MCP and remote clients | The `paths` ontology, where every identifier is data |

`#[derive(Identifier)]` writes `Serialize` and `Deserialize` too, so the four
types need nothing more for a store to record the ontology's runs.

### Identifiers are written as paths

`#[derive(Identifier)]` gives an identifier type one written form, its path,
and one order, the order of that path (D-044):

- A variant is its kebab-case name followed by its fields in order, and a
  struct is its fields in order: `Customer(0)` is `customer/0`,
  `Move { from: Start, to: Exit }` is `move/start/exit`, and `JobId(3)` is `3`.
- A field is another identifier, an unsigned integer, a `bool` (`true` or
  `false`) or an `Option` of one, written `none` or `some` followed by the
  value. Anything else, such as a signed integer, text, a float or a
  collection, has no written form and does not compile; say it another way,
  usually as bindings or as more identifiers.
- The order is the path's natural order: segment by segment, numbers by value
  (`customer/2` before `customer/10`) and before words, and a path before the
  longer paths it starts. Variants sort by name, not by where they are
  declared.

Derive `Debug`, `Clone`, `PartialEq`, `Eq` and `Hash` (and `Copy` when it
fits) as usual, but not `PartialOrd`, `Ord`, `Serialize` or `Deserialize`: the
derive writes them. Since an identifier's order is its path's, never rely on it
for a domain order. If moments come in time order, give them a chronology and
sort by it.

A typed identifier and its path sort alike, so every map the kernel keeps is in
the same order either way: a typed economy written down is a
[`paths`](#identifiers-as-data) economy byte for byte, and tools that speak only
paths, such as MCP, can read and explore it.

### Identifiers as data

`axionomy::paths::Paths` is the ontology whose accounts, assets, rates and
roles are all paths (`AccountPath`, `AssetPath`, `RatePath`, `RolePath`). It
fits an economy written down entirely as data: by a remote client over MCP, by
storage, or by a problem defined without Rust types of its own. The four kinds
still stay distinct, so an `AssetPath` never stands where an `AccountPath` is
expected. Choose `Paths` when the vocabulary arrives at runtime. Choose your
own types when it is part of your program, so that the compiler checks every
law you write. Derived identifiers are written as paths, so an economy of your
own types, written down, also reads as a `Paths` economy.

## Checklist

- [ ] Four types: account, asset, rate and role, each deriving `Debug`,
      `Clone`, `PartialEq`, `Eq`, `Hash` (`Copy` when it fits) and
      `Identifier`, whose fields are identifiers, unsigned integers, `bool` or
      `Option`.
- [ ] A marker, `enum MyOntology {}`, deriving `Debug`, `Clone`, `Copy`,
      `PartialEq`, `Eq`, `PartialOrd`, `Ord` and `Hash`, with a path `NAME`
      that is unique among your ontologies.
- [ ] Every fact a law reads is an asset; nothing a law needs lives outside
      the economy.
- [ ] Rates are laws over roles; who takes part is bound by each exchange.
- [ ] Nothing relies on an identifier's order for a domain order.
- [ ] Roles that must be a particular account preserve that account's
      identity asset.
- [ ] Conserved quantities are declared as invariants.
- [ ] Each measured asset has one atomic basis, and amounts are written
      through `axionomy-units` or `axionomy-time` when they come in other
      units.
- [ ] Goals are `Goal<O>` over balances.
