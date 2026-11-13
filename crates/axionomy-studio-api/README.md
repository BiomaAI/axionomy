# axionomy-studio-api

What Studio asks of an engine, once: the operations Studio needs, written
over `axionomy-engine`'s `Engine`, and the types they take and give. The
Studio server (`axionomy-studio-server`) serves them over HTTP and documents
them in OpenAPI; the browser Worker (`axionomy-studio-wasm`) calls the same
functions and posts the same JSON, so Studio talks to the native and the
browser engine through one TypeScript interface built on the generated
types ([plan](../../docs/SIMULATION.md#plan-axionomy-on-primitives), P12,
D-045). Nothing here is an operation the engine lacks: each composes the
engine's own and shapes its answer for the wire.

| Operation | Engine | Gives |
| --- | --- | --- |
| `packages` | `manifests`, `named_under`, `referred`, `shipped_by`, `experiment`, `objective` | `PackageList`: each package's manifest, its refs with their points (those it ships marked `shipped_by`, fixed), experiments, strategies, objectives, goals and candidate sets |
| `load` | `package`, `load` | `LoadedView`: the packages a file held, the names the store kept, and the versions it replaced |
| `draw` | `draw` | a point's history as a `ViewDocument`, drawn with its view |
| `ancestor` | `ancestor` | the point at an earlier step of a history |
| `balances` | `reach` | `Balances`: every account at a point, in path order, with what it holds, assets and exact quantities by the paths exchanges bind |
| `candidates` | `candidates`, `present` | each exchange a candidate set offers at a point, assessed there and read with the point's view |
| `assess` | `present` | a written exchange, assessed at a point; nothing is recorded |
| `apply` | `apply` | the new point exchanges reach, all of them or none |
| `run_strategy` | `run_strategy` | where a strategy run from a point toward a goal ended, how, and its search's thinking |
| `run_experiment`, `results` | `run_experiment`, `results` | every arm's trials with their seeds, ends and exact objective values |
| `compare` | `compare`, `pick` | points' objective values, the front no point dominates, and the best by objectives in order |
| `create_ref`, `read_ref`, `advance_ref`, `delete_ref`, `frames` | `refer`, `referred`, `shipped_by`, `advance`, `delete_ref`, `frames` | refs, moved forward or deleted only from where the mover saw them; a ref a package ships is fixed |

A point is written `PointId` (`run-…:3`), and a 64-bit number JSON cannot
carry exactly, such as a seed, a budget or turns, `Natural`, in decimal; quantities
are exact decimal text, as everywhere. A refusal is a `Refusal`, with the
status HTTP answers it with: 404 for what the store lacks, 400 for a request
that cannot be served as asked, 403 for what is never done however it is
asked, such as moving or deleting a ref a package ships, 409 with `RefMoved`
when a ref is not where its mover saw it, 500 when the engine fails.
`StrategyEvent` and `ExperimentEvent` report a run's progress before its
end, which a transport streams: a search's counts, or a policy's or a game's
turns counted as a search counts expansions. A watcher stops a run of any
kind, and an experiment stopped keeps no results.
