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
| `packages` | `manifests`, `named_under`, `referred`, `experiment`, `objective` | `PackageList`: each package's manifest, its refs with their points, experiments, strategies, objectives, goals and candidate sets |
| `load` | `package`, `load` | `LoadedView`: the packages a file held, and the names the store kept |
| `draw` | `draw` | a point's history as a `ViewDocument`, drawn with its view |
| `ancestor` | `ancestor` | the point at an earlier step of a history |
| `candidates` | `candidates`, `present` | each exchange a candidate set offers at a point, assessed there and read with the point's view |
| `assess` | `present` | a written exchange, assessed at a point; nothing is recorded |
| `apply` | `apply` | the new point exchanges reach, all of them or none |
| `run_strategy` | `run_strategy` | where a strategy run from a point toward a goal ended, how, and its search's thinking |
| `run_experiment`, `results` | `run_experiment`, `results` | every arm's trials with their seeds, ends and exact objective values |
| `compare` | `compare`, `pick` | points' objective values, the front no point dominates, and the best by objectives in order |
| `create_ref`, `read_ref`, `advance_ref`, `frames` | `refer`, `referred`, `advance`, `frames` | refs, moved forward only from where the mover saw them |

A point is written `PointId` (`run-…:3`), and a 64-bit number JSON cannot
carry exactly, such as a seed, a budget or turns, `Natural`, in decimal; quantities
are exact decimal text, as everywhere. A refusal is a `Refusal`, with the
status HTTP answers it with: 404 for what the store lacks, 400 for a request
that cannot be served as asked, 409 with `RefMoved` when a ref is not where
its mover saw it, 500 when the engine fails. `StrategyEvent` and
`ExperimentEvent` report a run's search progress before its end, which a
transport streams; a watcher that stops a search stops the run, and an
experiment stopped keeps no results.
