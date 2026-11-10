# Storage, runs and replay

Status: being built, in the order the phases at the end give. The kernel's
forks are constant-time, the canonical tree and the store exist, and MCP keeps
its snapshots in the store; Studio, search and every other producer of runs
move onto it next.

Axionomy is an economic debugger and a simulation engine. Both need states
that can be kept, revisited, compared and branched: time travel to any step,
step-by-step debugging, replay verification, counterfactual forks, rollouts and
Monte Carlo ensembles, counterexample search, and reinforcement-learning loops
that run many environments in parallel. This document describes how states,
runs and steps are represented, recorded, organized and stored so that all of
these are cheap, without slowing the step loop of whatever produces the runs:
search, simulation, evaluation, agents, servers or learning.

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

The tree (`crates/axionomy-tree/benches/tree.rs`), with balance-sized entries:

| Entries | Set one value | Set one value and hash the root | Build from scratch |
| --- | --- | --- | --- |
| 1,000 | 0.9 µs | 4.9 µs | 175 µs |
| 100,000 | 2.4 µs | 15 µs | 18 ms |

Recording a run (`crates/axionomy-store/benches/record.rs`): one payment
between two accounts, as the stepping thread sees it. Hashing and writing
happen on the writer thread, so the checkpoint rule barely shows.

| Accounts | Kernel apply | Recorded, `ends` | Recorded, `every(64)` | Recorded, `every_step` |
| --- | --- | --- | --- | --- |
| 1,000 | 1.9 µs | 5.0 µs | 6.1 µs | 5.7 µs |
| 100,000 | 2.2 µs | 5.6 µs | 6.4 µs | 5.9 µs |

## The kernel's state

Exchanges open and close accounts (specified in `Lifecycle.lean`). The kernel
keeps identifiers and holdings in persistent 32-way trees indexed by creation
slot, and a persistent hash trie from identifier to slot. A fork copies a few
pointers; replacing, opening or closing an account copies one root-to-leaf
path, in place when nothing else shares it; a closed account leaves an empty
slot until more slots are empty than full, when the accounts are compacted.
Accounts keep the order they were created in, so every serialized artifact
stayed byte for byte what it was before the index changed.

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

- **Canonical shape.** A keyed BLAKE3 hash of each key decides where nodes
  end: at each level a key ends its node with probability 1/32, from its own
  five bits of that hash. The same set of keys produces the same tree whatever
  order they were inserted in, and changing a value never moves a boundary. A
  node also ends at 256 items, so keys chosen to never end a node cannot make
  one larger; that boundary moves with insertions, so an edit near it
  re-chunks up to the next key-chosen boundary.
- **Immutable, shared nodes.** Nodes live behind `Arc`. An update copies the
  path from the root to the changed entries and shares everything else.
- **Content hashes on request.** A node's identity is the BLAKE3 hash of its
  deterministic encoding, computed when first asked for and remembered. A node
  shared by a thousand states is hashed once.
- **Counted subtrees.** Internal entries carry their subtree's entry count, so
  counts, positional access and paging are O(log n).
- **Operations:** get, put, delete and batched updates; an ordered cursor from
  any key or position; prefix ranges; ranks; `diff(a, b)`, which skips
  subtrees whose hashes are equal; the nodes a store is missing; and loading
  by root hash, which checks every node against its hash and the tree against
  the shape its keys determine, so a loaded tree is canonical.
- **Edits** walk the old tree in key order, feed a builder the edited entries
  and hand it every unchanged subtree that chunking would rebuild as it is, so
  an edited tree is exactly the tree a build from scratch gives.

A stored economy is one tree, whose keys say what each entry holds:

| Key | Value |
| --- | --- |
| `a` account | empty: the account exists |
| `b` account `\0` asset | the balance, 32 bytes big-endian, never zero |
| `r` rate | the rate, as the kernel serializes it |
| `v` invariant, as the kernel serializes it | empty |

Every identifier is written as its JSON, so a problem's own Rust types and
paths are stored the same way: an economy of any `axionomy::Ontology` whose
identifiers serialize can be recorded, and a run records its ontology's name
and is read back only as that ontology. JSON never contains `\0`, which sorts
before every other byte, so an account's balances are contiguous and in
account order, and a prefix range reads one account. A JSON string sorts as
its text does, so an economy identified by paths (`axionomy::paths::Paths`)
lists its accounts in path order and a path prefix is a key prefix. The
**state root** is the tree's hash. The tree holds the logical state, not the
order accounts were created in, so an economy rebuilt from a root lists
accounts in key order; decoding accepts only trees an economy could have
produced, in exactly the encoding the store writes.

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
  today, that maps to a run and step. Code that records its own runs never
  needs one.

## Runs and steps

A **run** records where it starts: its first state, always stored, and for a
fork the point it forked from, whose step number its own steps continue from.
It also records its checkpoint rule and whether it finished. Seeds, policies
and their versions are tags for now; they become typed provenance when the
producers of runs record them (phase 4), with the simulated-time origin. The trace
reproduces states; the seed and policy reproduce the decisions.

