# Storage, runs and replay

Status: design. Nothing here is implemented yet; the phases at the end say in
what order it should be, and which measurements gate each step.

Axionomy is an economic debugger and a simulation engine. Both need states
that can be kept, revisited, compared and branched: time travel to any step,
step-by-step debugging, replay verification, counterfactual forks, rollouts and
Monte Carlo ensembles, counterexample search, and reinforcement-learning loops
that run many environments in parallel. This document describes how states,
runs and steps are held in memory, recorded, organized and stored so that all
of these are cheap, without slowing the step loop that search and training
depend on.

## Principles

1. **The kernel decides; storage remembers.** Exchange validity, atomic
   application, conservation, invariants and rounding stay in the kernel and
   its Lean specification. The database never decides what an exchange means.
2. **A state is a value, a run is a trace.** A state is what replaying a
   trace of exchanges from an initial state produces. Stored states are a cache
   of replay, and replay is how they are verified.
3. **Pay for identity only when a state is kept.** Hashing and persistence
   happen when a state is sealed, never in the step loop. A training run that
   keeps nothing pays nothing.
4. **Content addressing makes everything mergeable.** Equal content has one
   address, so stores written independently combine by union, without
   coordination or conflicts.
5. **Measure before changing the kernel.** The kernel's in-memory
   representation changes only when benchmarks show it pays, and never its
   meaning.

## Three tiers

| Tier | Where | Holds | Cost per step |
| --- | --- | --- | --- |
| Working | Kernel, in memory | The current economy of each run, search node or environment | Applying the exchange; no hashing, no I/O |
| Recorded | Per run, in memory | Exchanges and receipts, appended; seal marks | An append of what `apply` already produced |
| Durable | redb | Content-addressed tree nodes, runs, steps, pins | Only for sealed states, in batches, off the step path |

**Sealing** turns a working state into a durable one: it computes the content
hashes of tree nodes created since the last seal and queues the nodes the store
does not have yet. Node hashes are memoized in the immutable nodes, so a node
shared by a thousand forks is hashed once.

## The tree

Every large collection in an economy is a persistent ordered map from byte keys
to byte values, built as a prolly tree:

- **Shape depends only on keys.** A cheap hash of each key decides where nodes
  end. The same set of keys produces the same tree whatever order they were
  inserted in, and changing a balance never moves a boundary, because balances
  change every step and keys rarely do.
- **Immutable, shared nodes.** Nodes live behind `Arc`. An update copies the
  path from the root to the changed entries and shares everything else, so a
  fork is a pointer copy and an exchange costs O(changed entries × log n).
- **Lazy content hashes.** A node's identity is the BLAKE3 hash of its
  deterministic encoding, computed on first request and memoized.
- **Counted subtrees.** Internal entries carry their subtree's entry count, so
  counts, positional access and paging are O(log n).
- **Operations:** get, put, delete and batched updates; an ordered cursor from
  any key; prefix ranges; `diff(a, b)`, which skips subtrees whose hashes are
  equal; node encoding and decoding.

An economy is a few trees:

- accounts: account ID → that account's balance-tree root and entry count;
- balances, one per account: asset ID → quantity, 32 bytes big-endian;
- rates: rate ID → rate record, with large baskets as their own trees;
- invariants: name → weight tree.

The **state root** is the hash of the accounts, rates and invariants roots.

The tree is its own crate, `axionomy-tree`, with no I/O and no knowledge of
economies, so the kernel can use it in memory and the store can persist it.

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

With memoized node hashes, a state root after an exchange costs only the nodes
that exchange created. Search can therefore use roots as incremental
fingerprints for transposition tables, which today hash the whole state.

## Runs, steps and recording

A **run** records where it starts (an initial root, or a parent run and step
for a fork), its seed, strategy or policy and their versions, budget, kernel
version, recording mode and simulated-time origin. The trace reproduces states;
the seed and policy reproduce the decisions.

A **step** records its exchange, a receipt digest, its simulated time, an
optional sealed root and optional metrics.

Each run chooses how much to keep:

