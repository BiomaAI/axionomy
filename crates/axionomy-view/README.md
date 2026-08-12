# axionomy-view

`axionomy-view` is Axionomy's runtime-neutral presentation boundary. It replays an accepted exchange trace through the core engine and derives portable snapshots, assessments, receipts, account deltas, frame cues, bounded solver observations, optional scenes, and optional per-snapshot leaderboards for a viewer. A scene separates geometric surface from semantic entities, typed references to the accounts or balances those entities illustrate, paths, exact metrics, annotations, and a constrained glyph vocabulary. Those stable identities, anchors, evidence links, and receipt deltas are sufficient for a generic client to compose deterministic movement and economic effects without embedding problem-specific behavior. Leaderboards retain exact score text, eligibility, rank, participant identity, and explanatory components without becoming a score store. Anchors, identities, and evidence references are validated before publication. These values are explanatory projections: assets, accounts, rates, exchanges, and their replay remain the only semantic authority.

`derive_document_with_frames` reports each frame as soon as replay verifies it,
allowing HTTP/SSE and browser Worker adapters to expose real incremental state
without giving transport callbacks any authority over the economy.

Quantities cross the JSON boundary as exact decimal strings, so JavaScript cannot silently round a `u64` or larger numeric backend. User-defined ontology identifiers are represented by a stable presentation key, a label, and optional encoded JSON; integrations control those representations through `ViewOntology` instead of exposing generic Rust types to TypeScript.
