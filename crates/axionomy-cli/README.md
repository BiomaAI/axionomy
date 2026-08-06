# axionomy-cli

Human- and script-friendly access to the same interface-neutral service used by
Axionomy Studio and its HTTP/MCP adapters.

```console
cargo run -p axionomy-cli -- catalog
cargo run -p axionomy-cli -- describe logistics
cargo run -p axionomy-cli -- run logistics --strategy reliable
cargo run -p axionomy-cli -- run marketplace --format json > marketplace.json
```