| Mode | Keeps | For |
| --- | --- | --- |
| `off` | The run record and its metrics | Training throughput |
| `trace` | Every exchange and receipt; no hashing | Replayable runs at near-zero cost |
| `checkpoint(n)` | The trace, plus a sealed root every n steps and at the end | Long simulations; any step is at most n replays away |
| `every_step` | The trace, plus a sealed root per step | Debugging, regression scenarios, replay buffers |

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
- **Groups** collect runs produced together. A group has a kind (for example
  `monte_carlo`, `mppi`, `eval_suite`, `epoch`), its source (the run and step
  or root the members started from), its parameters as tags, and its members
  with their sample index. A run may belong to several groups.
- **Metrics** are named numbers on runs and steps: reward, return, cost,
  importance weight, value estimate, objective values. They are `f64` for
  analysis; exact economic outcomes remain in the state and can always be
  derived again by replay. Each group keeps a running summary per metric
  (count, mean, minimum, maximum, variance), updated in the same transaction
  as its members, and computes quantiles on request.
- **Refs** are mutable names for a run, step or group, like Git refs:
  `baseline`, `best/ppo`, `regressions/overdraft-17`.

Patterns these support:

- **Monte Carlo.** One group per estimate: N rollouts forked from one state,
  each tagged with its sample index and seed and carrying its return. The
  group summary is the estimate; the best and worst members are pinned so they
  can be stepped through.
- **MPPI.** At each control step t of an executed run, one `mppi` group holds
  the K sampled rollouts forked from the state at t, with their costs and
  weights, and records the control it chose. Step t of the executed run links
  to its group, so in the debugger every decision shows the futures it
  sampled, how each scored, and how they were weighted. Rollouts are usually
  recorded as `off` or `trace` and sealed only when pinned.
- **Training and evaluation.** Episodes are tagged by experiment, policy
  version, split, epoch and environment seed; evaluation suites are groups
  whose summaries compare policies; promoting a policy moves a ref.
- **Counterexamples.** Failures found by adversarial search are tagged with the
  property they violate, pinned and given refs, and become regression scenarios.

## Writing

redb is the transaction layer for everything physical. One write transaction
publishes an operation's nodes, its run and step records and any handle
together, so a state or handle exists if and only if its transaction committed.

- **One writer, group commit.** Each store has one writer thread. Runs and MCP
  calls send sealed batches over a bounded channel, which applies backpressure
  instead of growing memory, and the writer commits many batches per
  transaction. Nothing holds a write transaction while the kernel computes.
- **Durability by need.** MCP calls and checkpoints a caller waits on commit
  with `Durability::Immediate`. Bulk training batches commit with
  `Durability::None`, with a periodic `Immediate` commit, so a crash loses at
  most the unflushed tail and never consistency.
- **Idempotent writes.** Nodes are inserted by hash, so retries and duplicates
  are harmless, and the initial state of ten thousand episodes is stored once.
- **Merge later.** A worker thread, process or machine may write its own store
  and never coordinate with others. Merging two stores is a union of nodes plus
  run records with globally unique run IDs (a random 128-bit store ID plus a
  counter), which cannot conflict.
- **Tests and the browser** use redb's `InMemoryBackend`: the same code, with
  storage in memory.

## Reading

Readers use redb read transactions, so a debugger or Studio sees one consistent
committed state while training keeps writing.

- **Time travel.** Opening step k opens its root if sealed; otherwise the
  nearest sealed root before it, plus at most n replayed exchanges.
- **Step debugging.** A step shows the state before and after, the exchange,
  its receipt and assessment, and `diff(before, after)`, which is exactly what
  changed. Stepping back opens the previous root.
- **Replay verification.** Replay the trace from the initial root, seal, and
  compare roots.
- **Watch and bisect.** Steps where a balance changed are found by comparing
  subtree hashes along the run, skipping equal subtrees; a predicate that
  flips once ("when did this go wrong?") is a binary search over sealed steps.
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

- **The step stays in memory.** An environment step is a kernel `apply`;
  storage is never on that path unless the run records, and then only as an
  append.
- **Reset is a fork.** Episodes start from a sealed initial root. With the
  kernel on the tree, that is a pointer copy; today it copies the account index.
