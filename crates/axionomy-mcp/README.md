# axionomy-mcp

`axionomy-mcp` is the stateless MCP 2026-07-28 reference boundary for Axionomy. It exposes immutable, content-addressed economy snapshots and durable task-backed search without putting transport, persistence, or async-runtime policy into the core engine.

The server deliberately targets only MCP `2026-07-28`. Every operation names an explicit `economy_id`; successful transitions create a new snapshot rather than changing hidden session state.
