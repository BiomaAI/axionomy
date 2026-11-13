# axionomy-cli

The `axionomy` application. It registers the canonical problems
(`axionomy_problems::registry()`) and studies them, or serves them: over MCP
with `axionomy mcp`, and to Studio with `axionomy studio`. The transports are
libraries that know no problem; this is where problems meet them. `run` and
`run_from` take the `Registry` to serve, so an application of your own reuses
them with its own problems. Planned: studies go through an `axionomy-engine`
like every other front end
([plan](../../docs/SIMULATION.md#interactive-execution), D-045).

```console
cargo run -p axionomy-cli -- catalog
cargo run -p axionomy-cli -- describe logistics
cargo run -p axionomy-cli -- study logistics --instance showcase --strategy reliable
cargo run -p axionomy-cli -- study marketplace --instance micro --format json > marketplace.json
cargo run -p axionomy-cli -- export-studies studio/public/studies
cargo run -p axionomy-cli -- mcp --bind 127.0.0.1:8000 --store economies.redb
cargo run -p axionomy-cli -- studio --bind 127.0.0.1:3000
```

`mcp` serves MCP over Streamable HTTP at `/mcp` with `/health`, keeping
economies in a store file when `--store` names one and in memory otherwise.
`studio` serves Studio's API and the frontend built in `studio/dist`
(`--frontend`). Each flag can be given by its environment variable instead:
`AXIONOMY_MCP_BIND`, `AXIONOMY_MCP_STORE`, `AXIONOMY_STUDIO_BIND`,
`AXIONOMY_STUDIO_DIR`.

Every problem declares its instances, and `describe` lists them with its
family and capabilities in the words the problem gives them; the canonical
problems declare `micro`, `showcase` and `stress`. Omitting `--instance`
selects the problem's default, Showcase for the canonical ones, and the study
records its key: the problem, instance, strategy, seed and budget it was
computed from. Problems, instances and strategies are written in kebab-case
(`work-league`, `showcase`, `a-star`), and the CLI refuses any other spelling
when it reads them.
