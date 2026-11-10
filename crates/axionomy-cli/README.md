# axionomy-cli

Human- and script-friendly access to the same interface-neutral service used by
Axionomy Studio and its HTTP/MCP adapters.

```console
cargo run -p axionomy-cli -- catalog
cargo run -p axionomy-cli -- describe logistics
cargo run -p axionomy-cli -- study logistics --instance showcase --strategy reliable
cargo run -p axionomy-cli -- study marketplace --instance micro --format json > marketplace.json
```

Every problem advertises Micro, Showcase, and Stress instances. Omitting
`--instance` selects Showcase, and the study records its key: the problem,
instance, strategy, seed and budget it was computed from. Problems and
strategies are written in kebab-case (`work-league`, `a-star`), and the CLI
refuses any other spelling when it reads them.
