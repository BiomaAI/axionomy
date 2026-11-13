# Lending market

Lending market: a collateralized bridge loan through a borrow index.

## Specification

A borrower owes a supplier's bill before its revenue arrives, and finances
the gap from one pooled market. Depositors hold vault shares. The borrower
posts collateral and draws cash as scaled debt shares, up to a loan-to-value
share of the collateral's value. A clock closes one period at a time. The
interest rate follows utilization along a kinked curve and accrues by growing
one borrow index instead of touching every account, and a reserve cut of each
period's interest stays with the market. An oracle lowers the collateral price
once, at a scheduled period: the clock cannot pass that period before it. A
period cannot close while the loan is past its liquidation threshold; the
borrower may cure it by posting collateral or repaying, or a keeper repays it
and seizes collateral plus a bonus. Market terms, the index, reserves and the
price are balances, never caller parameters. One thousand genesis shares are
permanently locked with matching seed cash, so no first depositor can set the
share price.

Every division is an explicit full-width `MulDiv` rounded against the caller.
Amounts a participant pays round up and amounts a participant receives round
down, so rounding dust accrues to the market and no round trip can create
value:

```text
Deposit       shares = ⌊cash · supply ÷ assets⌋              (receives: floor)
Redeem        cash   = ⌊shares · assets ÷ supply⌋            (receives: floor)
Borrow        debt   = ⌈cash · WAD ÷ index⌉                 (owes: ceil)
Repay         debt   = ⌊cash · WAD ÷ index⌋ retired          (retires: floor)
RepayShares   cash   = ⌈debt · index ÷ WAD⌉                  (pays: ceil)
limits        ⌊posted · price · term ÷ WAD⌋                  (granted: floor)
utilization   ⌈debt value · WAD ÷ (cash + debt value)⌉       (owed grows: ceil)
rate          base + ⌈min(u, kink) · low ÷ WAD⌉
                   + ⌈(max(u, kink) − kink) · high ÷ WAD⌉    (owed grows: ceil)
Accrue        index += ⌈index · rate ÷ WAD⌉                  (owed grows: ceil)
              reserves += ⌈interest · reserve factor ÷ WAD⌉  (dust to market)
Liquidate     seized = min(posted, ⌊⌊owed · (1 + bonus)⌋ ÷ price⌋)
assets        = cash + ⌊debt supply · index ÷ WAD⌋ − reserves
```

Cash, collateral (free plus posted), vault shares (holders plus locked
against supply) and debt shares (holders against supply) are declared
invariants. Identity witnesses bind the market, clock, oracle and borrower;
the supplier's invoice and the customer's purchase orders bind the
counterparties.

The search policy borrows exactly the bill's shortfall while it is unpaid,
repays everything owed or all available cash after, and returns collateral
once the loan is closed. These are candidate choices, not semantics; the
kernel still assesses every exchange. Dijkstra search charges each closed
period's interest to the plan; breadth-first search counts exchanges.

## Required results

- Least interest borrows later and repays earlier than fewest steps, and its
  Dijkstra cost equals the interest replayed from its trace (Micro 37 against
  56, Showcase 342 against 872). Both plans pay the bill, retire every debt
  share, return all collateral and conserve cash.
- With the loan repaid, the market's cash gain equals what the borrower paid
  beyond the bill and revenue; reserves are a strict part of it, and
  depositors' assets grow by exactly the rest.
- The rate is the kinked curve the view computes: 7.2% a period at 90%
  utilization, and the index grows by exactly that rate.
- Borrowing and withdrawal respect the loan-to-value limit exactly; nothing
  can be borrowed before collateral is posted.
- The price drop happens once, at its scheduled period, and the clock cannot
  pass it before.
- A safe loan cannot be liquidated. An unsafe one stops the clock until it is
  cured or liquidated; liquidation retires all debt and gives the keeper more
  collateral value than it repaid, up to what was posted.
- No round trip creates value: deposit-then-redeem returns at most the
  deposit, and borrow-then-repay costs at least the loan, from reachable
  states (property tested).
- A first-depositor inflation attack still leaves the victim with shares, a
  rounding loss below one share's price, and the attacker with most of the
  donation lost to the locked shares.
- Slippage bounds hold and dust deposits that would mint no shares are
  rejected instead of absorbed.

## API pressure

- Full-width `MulDiv` with explicit `Rounding` inside state-evaluated laws,
  including a kinked curve built from `minimum`, `maximum` and `subtract`.
- Fixed-point indexes and market terms as ordinary balances, without a special
  numeric type.
- Schedules and safety gates as rate conditions over balances, so time cannot
  pass an unsafe loan or a pending price drop.
- Rounding discipline as an economic safety property enforced by invariants.
- Derived valuation views remain disposable; the rates are the authority.

## In this folder

- `mod.rs` is the model: the market's terms, rates and every instance, the
  search policy, the scripted liquidation and first-depositor attack, and
  the valuation views.
- `package.rs` writes its package: its economies under refs, goals,
  strategies, experiments and view, whose scripts are `scripts/lending.rhai`
  and `scripts/lending-view.rhai`.

[`examples/lending.rs`](https://github.com/BiomaAI/axionomy/blob/main/crates/axionomy-problems/examples/lending.rs) runs it:

```console
cargo run -p axionomy-problems --example lending
```

The lending market is one of the [conformance problems](https://github.com/BiomaAI/axionomy/blob/main/PROBLEMS.md).
