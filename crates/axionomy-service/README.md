# axionomy-service

The interface-neutral application boundary for Axionomy. It describes
capabilities, accepts reproducible run requests, and returns replay-derived
artifacts without depending on HTTP, CLI, MCP, async runtimes, databases, or a
browser.

`ReferenceService` adapts the twelve canonical conformance problems. External
interfaces call the same service and are tested for semantic parity.
