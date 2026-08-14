# Axionomy Studio visual language

Axionomy Studio turns an authoritative economic replay into an explanation a
person can inspect. The picture is deliberately rich, but it is never a second
simulation model: accounts, assets, rates, exchanges, receipts, and replayed
snapshots remain the only source of truth.

This document is the normative visual-design reference for Studio renderers.
It exists to keep new problem views semantically consistent, visually legible,
and honest about what the engine actually proved.

## Canonical references

| Surface | Canonical problem | What it establishes |
| --- | --- | --- |
| Graph | **Autonomous Work League** | Topology, mobile agents, attached work queues, shared facilities, relationship lines, dense replay state, and live comparative outcomes |
| Market | **The Living Market** | Central pool dominance, exact reserves, endogenous price history, constant-product curve position, actor flows, liquidity depth, and causal attribution |
| Onboarding graph | **Key-door Maze** | Immediate comprehension, competing routes, solver expansion playback, inventory transfer, a stateful gate, planned/traversed route evidence, and rejected-move explanation |
| Grid | **Sokoban** | Stable terrain, independently animated occupants, cell anchoring, goals, and exact replay motion |
| Timeline | **Job-shop scheduling** | Resource lanes, bounded spans, ordering, duration, and temporal progress |
| Matrix | **Exact Cover** | Row/column meaning, sparse incidence, selection state, and exact combinatorial structure |

Work League is the primary stylistic reference for graph-backed problems. A
new graph scene does not need its ontology or arrangement, but it should be
compared against Work League before acceptance whenever it contains locations,
agents, vehicles, jobs, orders, inventory, or facilities.

Key-door Maze is the acceptance reference for the first-run experience. A new
visitor should be able to read its objective from the title, distinguish the
planned route from alternatives, watch the Explorer acquire and consume the
key, see the gate change from locked to open, compare solver work, and connect
each animation to account and receipt evidence without prior Axionomy
knowledge. Because it is the default Studio problem, regressions in any replay
frame are release-blocking even when another graph scene still looks correct.

The other canonical problems are equally normative for their surface types.
Graph conventions must not be forced onto a grid, matrix, or timeline when the
surface itself communicates the problem more clearly.

## Market grammar

The Living Market is the canonical market surface. A market scene explains how
replay-verified exchanges change one public reserve ratio; it never imports an
oracle price or recomputes an authoritative quote in TypeScript.

- The pool is the dominant central object and links directly to its economic
  account.
- Reserve quantities, liquidity supply, fee terms, product, and price are exact
  Rust-owned projection fields.
- Participants surround the pool and link to their authoritative accounts.
- The selected exchange emphasizes only the bound trader or liquidity provider
  and derives its direction and deltas from the receipt.
- Price history is assembled from the sequence of replay-derived market
  snapshots. It is not stored as independent market state.
- The bonding curve uses the snapshot's exact reserve product and marks the
  current reserve point. It explains sensitivity but cannot authorize a swap.
- Proportional liquidity changes depth without visually implying a price move.
- Direct attribution is accumulated from adjacent reserve-price changes;
  counterfactual attribution must state whether it is exact or sampled.
- No external comparison line may appear unless an external venue is itself
  encoded as accounts, assets, rates, and exchanges in the closed economy.

## Truth boundary

The scene contract may project authoritative state into geometry, labels,
icons, metrics, annotations, and transition cues. It may not invent state that
cannot be recovered from the economy or replay evidence.

- A structure corresponds to topology, a location, a facility, or another
  durable anchor represented by the problem view contract.
- An occupant is a stable entity whose authoritative anchor can change between
  adjacent replay snapshots.
- An attachment is an entity or collection related to an anchor without being
  part of the topology itself.
- A transition highlight is derived from the selected exchange and receipt.
- A metric summarizes balances or replay evidence; it does not become a second
  score store.
- Presentation geometry explains relationships but never makes an exchange
  possible, impossible, or valid.

If Studio needs presentation-only semantic state to explain a problem, that is
API pressure on the Rust-owned view contract. It should not be hidden in a
problem-specific TypeScript component.

## Graph grammar

### Structures define topology

Structures form the stable layer of a graph. Their placement should make the
problem's topology readable before any animation begins.

- Locations and ordinary topology anchors are circular.
- Facilities, resources, buyers, sellers, and carriers are rounded rectangles.
- Goals use a distinct double boundary rather than color alone.
- Structure coordinates remain stable throughout replay.
- Directed transition edges sit behind nodes and expose their actual source
  and target ports.
- Edge labels describe costs, duration, conditions, or other authoritative
  transition facts.
- A retained solution may distinguish its remaining planned passages from
  unrelated alternatives; current and traversed statuses remain visibly
  distinct and are derived from the selected replay trace.

Shape is semantic. Two nodes with the same role should not receive unrelated
shapes, and two materially different roles should not be made indistinguishable
when the scene contract can identify the difference.

### Occupants dock to topology

Agents, vehicles, and other moving entities use compact pill-shaped nodes.
They are positioned relative to their current anchor and connected to it with a
subordinate relationship line when that relationship would otherwise be
ambiguous.

