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
atomically refunded and rebooked. The larger planner is a bounded beam of 24
states with evaluated buyer/seller tradeoffs; Micro retains exact clearing.

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
order status are protected by declared invariants. Candidate enumeration
derives the buyer, seller, and carrier sets from economy accounts and produces
ordinary exchanges. Exact matching uses core applicability. Near matching
uses complete core shortfalls plus a caller-provided valuation; that scalar is
disposable policy and never changes settlement law or feasibility. A
disposable clearing search explores compatible applicable settlements,
maximizing encoded settled-order count and gross value. Its result is an
ordinary replayable exchange trace, not a second market state.

## Required results

- The account-derived Cartesian set contains 36 candidate exchanges.
- Five candidates are initially exact and filtering does not mutate the market.
- One candidate reports buyer, seller, and carrier shortfalls together.
- A successful exchange settles six accounts atomically.
- Projected assessment deltas exactly equal the resulting receipt deltas.
- Failed settlement preserves every account.
- Changing caller-owned shortfall weights changes near-match ranking without
  changing validity.
- Clearing selects two compatible settlements with gross value 190.
- The two-exchange clearing reaches both encoded order goals and replays
  deterministically.
- Exact Pareto clearing retains four participant-utility allocations covering
  the two eligible Order A buyers and two widget sellers; every entry replays.

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

- `mod.rs` is the model: the Micro market, its settlement rates, matching,
  ranking and clearing.
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
