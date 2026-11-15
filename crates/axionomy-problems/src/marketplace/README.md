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

Each order-specific settlement rate binds the same six distinct roles:

```text
Buyer pays 100 and receives widget + receipt
Seller provides widget + offer and receives 80
Platform preserves its license and receives 5
Tax authority preserves its policy and receives 10
Carrier spends one capacity and receives 5
Order account converts matching OpenOrder into SettledOrder + SettledValue
```

The settlement terms also produce declared `Utility` assets for the bound
buyer and seller. They make participant benefit part of the outcome rather
than a clearing callback.

Money, widgets, buyer and seller lifecycle tokens, shipping capacity, and
order status are protected by declared invariants. The package's candidates
derive the buyer, seller, and carrier sets from economy accounts and propose
ordinary exchanges, which core applicability filters; a refused settlement
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

[`examples/marketplace.rs`](https://github.com/BiomaAI/axionomy/blob/main/crates/axionomy-problems/examples/marketplace.rs) runs it:

```console
cargo run -p axionomy-problems --example marketplace
```

The marketplace is one of the [conformance problems](https://github.com/BiomaAI/axionomy/blob/main/PROBLEMS.md).
