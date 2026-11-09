# Storage, runs and replay

Status: design, with its first step done. The kernel's forks are now
constant-time; everything else here is planned, in the order the phases at the
end give.

Axionomy is an economic debugger and a simulation engine. Both need states
that can be kept, revisited, compared and branched: time travel to any step,
step-by-step debugging, replay verification, counterfactual forks, rollouts and
Monte Carlo ensembles, counterexample search, and reinforcement-learning loops
that run many environments in parallel. This document describes how states,
runs and steps are represented, recorded, organized and stored so that all of
these are cheap, without slowing the step loop that search and training depend
on.

## Principles

1. **The kernel decides; storage remembers.** Exchange validity, atomic
   application, conservation, invariants and rounding stay in the kernel and
   its Lean specification. The database never decides what an exchange means.
2. **A state is a value, a run is a trace.** A state is what replaying a
   trace of exchanges from an initial state produces. Stored states are a cache
   of replay, and replay is how they are verified.
3. **Every recorded state has an identity; durability follows rules.** Any
   state a run reached has a canonical content hash, available whenever it is
   asked for. What reaches disk, when, and for how long is decided by a few
   explicit rules, not by how a state was produced.
4. **Content addressing makes everything mergeable.** Equal content has one
   address, so stores written independently combine by union, without
   coordination or conflicts.
5. **Measure where a decision depends on it.** Performance changes to the
   kernel are made when a benchmark shows they pay, and never change its
   meaning.

## Measurements

Release build on an Apple-silicon laptop; an exchange between two accounts.

| Accounts | Fork, before → after | Fork and apply, before → after | Apply in place |
| --- | --- | --- | --- |
| 10 | 69 ns → 8 ns | 1.6 µs → 1.6 µs | 1.5 µs |
| 1,000 | 3.2 µs → 6 ns | 5.0 µs → 1.9 µs | 1.5 µs |
| 100,000 | 399 µs → 6 ns | 430 µs → 1.8 µs | 1.4 µs |

Before, a fork copied the account index, so an MCTS expansion at 100,000
accounts cost about 270 times what it cost at ten, and a rollout's fork cost as
much as 280 of its steps. `benches/economy.rs` keeps the fork-and-apply
benchmark by account count.

BLAKE3 takes 122 ns for 128 bytes, 467 ns for 512 bytes and 2.1 µs for 4 KB.
Hashing the tree nodes an exchange changes would cost as much as the exchange,
so hashes are computed when a root is asked for and remembered in the node,
never on the step path.

## The kernel's state

In the current kernel, accounts are part of the problem definition: they are
declared when an economy is built, and an exchange that binds an undeclared
account is rejected. A new actor is modeled today by declaring its account in
advance and activating it with an asset. The kernel therefore keeps account
identifiers in one shared index and holdings in a persistent 32-way tree. A
fork copies two pointers, and replacing an account copies one root-to-leaf
path, in place when nothing else shares it.

Creating accounts by exchange, for spawned actors, opened positions or firms
entering a market, would change the kernel's semantics and starts in the Lean
specification: how a rate opens an account, how its identifier is chosen so
replay reproduces it, and how existence interacts with conservation. The
representation already accommodates it: the identifier index becomes a
persistent map and holdings grow by appending, in O(log n), so forks stay
constant-time. Accounts keep the
order they were declared in, so every serialized artifact is byte for byte
what it was before. This is a change of representation only; the Lean
specification and its fixtures are untouched.

The kernel does not produce content hashes. Problem ontologies are Rust types
with `Ord` and `Hash` but no canonical byte encoding, and a canonical root
needs one. Storage builds canonical trees from encoded states instead.

Two costs remain for later, each only when a problem makes it matter:
applying an exchange copies each touched account's whole basket, which matters
for accounts with very many assets; and `state_key`, which graph search uses
to recognize visited states, sorts every balance of a state.

## The tree

Stored states are persistent ordered maps from byte keys to byte values,
built as prolly trees, in a new crate, `axionomy-tree`:

- **Canonical shape.** A hash of each key's bytes decides where nodes end. The
  same set of keys produces the same tree whatever order they were inserted in,
  and changing a value never moves a boundary.
