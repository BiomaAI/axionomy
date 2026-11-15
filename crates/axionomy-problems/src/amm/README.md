# Living Market AMM

The Living Market: endogenous price discovery in a closed economy.

Energy has value because actors need it to satisfy production and household
goals. Credit has value because it settles obligations and buys scarce
access to that energy. No external price or oracle exists: the AMM reserve
ratio is the economy's only public marginal price.

## Specification

Energy and settlement credit trade through a constant-product pool. The
founding reserve ratio is only an opening hypothesis: a generator supplies
energy, a factory and household buy it to satisfy encoded needs, an informed
speculator and demand whale buy for different reasons, an adaptive provider
changes market depth, and an internal arbitrageur sells into the resulting
scarcity. Credit remains useful because it settles encoded obligations;
energy remains useful because consuming it produces goal and utility assets.
There is no external market, stablecoin peg, or oracle.

Every swap is an exact state-evaluated rate law. The kernel reads pre-exchange
reserves, evaluates a full-width mul-div rounded down for the trader with a 0.3% fee, checks the caller's
minimum output, applies both reserve and trader deltas atomically, and emits a
replayable receipt. Proportional liquidity deposits preserve price while
changing depth. Withdrawals burn shares against exact pre-state ownership.

Showcase and Stress are seasons (`season.rs`). Actors repeatedly choose from
live reserves, stock, remaining credit, current needs, and encoded
closing-price memory. Each has six economic decisions per day and may finish
trading early. Days advance only after every participant has finished. Unmet
needs and unpaid obligations are recorded before future entitlements become
current.

Each rate only one account may take fixes that role's account: the pool,
the treasury, the information account, the calendar's world account, and
the actor whose need, obligation, production or income it is, so no identity
token pins a binding; a trader or liquidity provider is any actor. Ending an
actor's trading is one rate over every actor, `end-trading/{actor}`. Each
day but the last closes as one rate, `advance-day/{day}`, deriving the day
it opens, `{next}`; the season's weather never changes, so it is law
structure: a relation's row for each day gives the sunlight of the day it
opens, `{sunlight}`, and whether the day after falls short, `{shortage}`,
which the information account signals. Meeting a need is one rate over the
factory and the household, `use-energy/{actor}`, and settling an obligation
one over the generator and the factory, `settle-obligation/{actor}`, a
relation giving each actor's amounts: the energy its need takes, the
utility it yields and, in a season, the goods the factory makes (none for
the household), and what each obligation costs. Micro has 8 rates as
written, for 10 instances; Showcase and Stress 16, for 31 and 35. A
season's length and scale are the whole world's and never change, so they
are the economy's constants, `{season-length}` and `{scale}`: investment
reads the length, and the policies read both (`state.constant`).

Sunlight limits daily production. Investment pays credit now and increases
capacity two days later. Energy storage is bounded. Factory output produces
sellable goods and procurement pays from finite Treasury credit; household
income also has a finite counterparty. Liquidity providers may adjust once per
day, including deposits computed at the current ratio with at most one atom of
rounding per reserve. Direct attribution includes these rounding effects.
Counterfactuals retain the same seeded sunlight and entitlements while the
remaining actors respond to changed market conditions.

## Required results

The unit tests check the laws by applying exchanges:

- Independent `u128` quotes agree with kernel-computed swap receipts.
- Constant product never decreases during a swap and round trips cannot create
  credit; quotes are monotone in input and stay below the output reserve.
- Proportional deposits keep the price; seasonal entitlements, storage,
  production, investment and liquidity limits hold at their boundaries.

The package's experiments find the rest, pinned in `tests/outcomes`: Micro's
market day reaches the actors' goals; Showcase and Stress close their seasons
with every need and obligation satisfied or explicitly missed; the no-whale
and thin-liquidity counterfactuals discover different prices under the same
law; and the view ranks each actor's direct contribution to the price.

## API pressure

- Serializable state-dependent rates without opaque domain callbacks.
- Exact named exchange parameters and authoritative inequality conditions.
- Nonlinear effects while retaining assets, accounts, rates, and exchanges as
  the only semantic primitives.
- Causal attribution from replay evidence rather than external analytics
  state.
- A picture on Studio's neutral reserves surface, every caption written by
  the problem, that remains a projection of Rust-owned truth.

## In this folder

- `mod.rs` is the model: the pool and its swap and liquidity laws, the
  actors, their accounts and goals, and the Micro Market Day's economy.
- `season.rs` builds the Showcase and Stress seasons: repeated production,
  consumption and investment over the same exact AMM.
- `package.rs` writes its package: its economies under refs, goals,
  strategies, experiments and view, whose scripts are `scripts/amm.rhai`
  and `scripts/amm-view.rhai`.

The Living Market is one of the [conformance problems](https://github.com/BiomaAI/axionomy/blob/main/PROBLEMS.md).