A **step** records its exchange and the digest of its receipt. A separate
table holds the roots of stored states, by run and step, so the nearest stored
state before any step is one range lookup.

Anything that should leave no record is not a run: the nodes of a search tree
and rollouts nobody keeps live and die in memory. Whether a rollout is kept is
its caller's choice, not the rollout's: code that steps an economy takes
`&mut impl axionomy::Apply`, which an economy implements by keeping nothing
and a `Recorder` implements by recording every step.

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
   | `ends` | The first and last states | Rollouts and episodes: Monte Carlo and MPPI samples, simulations, learning |

3. **Retention decides deletion.** Pins, refs, tags, groups and age select what
   stays; garbage collection removes the rest (see Retention).
4. **Each transaction states its durability.** A caller that waits, such as an
   MCP call or a checkpoint someone asked for, commits with
   `Durability::Immediate`. Batches nobody waits on commit with
   `Durability::None`, with a periodic `Immediate` commit, so a crash loses at
   most the unflushed tail and never consistency.

## Tags, groups and metrics

Runs come in families: the rollouts of one Monte Carlo estimate, the sampled
trajectories of one MPPI decision, an evaluation suite, the branches of one
search, the episodes of one learning epoch. Organizing them takes four kinds of metadata. None of it is
authoritative: tags, groups and metrics never change what an exchange does or
whether a trace is valid, so they may be plain strings and numbers without
weakening the economy's closure.

- **Tags** are key-value labels on runs, such as
  `experiment=pricing-v3`, `split=eval`, `policy=ppo@412`, `kind=counterexample`
  or `note=suspicious`. Keys are paths; values are text people read. Tags can
  be added and removed later. An index answers "runs where this tag holds",
  paged. Tags on steps wait for a use that needs them.
- **Groups** collect what was produced together. A group has a kind (for
  example `monte-carlo`, `mppi`, `eval-suite`, `epoch`), its source (the run
  and step or root its members started from), its parameters as tags, and its
  members with their sample index. A member is either a run, or a record of its
  seed and metrics that becomes a run when someone asks, by running the
  sampler again from the group's source. A run may belong to several groups.
- **Metrics** are numbers named by path on runs, steps and group members: reward,
  return, cost, importance weight, value estimate, objective values. They are
  `f64` for analysis; exact economic outcomes remain in the state and can
  always be derived again by replay. Each group keeps a running summary per
  metric (count, mean, minimum, maximum, variance), updated in the same
  transaction as its members, and computes quantiles on request.
- **Refs** are mutable names for a run, step or group, like Git refs:
  `baseline`, `best/ppo`, `regressions/overdraft-17`. Ref names, metric names,
  tag keys and group kinds are paths (`axionomy_store::{RefPath, MetricPath,
  TagPath, GroupKindPath}`); run and group IDs are minted types.

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
- **Experiments.** Runs are tagged by experiment, configuration or policy
  version, split and seed; suites are groups whose summaries compare the
  variants; promoting a variant moves a ref. Benchmarks, parameter sweeps,
  A/B comparisons of strategies and learning all take this shape.
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
  are harmless, and the initial state of ten thousand runs is stored once.
- **Merge later.** Any writer, a thread, a process, a machine or a server,
  may write its own store and never coordinate with others. Merging two stores is a union of nodes plus
  run records with globally unique run IDs (the store's random 64-bit prefix
  and a 64-bit counter), which cannot conflict. Refs are the one mutable
  name: when both stores name something different, the merging store keeps
  its own and reports the name.
- **Tests and the browser** use `InMemoryBackend`: the same code, with storage
  in memory.

## Reading

Readers use redb read transactions, so a debugger or Studio sees one consistent
committed state while producers keep writing.

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

## Recording at speed

Search expanding states, a simulation advancing, Monte Carlo and MPPI sampling
futures, an agent session, a server handling calls and a learning loop all do
the same thing: apply exchanges fast, and often from many places at once.
Storage serves them the same way.

- **The step stays in memory.** A step is a kernel `apply`. Recording it
  appends to the run's trace buffer, which the writer flushes in batches; the
  step never waits on disk.
- **Branching and resetting are forks,** which cost a few nanoseconds at any
  size.
- **Parallel producers share nothing mutable.** Each owns its economy.
  Recording goes through one store's writer, or into a store of its own merged
  later.
- **Roots are available on request** for transposition tables, novelty counts
  or any cache keyed by state, across processes.
- **Transitions can reference states.** A transition can be (root before,
  exchange, receipt, root after, outcome) instead of copied observations, with
  observations projected from roots on demand and cached: for replay buffers,
  datasets, audits or anything that consumes a run step by step. It is exact,
  deduplicated and much smaller. `Reader::transitions` reads a run's steps
  this way, each replayed and checked against its receipt; under `every_step`
  every root it returns is also stored.
