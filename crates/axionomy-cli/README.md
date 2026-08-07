# axionomy-cli

Human- and script-friendly access to the same interface-neutral service used by
Axionomy Studio and its HTTP/MCP adapters.

```console
cargo run -p axionomy-cli -- catalog
cargo run -p axionomy-cli -- describe logistics
cargo run -p axionomy-cli -- run logistics --instance showcase --strategy reliable
cargo run -p axionomy-cli -- run marketplace --instance micro --format json > marketplace.json
```

Every problem advertises Micro, Showcase, and Stress instances. Omitting
`--instance` selects Showcase and the resolved identity is recorded in the
artifact.