- **Immutable, shared nodes.** Nodes live behind `Arc`. An update copies the
  path from the root to the changed entries and shares everything else.
- **Content hashes on request.** A node's identity is the BLAKE3 hash of its
  deterministic encoding, computed when first asked for and remembered. A node
  shared by a thousand states is hashed once.
- **Counted subtrees.** Internal entries carry their subtree's entry count, so
  counts, positional access and paging are O(log n).
- **Operations:** get, put, delete and batched updates; an ordered cursor from
  any key; prefix ranges; `diff(a, b)`, which skips subtrees whose hashes are
  equal; node encoding and decoding.

A stored economy is a few trees:

- accounts: account ID → that account's balance-tree root and entry count;
- balances, one per account: asset ID → quantity, 32 bytes big-endian;
- rates: rate ID → rate record, with large baskets as their own trees;
- invariants: name → weight tree.

The **state root** is the hash of the accounts, rates and invariants roots.
Keys and values need canonical bytes: the string-ID profile MCP uses has them
(UTF-8 identifiers, big-endian quantities); a typed ontology is stored through
an encoding it declares.

A recorded run keeps its tree beside the kernel's economy. After a step, the
receipt names exactly which balances changed; the recorder reads their new
values from the kernel, which stays the source of truth, and updates the tree
in O(changes × log n). Tests check that building the tree from scratch gives
the same root.

## Identity

Three identities answer three different questions:

- **State root:** what the state is. An unkeyed content hash: equal states have
  equal roots whatever their history. It deduplicates, merges stores, verifies
  replays, and is a transposition or cache key that holds across processes,
  unlike `state_fingerprint`, which is a process-local 64-bit hash.
- **Run and step:** where a state occurred. A run ID and a step index name a
  point in a history; two steps may share a root.
- **MCP handle:** what an unauthenticated caller may read. A keyed hash, as
  today, that maps to a run and step. Training never needs one.

## Runs and steps

A **run** records where it starts (an initial root, or a parent run and step
for a fork), its seed, strategy or policy and their versions, budget, kernel
version, checkpoint rule and simulated-time origin. The trace reproduces
states; the seed and policy reproduce the decisions.

A **step** records its exchange, a receipt digest, its simulated time, its root
when the checkpoint rule keeps it, and optional metrics.

Anything that should leave no record is not a run: the nodes of a search tree
and rollouts nobody keeps live and die in memory.

## Durability rules

1. **A run's trace is durable.** Every step's exchange and receipt digest is
   written, about a hundred bytes per step, in batches. Any state a run reached
   can therefore be rebuilt.
2. **States are durable at checkpoints.** A run's checkpoint rule says which of
   its states are stored; any other step is the nearest checkpoint before it
   plus replay. Pinning a step later stores its state too.

   | Rule | Stores | For |
   | --- | --- | --- |
   | `every_step` | Every state | Debugging, regression scenarios, replay buffers |
   | `every(n)` | Every nth state and the last | Long simulations: any step is at most n replays away |
   | `ends` | The first and last states | Training episodes, Monte Carlo and MPPI rollouts |

3. **Retention decides deletion.** Pins, refs, tags, groups and age select what
   stays; garbage collection removes the rest (see Retention).
4. **Each transaction states its durability.** A caller that waits, such as an
   MCP call or a checkpoint someone asked for, commits with
   `Durability::Immediate`. Batches nobody waits on commit with
   `Durability::None`, with a periodic `Immediate` commit, so a crash loses at
   most the unflushed tail and never consistency.

## Tags, groups and metrics

Runs come in families: the rollouts of one Monte Carlo estimate, the sampled
trajectories of one MPPI decision, an evaluation suite, the episodes of a
training epoch. Organizing them takes four kinds of metadata. None of it is
authoritative: tags, groups and metrics never change what an exchange does or
whether a trace is valid, so they may be plain strings and numbers without
weakening the economy's closure.

- **Tags** are string key-value labels on runs and steps, such as
  `experiment=pricing-v3`, `split=eval`, `policy=ppo@412`, `kind=counterexample`
  or `note=suspicious`. Provenance (seed, policy, versions) stays typed in the
  run record and never changes; tags can be added and removed later. An index
  answers "runs where these tags hold", paged.
