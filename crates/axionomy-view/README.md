# axionomy-view

`axionomy-view` is Axionomy's runtime-neutral presentation boundary. It replays an accepted exchange trace through the core engine and derives portable snapshots, assessments, receipts, account deltas, frame cues, bounded thinking (the search observations of how an outcome was found), optional scenes, and optional per-snapshot leaderboards for a viewer. A scene separates geometric surface from semantic entities, typed references to the accounts or balances those entities illustrate, paths, exact metrics, annotations, and a constrained glyph vocabulary. Every entity declares a presentation role: `structure` defines topology, `occupant` moves through it, `attachment` belongs to a location or route, `state` describes its owning structure, and `context` applies scenario-wide conditions. The role tells a generic viewer whether to integrate, dock, group, tether, or separate an entity instead of placing every item as an unexplained peer. Those stable identities, anchors, roles, evidence links, and receipt deltas are sufficient for a generic client to compose deterministic movement and economic effects without embedding problem-specific behavior. Leaderboards retain exact score text, eligibility, rank, participant identity, and explanatory components without becoming a score store. Anchors, identities, and evidence references are validated before publication. These values are explanatory projections: assets, accounts, rates, exchanges, and their replay remain the only semantic authority.

`derive_document_with_frames` reports each frame as soon as replay verifies it,
allowing HTTP/SSE and browser Worker adapters to expose real incremental state
without giving transport callbacks any authority over the economy.

Quantities cross the JSON boundary as exact decimal strings, so JavaScript cannot silently round a 256-bit quantity. An ontology's identifiers are represented by a stable item path under the ontology's name, such as `maze/account/agent`, a label, and optional encoded JSON; integrations control those representations through `ViewOntology<O>` instead of exposing generic Rust types to TypeScript, and `PathOntology<O>` keys each by its written path, the form `#[derive(axionomy::Identifier)]` serializes it as (`maze/asset/edge/key-room/gate`), and labels it from its `Debug` output. Every key a view carries is a typed path (`ItemPath`, `MeasurePath`, `ClassPath`, `DocumentPath`, `ProposalPath`, `PhasePath`, `AlgorithmPath`), so a metric's key can never stand where an account's is expected; a key is written from an identifier's path (`axionomy::path::write`) or a literal checked when compiling (`axionomy::path!`), never from `Debug` text.

The kernel counts atoms, so a view says how people read them. `ViewOntology<O>::format` gives each asset a `QuantityFormat`, `atoms ÷ 10^scale` with a unit, and every document carries the formats of its assets that are not plain counts. A WAD-scaled rate of 2 × 10^15 atoms reads `0.2 %`, 1,250 cent atoms read `12.50 USD`, and the exact atoms are never lost: a format never rounds, and Studio shows the atom count beside every formatted value.

A problem presents itself with the same crate. `Label` names an identifier as
people read it and `Format` says how an asset's quantities are written;
`debug_labels!` and `counted!` implement them for identifiers labeled by their
`Debug` words and assets written as counts. `LabeledOntology` is the
`ViewOntology` those give, keyed by paths, with the problem's scene and
leaderboards. Scenes are built from `visual_entity`, `visual_metric` and
`asset_metric`, keyed through `Keyed` and tied to the economy by
`link_account` and `link_balance`; `exact_front`, `evaluated_front` and
`sampled_policy_front` show exact, evaluated and sampled Pareto fronts.