- **Parallel environments share nothing mutable.** Each environment owns its
  economy. Recording goes through the writer's channel, or into a per-worker
  store merged later.
- **Replay buffers can reference states.** A transition can be (root before,
  exchange, receipt, root after, outcome) instead of copied observations, with
  observations projected from roots on demand and cached. It is exact,
  deduplicated and much smaller, and requires that the referenced states be
  sealed.
- **Hash only what is kept.** Sealing costs the nodes created since the last
  seal, so per-step sealing in a long episode stays proportional to what
  changed.

## Retention

Training produces far more than anyone keeps. A **pin** keeps a run or step and
everything it reaches: debugging sessions, counterexamples, benchmark results,
live MCP handles, and checkpoints by policy (for example the last k per run).
Refs pin what they name. Retention policies select by tags and groups, for
example "keep every `split=eval` run", "keep the five best members of each
Monte Carlo group", or "drop `kind=mppi` rollouts after a day". Garbage
collection marks from pins and sweeps unreachable nodes in bounded batches
inside write transactions; redb reuses the freed pages, and compaction runs
offline. Group summaries survive the members they summarize.

## What changes where

- **`axionomy-tree`** (new): the persistent tree. No I/O and no economics.
- **`axionomy-store`** (new): redb tables, the writer, sealing, runs and steps,
  tags, groups, metrics and refs, merge and garbage collection.
- **`axionomy-mcp`**: the `SnapshotStore` trait, `MemorySnapshotStore` and the
  assessment and artifact maps give way to the store. Resources gain runs,
  steps, groups and their summaries, tag queries, refs, watch, bisect and
  actor views; a tool adds and removes tags and moves refs.
- **Kernel**: baskets and the account index move onto the tree. This is a
  change of representation only; the Lean specification and its fixtures do
  not change.
- **Studio server**: runs and artifacts move into the store, and scrubbing
  renders frames from stored roots instead of loading whole artifacts.
- **Search and RL**: rollouts, sessions and RL transitions record through the
  modes above.

## Phases

Each phase is a validated commit or series of commits.

0. **Measure.** Extend `benches/economy.rs` with step throughput, fork cost and
   fingerprint cost on small problems and on a 100,000-asset economy. Record
   the baselines here; they set the targets below.
1. **`axionomy-tree`**, with tests for what makes it correct: the same keys give
   the same root in any insertion order, a lazy hash equals an eager one, diff
   and cursors agree with a sorted map, and encoding round-trips.
2. **`axionomy-store`** on redb: nodes, roots, runs and steps, tags, groups with
   running summaries, metrics and refs, the writer with group commit and
   durability policy, merge and garbage collection, tested on
   `InMemoryBackend`. Working states convert to trees when sealed.
3. **MCP on the store.** Retire the snapshot trait, the in-memory store and the
   registries; add run and step resources; extend the Python interop suite.
4. **Kernel on the tree**, gated by phase 0: no regression on small problems,
   pointer-copy forks and incremental roots on large ones. Sealing then hashes
   only new nodes.
5. **Recording and debugging:** recording modes in search, rollouts and RL;
   Monte Carlo and MPPI groups with decision links; replay-buffer transitions
   by root; watch, bisect, actor views and the time index; Studio scrubbing
   from the store.
6. **Distributed training:** per-worker stores, merge tooling and retention
   policies.

## Open decisions

1. **Basket order.** Key order, which the tree gives and content addressing
   needs, or insertion order as today. Key order is recommended; it changes the
   order of serialized output, not semantics.
2. **Chunk boundaries** from the key alone, so balance changes never re-chunk
   (recommended), or from the key and value.
3. **Quantity encoding** in nodes: fixed 32-byte big-endian (recommended:
   simple, ordered, exact) or minimal length.
4. **Training durability interval** for periodic `Immediate` commits; one second
   is a starting point.
5. **Rates and invariants on the tree** from phase 1, or flat until a problem
   with large rate sets needs it.
6. **Tag keys:** free-form (recommended to start), or a small declared
   vocabulary per store once tag queries need validation.
7. **Running summaries** beyond count, mean, minimum, maximum and variance,
   such as streaming quantile sketches, only if quantiles over large groups
   become a measured cost.
