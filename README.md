# Axionomy

> A closed economic state machine for verifiable problem solving.

**Everything is an asset. Every change is an exchange.**

Axionomy treats every problem as an economy: assets represent anything that
can exist or matter—resources, facts, permissions, beliefs, goals, or
state—while accounts define where those assets belong, rates define the laws
by which they may change, and exchanges are the only events that make those
changes real. Search algorithms, optimizers, simulations, and policies may
explore this economy and propose exchanges, but they never own its truth; the
economy remains the single authoritative model, making every accepted decision
inspectable, verifiable, and replayable.

Because every transition exposes explicit preconditions, shortfalls, consumed
resources, preserved facts, produced outcomes, and invariant violations,
Axionomy provides much denser feedback than a sparse success-or-failure
signal—especially for reinforcement learning, where it can supply valid-action
masks, intermediate progress, resource costs, and structured failure reasons.
The same encoding can also serve graph search, optimization, Monte Carlo
simulation, and learned policies without duplicating domain logic; exact forks
enable counterfactual exploration, while replayable traces make decisions
auditable, comparable, and independently verifiable.

Axionomy is a Rust engine for encoding bounded problem spaces through four
primitives:

| Primitive | Meaning |
| --- | --- |
| Asset | A resource, fact, proposition, capability, condition, memory item, or state token |
| Account | An owner, actor, location, scope, or namespace |
| Rate | A law describing what may be consumed, produced, and preserved |
| Exchange | One concrete, role-bound firing of a rate |

The non-negotiable rule is:

> Nothing semantically authoritative may live outside assets, accounts, rates,
> and exchanges.

There is no parallel `WorldState`. Position, topology, time, goals, machine
capacity, uncertainty, beliefs, random seeds, bids, and rewards are modeled in
the same closed state. Search and optimization algorithms may fork that state
and propose exchanges, but only the core can accept a transition.

```text
State       = Account × Asset → Quantity
Problem     = Initial accounts + available rates + declared invariants
Computation = A sequence of valid exchanges
Goal        = A required asset configuration
Solution    = A replayable exchange trace reaching that configuration
```

See [PDD.md](PDD.md) for the product and technical contract and
[PROBLEMS.md](PROBLEMS.md) for the conformance problems that drive the API.

## Workspace

The repository separates execution semantics, solver strategies, and domain
models into one-way workspace dependencies:

| Crate | Responsibility |
| --- | --- |
| `axionomy` | Pure closed-state validation and exchange execution kernel |
| `axionomy-search` | Non-authoritative BFS and best-first reference strategies |
| `axionomy-problems` | Canonical problem encodings, specialized proposers, and conformance tests |

```text
axionomy-search ──────→ axionomy
axionomy-problems ────→ axionomy
                    └─→ axionomy-search
```

The kernel does not depend on any solver or domain crate.
The packages are independently publishable and must be released in dependency
order: `axionomy`, then `axionomy-search`, then `axionomy-problems`.

## What is implemented

- User-defined asset, account, rate-ID, and role types.
- Atomic multi-account rewrite rates.
- Separate consume, produce, and preserved/read-only baskets per role.
- Explicit exchange role bindings and multiplicity.
- Checked `u64` arithmetic and exact balance shortfalls.
- Global declared linear invariants checked on every firing.
- Asset-configured goals.
- Isolated forks, speculative execution, and deterministic trace replay.
- Account-restricted economic views.
- Generic BFS and best-first search in `axionomy-search`.
- Seven closed benchmark encodings in `axionomy-problems`, with independent
  solver strategies and core-encoded stochastic priors.
- No third-party runtime dependencies.

The core contains no application ontology or search algorithm. Problem assets
and specialized solvers compile against the same public kernel API available
to users.

## Core example

