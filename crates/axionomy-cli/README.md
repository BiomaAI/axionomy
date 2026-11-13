# axionomy-cli

The `axionomy` application. It registers the sample problems
(`axionomy_problems::registry()`) and studies them, or serves them: over MCP
with `axionomy mcp`, and to Studio with `axionomy studio`. The transports are
libraries that know no problem; this is where problems meet them. `run` and
`run_from` take the `Registry` to serve, so an application of your own reuses
them with its own problems. Planned: problems and studies give way to
packages, redb files the application loads, and experiments on them
([plan](../../docs/SIMULATION.md#plan-axionomy-on-primitives), D-048, D-050).

```console
cargo run -p axionomy-cli -- catalog
cargo run -p axionomy-cli -- describe logistics
cargo run -p axionomy-cli -- study logistics --instance showcase --strategy reliable
cargo run -p axionomy-cli -- study marketplace --instance micro --format json > marketplace.json
cargo run -p axionomy-cli -- export-studies studio/public/studies
cargo run -p axionomy-cli -- mcp --bind 127.0.0.1:8000 --store economies.redb
cargo run -p axionomy-cli -- studio --bind 127.0.0.1:3000
cargo run -p axionomy-cli -- package export work.redb maze.zip --name maze --version 1.0.0
cargo run -p axionomy-cli -- package inspect maze.zip
cargo run -p axionomy-cli -- package load mine.redb maze.zip
```

`package export` writes a package of everything a store names under
`--name`, or of the `--ref` and `--definition` given, with all they reach,
to a new file, zipped when its name ends in `.zip`; `package load` merges
one into a store file, creating it if needed; `package inspect` lists a
package's manifest, refs and definitions.

`mcp` serves MCP over Streamable HTTP at `/mcp` with `/health`, keeping
economies in a store file when `--store` names one and in memory otherwise.
`studio` serves Studio's API and the frontend built in `studio/dist`
(`--frontend`). Each flag can be given by its environment variable instead:
`AXIONOMY_MCP_BIND`, `AXIONOMY_MCP_STORE`, `AXIONOMY_STUDIO_BIND`,
`AXIONOMY_STUDIO_DIR`.

Every problem declares its instances, and `describe` lists them with its
family and capabilities in the words the problem gives them; the canonical
problems declare `micro`, `showcase` and `stress`. Omitting `--instance`
selects the problem's default, Showcase for the samples, and the study
records its key: the problem, instance, strategy, seed and budget it was
computed from. Problems, instances and strategies are written in kebab-case
(`work-league`, `showcase`, `a-star`), and the CLI refuses any other spelling
when it reads them.