- The same stable entity ID moves between anchors; it is not recreated under a
  new label.
- Multiple occupants use deterministic, non-overlapping slots.
- An occupant never visually covers the structure that explains where it is.
- Movement animates between adjacent replay states without moving unrelated
  layout around it.

### Attachments explain ownership and queues

Jobs, orders, inventory, and similar collections use compact panels attached to
their owner or location. A panel has a heading, count, and aligned rows with a
semantic icon, primary label, and concise detail.

- Attachments do not float without a visible relationship or unambiguous
  containment.
- Collections remain grouped instead of becoming clouds of unrelated pills.
- Long collections may summarize overflow, but their existence must remain
  explicit.
- Attachment relation lines are visually subordinate to topology edges.

### Context stays separate

Scenario-wide chance, weather, hidden information, and similar context belongs
in a dedicated context lane. It may affect topology or transitions, but it
must not masquerade as a location or mobile entity.

## Content hierarchy

Each node has explicit internal regions. Browser font inheritance or automatic
grid placement must never decide the node hierarchy.

1. The semantic icon identifies the role.
2. The title names the entity or structure.
3. Optional status or metric text explains the current replay state.
4. A receipt-derived effect marker sits on the outer boundary.

Current graph typography uses compact monospace diagram labels: 8px primary
labels and 5.5px metadata at the canonical desktop scale. These values may
evolve as a system, but a graph node must not inherit the surrounding 16px
application typography.

- Icons, titles, and metadata occupy separate layout regions.
- Primary labels must remain readable at the normal fitted view.
- Structure titles may wrap to two controlled lines.
- Metadata truncation is acceptable only when the complete value is available
  through the node's inspection path or title.
- Text must never overlap an icon, handle, marker, or another text region.
- Effect markers must not obscure the title.

## Motion and effects

Motion is explanatory evidence, not decoration. Studio should animate the
parts of the picture that changed while keeping the surrounding cockpit and
topology stable.

- Stepping one exchange animates travel, arrival, production, consumption,
  preservation, and changed state when replay evidence supports the cue.
- Seeking directly to another step updates immediately instead of fabricating
  motion across skipped exchanges.
- Layout does not reflow because a title, metric, panel, or effect appeared.
- Moving entities lift visually above edges during travel and settle clearly at
  their destination.
- Production, consumption, preservation, and change highlights animate on the
  actual outer node or collection surface.
- A highlight follows the affected shape: circle, pill, rounded rectangle,
  collection panel, or grid entity.
- Effect badges use a compact circular boundary marker and never a square box
  floating inside a rounded node.

Animation should remain visible enough to explain the transition. Solving
layout instability by suppressing meaningful motion is not acceptable.

## Density and composition

Substantial examples are intentionally dense. Density is resolved through
hierarchy, grouping, and stable geometry—not by hiding important state.

- The initial fitted view should expose the whole problem structure.
- Theater mode prioritizes the picture; leaving it returns to the unchanged
  replay controls and current economic step.
- Source-of-truth and current-step panels complement the picture rather than
  duplicate every label inside it.
- Legends name the symbols currently needed to read the scene.
- Z-order remains consistent: edges, structures, attachments, occupants,
  moving occupants, then effect markers.
- Empty space should clarify topology; it should not result from entities being
  stranded far from their anchors.

## Renderer-specific invariants

### Grid

- Terrain stays in normal grid flow.
- Players, crates, and pieces are absolutely positioned stable entities above
  cells.
- Entity centers match their authoritative cell centers at every replay step.
- Cell gaps are included in coordinate calculations.
- Goals and receipt-derived effects do not displace occupants.

### Timeline

- Lanes have stable semantic ownership.
- Spans communicate start, duration, and ordering against one scale.
- Replay progress does not change lane geometry.
- Simultaneous and constrained activity remains visually distinguishable.

### Matrix

- Row and column identities remain visible.
- Selection and exclusion preserve the underlying incidence structure.
- Visual emphasis reflects replay or solver evidence without changing the
  matrix's meaning.

## Acceptance checklist

Before a new or materially changed scene is accepted:

- Compare graph work directly with the Work League Showcase replay.
- Inspect initial, representative middle, effect-heavy, and final frames.
- Confirm every visible entity has an intelligible anchor or container.
- Confirm titles, metadata, icons, handles, and effect markers do not overlap.
- Confirm no primary labels are clipped at the canonical desktop viewport.
- Confirm highlights match their node shapes.
- Confirm stepping animates changed entities while seeking does not invent
  intermediate motion.
- Confirm unrelated topology and cockpit layout remain stable.
- Confirm the picture is derivable from the Rust-owned scene and replay
  contracts.
- Add geometric browser assertions for any regression that could recur.
- For Key-door Maze, inspect every Showcase replay frame plus the retained
  BFS, Dijkstra, A*, and Pareto evidence surfaces; representative frames are
  insufficient for the default onboarding problem.

The gallery images in the root README are snapshots of these canonical
expectations. They should be refreshed whenever a visual-system change makes
the checked-in images materially different from the deployed Studio.