```rust
use axionomy::{
    Account, EconomyBuilder, Exchange, Goal, LinearInvariant, Quantity, Rate,
    basket,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum Asset {
    Raw,
    Finished,
    Tool,
    Solved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum AccountId {
    Workshop,
    Success,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum Role {
    Shop,
    Goal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum RateId {
    Build,
    Finish,
}

let mut economy = EconomyBuilder::new()
    .account(
        AccountId::Workshop,
        Account::from(basket([(Asset::Raw, 2), (Asset::Tool, 1)])),
    )
    .account(AccountId::Success, Account::default())
    .rate(
        RateId::Build,
        Rate::new()
            .consume(Role::Shop, basket([(Asset::Raw, 2)]))
            .preserve(Role::Shop, basket([(Asset::Tool, 1)]))
            .produce(Role::Shop, basket([(Asset::Finished, 1)])),
    )
    .rate(
        RateId::Finish,
        Rate::new()
            .preserve(Role::Shop, basket([(Asset::Finished, 1)]))
            .produce(Role::Goal, basket([(Asset::Solved, 1)]))
            .distinct(Role::Shop, Role::Goal),
    )
    .invariant(
        LinearInvariant::new("material")
            .weight(Asset::Raw, 1)
            .weight(Asset::Finished, 2),
    )
    .build();

economy.apply(
    Exchange::new(RateId::Build, Quantity::new(1))
        .bind(Role::Shop, AccountId::Workshop),
)?;
economy.apply(
    Exchange::new(RateId::Finish, Quantity::new(1))
        .bind(Role::Shop, AccountId::Workshop)
        .bind(Role::Goal, AccountId::Success),
)?;

let goal = Goal::new().require(
    AccountId::Success,
    basket([(Asset::Solved, 1)]),
);
assert!(economy.matches(&goal));
# Ok::<(), Box<dyn std::error::Error>>(())
```

The `Build` exchange is valid only if every role binding, balance,
preservation requirement, arithmetic operation, and invariant succeeds. The
core prepares all affected accounts first and commits all of them together.

## Conformance problems

| Problem | Encoded concepts | Compared strategies |
| --- | --- | --- |
| Key-door maze | Topology, position, lock, key, energy, heuristic, goal | BFS, Dijkstra, A* |
| Sokoban | Cell occupancy, push legality, deadlock | BFS and infeasibility |
| Exact cover | Universe, subsets, coverage, selection | BFS and Algorithm X |
| Workshop | Recipes, catalysts, material, labor, waste | BFS and waste minimization |
| Job shop | Precedence, discrete machine capacity, makespan | Best-first and independent branch optimizer |
| Rescue | Hidden truth, seed, observation, belief, chance | Policy rollouts and Monte Carlo |
| Bridge | Capacity, bids, escrow, priority, joint resolution | BFS, first-come, and auction mechanisms |

Every accepted result is an exchange trace replayed by the same core. The
specialized algorithms are proposers, not alternate execution engines.

Every problem has a small consumer-facing example. The examples contain no
domain rules: they instantiate a public model, invoke one or more strategies,
replay the proposed trace, assert the encoded goal, and print a summary.

```console
cargo run -p axionomy-problems --example maze
cargo run -p axionomy-problems --example sokoban
cargo run -p axionomy-problems --example exact_cover
cargo run -p axionomy-problems --example workshop
cargo run -p axionomy-problems --example scheduling
cargo run -p axionomy-problems --example rescue
cargo run -p axionomy-problems --example bridge
```

For example, the maze prints:

```text
BFS: 3 exchanges; A*: 6 energy across 6 exchanges
```

## Semantics in brief

For exchange units `n`, consume and produce baskets scale by `n`. A preserved
basket is an unscaled read threshold: it must exist, but it is not consumed.
This makes a batched exchange observationally consistent with sequential
firings that reuse the same catalyst or fact.

For every affected account:

```text
required = consume × n + preserve
next     = current - consume × n + produce × n
```

All quantities are checked. Every declared linear invariant must have the same
measure before and after. A failure leaves the economy unchanged.

## Development

Axionomy uses Rust Edition 2024, supports Rust 1.85 or newer, and currently has
no third-party dependencies.

```console
cargo test --workspace --all-targets
cargo test --workspace --doc
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
cargo package -p axionomy
cargo package -p axionomy-search --list
cargo package -p axionomy-problems --list
```

Until `axionomy` is available from a registry, Cargo cannot perform an
ordinary registry-resolution dry run for the two dependent packages. CI can
verify their package archives using local `[patch.crates-io]` overrides; their
source manifests retain both path and version requirements for normal
workspace development and future ordered publication.
