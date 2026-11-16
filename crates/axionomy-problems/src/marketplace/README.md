# Multi-party marketplace settlement

Assessment-driven matching and atomic multi-party market settlement.

Candidate enumeration and ranking are derived, disposable policy. The
economy remains the authority for participant state, settlement terms,
feasibility, effects, and completed orders.

Showcase and Stress use `fulfillment.rs`. Contracts hold the actual
goods and buyer funds in an order account. Buyer, seller, and carrier commitment
tokens bind later dispatch, delivery, and refund to those same participants.
Each carrier holds one shipment at a time and regains capacity on delivery.
Orders arrive in waves; fast shipping is exposed to known route delays, while
the slower carrier avoids them. A ten-credit price concession increases buyer
benefit and reduces seller benefit. Late delivery returns ten credits to the
buyer from the seller's proceeds. An unshipped reservation expires and can be
atomically refunded and rebooked. The package plans these larger markets with
a bounded beam of 24 states, scored for volume or weighted toward buyers or
sellers; Micro's clearing is exact.

## Specification

Three buyer accounts, three seller accounts, two carrier accounts, and two
order accounts form a bounded market. Participant identity comes from account
IDs; eligibility comes only from account assets. Order A requests a widget for
100 and Order B requests a gadget for 90. Buyer A can fund both, Buyer B can
fund Order A, and Buyer C is 25 short on Order A. Two sellers offer widgets,
one offers a gadget, Carrier A owns two units of capacity, and Carrier B has no
capacity.

Settling is one rate over every order, `settle-order/{order}`, binding the
same six distinct roles; for order A, of a widget:

```text
Buyer pays 100 and receives widget + receipt
Seller provides widget + offer and receives 80
Platform receives 5
Tax authority receives 10
Carrier spends one capacity and receives 5
Order account converts matching OpenOrder into SettledOrder + SettledValue
```

An order's terms never change, so they are law structure: a relation's
row for each order gives the item it trades, a word the rate writes into
`item/{item}`, and the numbers of its terms, its gross price, the seller's
proceeds, the tax, the commission, the shipping fee and the utility its
buyer and seller gain, which the rate names as `{gross}`, `{proceeds}` and
so on. No identifier names the item: `settle-order/a` trades a widget
because order A's row says so.

The institutions are fixed accounts: every rate fixes its platform role to
the platform, its tax authority to the tax authority and its order role to
the order's own account, so no licence or policy token pins them. The
buyer, seller and carrier are the exchange's to choose, distinct from one
another and from the institutions.

The larger markets have six rates as written. A contract, at the order's
price or ten credits less, is one rate, `contract/{order}/{discounted}`,
and so is its cancellation, `cancel/{order}/{discounted}`, a relation on
`{discounted}` giving the concession and `{price}` derived from it. A
delivery is two, `deliver/{order}/{discounted}/false` on time and
`.../true` late with a rebate: the two meet the deadline differently and
record a different outcome. Each order's schedule, when it arrives, its
deadline and the delay its route meets, is another relation's row, and
each carrier's transit time and exposure to delays another's, so a
dispatch names the carrier that booked it, `dispatch/{order}/{carrier}`;
the clock is one rate over every tick, `advance/{to}`. Micro has one rate
as written, for 2 instances; Showcase and Stress six, for 72 and 92. The
market has no constants: what the platform holds for the whole market,
the time and the time it has left, changes as the clock advances. The
scripts read an order's terms, its item among them, from the instance
agreeing to it at its price (`state.values`), never from a table of their
own, and where the market's day ends from the time the platform has left.

The settlement terms also produce declared `Utility` assets for the bound
buyer and seller. They make participant benefit part of the outcome rather
than a clearing callback.

Money, widgets, buyer and seller lifecycle tokens, shipping capacity, and
order status are protected by declared invariants. The buyer, seller and
carrier are roles each rate leaves free, and the engine binds each to every
account holding what the role takes (D-061): Micro's settlements are the
engine's list, and the market's script keeps only its choices, a dispatch
or delivery first, alone, and the clock advanced to the next event. Core
applicability filters every candidate; a refused settlement
reports every party's complete shortfall. Its disposable clearing search
explores compatible applicable settlements, minimizing the gross value a
clearing forgoes, and its Pareto picks rank the exact front of participant
utility. Each result is an ordinary replayable exchange trace, not a second
market state.

## Required results

The unit tests check the laws by applying exchanges:

- Of the 36 settlement bindings the accounts allow, five apply at first, and
  assessing them does not mutate the market.
- One settlement reports buyer, seller, and carrier shortfalls together.
- A successful exchange settles six accounts atomically, and projected
  assessment deltas exactly equal the resulting receipt deltas.
- Failed settlement preserves every account.
- Two compatible settlements with gross value 190 reach both encoded order
  goals and replay deterministically.
- A signed contract locks money, goods and capacity until delivery; an
  expired reservation refunds every party and can be rebooked.

The package's experiments find these, pinned in `tests/outcomes`:

- Micro's clearing settles both orders in two exchanges, and its Pareto
  picks each take an allocation from the exact front of participant utility.
- Showcase and Stress fulfil every order, planned for volume or toward buyers
  or sellers.

## API pressure

- Atomic settlement beyond bilateral exchange, including tax, commission, and
  fulfillment.
- Complete multi-account distance to feasibility as matching information.
- Separation between authoritative economic truth and disposable ranking
  policy.
- Finite role-binding enumeration derived from account capabilities.
- Global compatibility search over sequential multi-order settlement.
- Ergonomic all-distinct role constraints for larger joint transactions.
- Conservation and lifecycle invariants across heterogeneous participants.
- Participant-level allocation fronts without an external market state.

## In this folder

- `mod.rs` is the model: the markets' parties and orders, the Micro market,
  its settlement rates, goals and typed exchanges.
- `fulfillment.rs` builds the Showcase and Stress markets: funded offers
  become reservations, shipments and only then settlements.
- `package.rs` writes its package: its economies under refs, goals,
  strategies, experiments and view, whose scripts are `scripts/marketplace.rhai`
  and `scripts/marketplace-view.rhai`.

The marketplace is one of the [conformance problems](https://github.com/BiomaAI/axionomy/blob/main/PROBLEMS.md).