- **Groups** collect what was produced together. A group has a kind (for
  example `monte_carlo`, `mppi`, `eval_suite`, `epoch`), its source (the run
  and step or root its members started from), its parameters as tags, and its
  members with their sample index. A member is either a run, or a record of its
  seed and metrics that becomes a run when someone asks, by running the
  sampler again from the group's source. A run may belong to several groups.
- **Metrics** are named numbers on runs, steps and group members: reward,
  return, cost, importance weight, value estimate, objective values. They are
  `f64` for analysis; exact economic outcomes remain in the state and can
  always be derived again by replay. Each group keeps a running summary per
  metric (count, mean, minimum, maximum, variance), updated in the same
  transaction as its members, and computes quantiles on request.
- **Refs** are mutable names for a run, step or group, like Git refs:
  `baseline`, `best/ppo`, `regressions/overdraft-17`.

Patterns these support:

- **Monte Carlo.** One group per estimate: N rollouts from one state, each with
  its sample index, seed and return. The group summary is the estimate; the
  best and worst members are kept as runs so they can be stepped through.
- **MPPI.** At each control step t of an executed run, one `mppi` group holds
  the K sampled rollouts from the state at t, with their costs and weights, and
  records the control it chose. Step t of the executed run links to its group,
  so in the debugger every decision shows the futures it sampled, how each
  scored, and how they were weighted. Members are usually seed-and-metrics
  records, so thousands of samples per decision stay cheap, and any one of them
  can be replayed as a run.
- **Training and evaluation.** Episodes are tagged by experiment, policy
  version, split, epoch and environment seed; evaluation suites are groups
  whose summaries compare policies; promoting a policy moves a ref.
- **Counterexamples.** Failures found by adversarial search are tagged with the
  property they violate, pinned and given refs, and become regression scenarios.

## Writing

redb is the transaction layer for everything physical, and the only storage
engine: there is no storage trait of our own. Where data lives is chosen with
redb's backend, `FileBackend` or `InMemoryBackend`. One write transaction
publishes an operation's nodes, its run and step records and any handle
together, so a state or handle exists if and only if its transaction committed.

- **One writer, group commit.** Each store has one writer thread. Runs and MCP
  calls send batches over a bounded channel, which applies backpressure instead
  of growing memory, and the writer commits many batches per transaction.
  Nothing holds a write transaction while the kernel computes.
- **Idempotent writes.** Nodes are inserted by hash, so retries and duplicates
  are harmless, and the initial state of ten thousand episodes is stored once.
- **Merge later.** A worker thread, process or machine may write its own store
  and never coordinate with others. Merging two stores is a union of nodes plus
  run records with globally unique run IDs (a random 128-bit store ID plus a
  counter), which cannot conflict.
- **Tests and the browser** use `InMemoryBackend`: the same code, with storage
  in memory.

## Reading

Readers use redb read transactions, so a debugger or Studio sees one consistent
committed state while training keeps writing.

- **Time travel.** Opening step k opens its root if it is a checkpoint;
  otherwise the nearest checkpoint before it, plus the exchanges in between.
- **Step debugging.** A step shows the state before and after, the exchange,
  its receipt and assessment, and `diff(before, after)`, which is exactly what
  changed. Stepping back opens the previous state.
- **Replay verification.** Replay the trace from the initial root and compare
  the resulting root with the stored one.
- **Watch and bisect.** Steps where a balance changed are found by comparing
  subtree hashes along the run, skipping equal subtrees; a predicate that
  flips once ("when did this go wrong?") is a binary search over checkpoints,
  then replay within the last interval.
- **Counterfactuals.** Fork a new run at any step, then diff the branches at
  aligned steps or at their ends; equal roots mean they converged.
- **Combining branches** is replaying one branch's exchanges onto the other,
  so the kernel validates each one. There is no tree-level merge of economic
  state, because a merged balance that no exchange produced would not be a
  state of the economy.
- **Actor views.** State and step resources take an optional actor and
  project through the kernel's existing visibility, for stepping through what
  one actor could see.
- **Simulated time.** Step records carry simulated time, indexed so that
  "state at time t" resolves to a step.

## Training loops

