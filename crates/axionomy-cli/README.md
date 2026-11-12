# axionomy-cli

Human- and script-friendly access to the same interface-neutral study runtime
used by Axionomy Studio and its HTTP/MCP adapters. `execute` and `run_from`
study the problems of the `Registry` they are given; the `axionomy` binary
uses `axionomy_problems::registry()`.

```console
cargo run -p axionomy-cli -- catalog
cargo run -p axionomy-cli -- describe logistics
cargo run -p axionomy-cli -- study logistics --instance showcase --strategy reliable
cargo run -p axionomy-cli -- study marketplace --instance micro --format json > marketplace.json
```

Every problem declares its instances, and `describe` lists them with its
family and capabilities in the words the problem gives them; the canonical
problems declare `micro`, `showcase` and `stress`. Omitting `--instance`
selects the problem's default, Showcase for the canonical ones, and the study
records its key: the problem, instance, strategy, seed and budget it was
computed from. Problems, instances and strategies are written in kebab-case
(`work-league`, `showcase`, `a-star`), and the CLI refuses any other spelling
when it reads them.
