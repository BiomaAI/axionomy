# axionomy-view

`axionomy-view` is Axionomy's runtime-neutral presentation boundary. It replays an accepted exchange trace through the core engine and derives portable snapshots, assessments, receipts, account deltas, and optional scenes for a viewer. These values are explanatory projections: assets, accounts, rates, exchanges, and their replay remain the only semantic authority.

Quantities cross the JSON boundary as exact decimal strings, so JavaScript cannot silently round a `u64` or larger numeric backend. User-defined ontology identifiers are represented by a stable presentation key, a label, and optional encoded JSON; integrations control those representations through `ViewOntology` instead of exposing generic Rust types to TypeScript.