- **The step stays in memory.** An environment step is a kernel `apply`.
  Recording it appends to the run's trace buffer, which the writer flushes in
  batches; the step never waits on disk.
- **Reset is a fork,** which costs a few nanoseconds at any size.
- **Parallel environments share nothing mutable.** Each environment owns its
  economy. Recording goes through the writer's channel, or into a per-worker
  store merged later.
- **Roots are available on request** for transposition tables, novelty counts
  or any cache keyed by state, across processes.
- **Replay buffers can reference states.** A transition can be (root before,
  exchange, receipt, root after, outcome) instead of copied observations, with
  observations projected from roots on demand and cached. It is exact,
  deduplicated and much smaller, and needs the episode's states stored, under
  `every_step`.

## Retention

Training produces far more than anyone keeps. A **pin** keeps a run or step and
everything it reaches: debugging sessions, counterexamples, benchmark results,
live MCP handles. Refs pin what they name. Retention policies select by tags,
groups and age, for example "keep every `split=eval` run", "keep the five best
members of each Monte Carlo group", or "drop `kind=mppi` runs after a day".
Garbage collection marks from what is kept and sweeps unreachable nodes and
traces in bounded batches inside write transactions; redb reuses the freed
pages, and compaction runs offline. Group summaries survive the members they
summarize.

## What changes where

- **Kernel** (done): a persistent account index, so forks are constant-time.
- **`axionomy-tree`** (new): the canonical persistent tree. No I/O and no
  economics.
- **`axionomy-store`** (new): redb tables, the writer and durability rules,
  runs and steps, tags, groups, metrics and refs, merge and garbage collection.
- **`axionomy-mcp`**: the `SnapshotStore` trait, `MemorySnapshotStore` and the
  assessment and artifact maps are removed in favor of the store. Resources
  gain runs, steps, groups and their summaries, tag queries, refs, watch,
  bisect and actor views; a tool adds and removes tags and moves refs.
- **Studio server**: runs and artifacts move into the store, and scrubbing
  renders frames from stored roots instead of loading whole artifacts.
- **Search and RL**: kept rollouts, sessions and RL episodes record as runs with
  checkpoint rules; Monte Carlo and MPPI record groups.

## Phases

Each phase is a validated commit or series of commits.

0. **Constant-time forks** (done): the kernel's persistent account index, with
   every Studio artifact unchanged byte for byte and the fork benchmark in
   `benches/economy.rs`.
1. **`axionomy-tree`**, with tests for what makes it correct: the same keys give
   the same root in any insertion order, an incrementally updated tree has the
   same root as one built from scratch, diff and cursors agree with a sorted
   map, and encoding round-trips.
2. **`axionomy-store`** on redb: nodes, roots, runs and steps, the durability
   rules, tags, groups with running summaries, metrics and refs, the writer
   with group commit, merge and garbage collection, tested on
   `InMemoryBackend`.
3. **MCP on the store.** Remove the snapshot trait, the in-memory store and
   the registries; add run, step and group resources; extend the Python
   interop suite.
4. **Recording and debugging:** runs, checkpoint rules and groups in search,
   rollouts, Monte Carlo, MPPI and RL; replay-buffer transitions by root;
   watch, bisect, actor views and the time index; Studio scrubbing from the
   store.
5. **Distributed training:** per-worker stores, merge tooling and retention
   policies.

## Open decisions

1. **Chunk boundaries** from the key alone, so value changes never re-chunk
   (recommended), or from the key and value.
2. **Quantity encoding** in nodes: fixed 32-byte big-endian (recommended:
   simple, ordered, exact) or minimal length.
3. **Typed ontologies in storage:** an encoding trait the ontology implements,
   or storing typed economies through the string-ID profile.
4. **Periodic `Immediate` commit interval** for batches nobody waits on; one
   second is a starting point.
5. **Rates and invariants on the tree** from phase 1, or flat until a problem
   with large rate sets needs it.
6. **Tag keys:** free-form (recommended to start), or a small declared
   vocabulary per store once tag queries need validation.
7. **Running summaries** beyond count, mean, minimum, maximum and variance,
   such as streaming quantile sketches, only if quantiles over large groups
   become a measured cost.