- **Bisection finds when something started.** `Reader::bisect` returns the
  first step where a condition that keeps holding once it holds is true,
  loading O(log n) stored states and replaying at most the distance between
  two of them. A time index is the same search with "the clock has reached t"
  as the condition.

## Retention

Whatever produces runs, search, simulation, evaluation, servers or learning,
produces far more than anyone keeps. A **pin** keeps a run or step and
everything it reaches: debugging sessions, counterexamples, benchmark results,
live MCP handles. Refs pin what they name. Retention policies select by tags,
groups and age, for example "keep every `split=eval` run", "keep the five best
members of each Monte Carlo group", or "drop `kind=mppi` runs after a day".
Deleting a run removes its trace, stored states, tags and metrics, and is
refused while a point of it is pinned or a ref names it. Garbage collection
marks every node reachable from a stored state and sweeps the rest, inside one
write transaction so no concurrent write can revive a node being swept; redb
reuses the freed pages, and compaction runs offline. Sweeping in bounded
batches waits until stores are large enough to need it. Group summaries
survive the members they summarize.

## What changes where

- **Kernel** (done): a persistent account index, so forks are constant-time.
- **`axionomy-tree`** (done): the canonical persistent tree. No I/O and no
  economics.
- **`axionomy-store`** (done): redb tables, the writer and durability rules,
  runs and steps, tags, groups, metrics and refs, merge and garbage
  collection, for any ontology whose identifiers serialize.
- **`axionomy-mcp`** (done): the `SnapshotStore` trait, `MemorySnapshotStore`
  and the assessment and artifact maps are gone; snapshots are points of runs
  in the store, durable across restarts with a store file. Results and
  snapshots carry state roots, `axionomy_trace_step` names any state of the
  exchanges that made a snapshot, `axionomy_trace_find` names the first state
  where a balance condition holds, a `diff` resource compares two snapshots,
  and accounts and balances page from the state tree. Run IDs, tags, refs and
  groups are store-wide names that would let one caller find another's
  snapshots, so MCP exposes only what a handle reaches until it has
  authentication; watch and actor views follow with phase 4.
- **Studio server**: studies record their runs in the store, and scrubbing
  renders frames from stored roots instead of loading whole studies.
- **Problems** (done): every canonical problem's `World` has an `Ontology`,
  and its runs are stored and read back unchanged.
- **Producers of runs**: kept search branches and solutions, rollouts,
  simulations, sessions and episodes record as runs with checkpoint rules;
  Monte Carlo and MPPI record groups. Rollouts and Monte Carlo episodes
  record where they run: `axionomy_search::rollout` steps any `Apply`, and a
  problem's Monte Carlo estimates take an `Episodes` destination, either
  `Unrecorded` or one that starts each episode as a `Recorder` of a
  `GroupRecorder` and adds it with its metrics when it ends, as
  `axionomy-problems/tests/storage.rs` does for logistics. Search sessions,
  MPPI and RL follow.

## Phases

Each phase is a validated commit or series of commits.

0. **Constant-time forks** (done): the kernel's persistent account index, with
   every Studio replay unchanged byte for byte and the fork benchmark in
   `benches/economy.rs`.
1. **`axionomy-tree`** (done), with tests for what makes it correct: the same keys give
   the same root in any insertion order, an incrementally updated tree has the
   same root as one built from scratch, diff and cursors agree with a sorted
   map, and encoding round-trips.
2. **`axionomy-store`** (done) on redb: nodes, roots, runs and steps, the durability
   rules, tags, groups with running summaries, metrics and refs, the writer
   with group commit, merge and garbage collection, tested on
   `InMemoryBackend`.
3. **MCP on the store** (done). Remove the snapshot trait, the in-memory
   store and the registries; name intermediate states, diff snapshots, carry
   state roots; extend the Python interop suite.
4. **Recording and debugging:** typed ontologies in the store (done: every
   identifier is stored as its JSON, and a run is read back only as the
   ontology that recorded it); replay-buffer transitions by root, bisection
   and the time index it gives; every canonical problem recordable, with
   rollouts recorded as runs in a Monte Carlo group (done); rollouts and
   Monte Carlo episodes recording where they run, through `Apply` and
   `Episodes` (done); studies recording their runs in the store; recording from
   inside search sessions, MPPI and RL; watch and actor views; Studio
   scrubbing from the store.
5. **Many writers, one history:** stores written independently by any
   producer, a thread, process, machine or server, merged by tooling that
   inspects, verifies and combines them; and retention policies that keep
   what tags, groups, metrics, refs, pins and age select, for whatever
   produced the runs.

## Open decisions

1. **Periodic `Immediate` commit interval** for batches nobody waits on; one
   second is a starting point.
2. **Running summaries** beyond count, mean, minimum, maximum and variance,
   such as streaming quantile sketches, only if quantiles over large groups
   become a measured cost.
