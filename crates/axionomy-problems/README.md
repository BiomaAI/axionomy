# axionomy-problems

Canonical closed-problem encodings for
[Axionomy](https://github.com/BiomaAI/axionomy), and how each is studied and
shown.

Problem models exercise Axionomy exclusively through its public API. They are
reference encodings and conformance fixtures, not privileged engine semantics.
They are intentionally concrete examples, not reusable domain frameworks:
Axionomy users create their own economies. The fixtures prove that the common
axioms generalize, and their adversarial tests ensure accepted exchanges remain
domain-valid even when a caller bypasses the examples' action helpers and
rebinds roles directly. The mission additionally proves caller-owned belief
filtering and repeated planning across information sets.

The applicable fixtures also expose multi-objective decisions. Maze, workshop,
scheduling, bridge, marketplace, and perishables exhaust finite state spaces
and return exact fronts of replay-verified traces. Logistics, rescue, and
mission compare sampled policy statistics and therefore return explicitly
approximate fronts. In both cases the objective values come from encoded
outcomes; dominance is disposable comparison policy, never a parallel domain
model.

Every model's `World` has an `Ontology` naming its identifier types, such as
`maze::Ontology` or `logistics::fleet::Ontology`, and every identifier
serializes, so `axionomy-store` can record its runs: whatever explores a
model records each rollout as a run and a Monte Carlo estimate as a group of
them
(`tests/storage.rs`).

## One folder per problem

Each problem lives in one folder of `src`, named as its module (`maze`,
`work_league`):

- `README.md` explains the problem: its specification, the results its tests
  require, and the pressure it puts on the API. It is also the module's
  documentation.
- `mod.rs` is the model, with the model's own submodules beside it
  (`maze/dynamics.rs`).
- `study.rs` is the problem's `axionomy_study::Problem`, such as `maze::Maze`:
  its catalog entry, its strategies and the documents a study holds, each
  derived by replay. The catalog entry is the only copy of what each instance
  holds and what each strategy does.
- `view.rs` is its presentation: labels, units, scene and leaderboards.

Sub-ontologies such as `logistics/fleet` follow the same layout inside their
problem's folder. A model uses neither its study nor its view, so it remains a
pure domain encoding usable without Studio, and no viewer becomes part of a
problem's semantics.

`registry()` registers every problem, in catalog order, for the CLI, MCP and
Studio. The suite's index, with its conformance matrix and instance profiles,
is [`PROBLEMS.md`](https://github.com/BiomaAI/axionomy/blob/main/PROBLEMS.md).

## Study your own problem

A problem of your own is studied the same way. Implement
`axionomy_study::Problem`: `descriptor` gives its catalog entry, and `study`
gives the documents of the study a `StudyKey` names, built with the runtime's
helpers (`document` replays a trace into a view document, and `proposal`,
`telemetry`, `simulate` and `frontier_trace` add the evidence beside it). Its
ontology's identifiers derive `axionomy::Identifier` and implement
`axionomy_view::Label`, with `Format` for assets, so a document can name them.
A catalog entry declares its own family, what it exercises and its instances,
each a path with the words people read for it; Studio, MCP and the CLI show
those words and know nothing else about the problem. This one studies the
smallest maze by breadth-first search alone:

```rust
use axionomy::path;
use axionomy_problems::maze;
use axionomy_study::{
    Capability, DocumentSpec, InstanceDescriptor, Problem, ProblemDescriptor, ProblemFamily,
    Registry, StrategyDescriptor, StudyError, StudyKey, StudyProgress, StudyRequest, document,
    problem_error,
};
use axionomy_view::ViewDocument;

struct FewestMoves(ProblemDescriptor);

impl Problem for FewestMoves {
    fn descriptor(&self) -> &ProblemDescriptor {
        &self.0
    }

    fn study(
        &self,
        key: &StudyKey,
        progress: &mut StudyProgress<'_>,
    ) -> Result<Vec<ViewDocument>, StudyError> {
        let initial = maze::initial();
        let route = maze::solve_bfs(&initial)
            .ok_or_else(|| problem_error(&key.problem, "no route reaches the exit"))?;
        let spec = DocumentSpec {
            problem: &key.problem,
            document: path!("breadth-first"),
            title: "Fewest moves",
            description: "The route with the fewest exchanges.",
            source_label: "Fewest moves",
        };
        // Replays the route through the core, with no scene.
        let document = document(
            progress,
            spec,
            &initial,
            &maze::goal(),
            route.trace(),
            Vec::new(),
            |_, _| None,
        )
        .map_err(|error| problem_error(&key.problem, error))?;
        Ok(vec![document])
    }
}

let mut registry = Registry::new();
registry.register(FewestMoves(ProblemDescriptor {
    key: path!("fewest-moves"),
    title: "Fewest moves".into(),
    summary: "Reach the maze's exit in as few exchanges as possible.".into(),
    family: ProblemFamily::new(path!("pathfinding"), "Pathfinding"),
    default_instance: path!("smallest"),
    instances: vec![InstanceDescriptor {
        key: path!("smallest"),
        label: "Smallest".into(),
        description: "The smallest key-door maze".into(),
    }],
    default_strategy: path!("breadth-first"),
    strategies: vec![StrategyDescriptor {
        key: path!("breadth-first"),
        label: "Breadth-first".into(),
        description: "Tries every route one exchange longer at a time.".into(),
        algorithm: "breadth-first search".into(),
    }],
    capabilities: vec![Capability::new(path!("fewest-exchanges"), "Fewest exchanges")],
}))?;

let study = registry.study(StudyRequest::new(path!("fewest-moves")))?;
assert_eq!(study.key.instance, path!("smallest"));
assert_eq!(study.selected().map(|document| document.frames.len()), Some(3));
# Ok::<(), Box<dyn std::error::Error>>(())
```

`Registry::register` refuses a key already registered, so a problem can join
the samples in `registry()` or stand in a registry of its own. Hand
that registry to a front end: `axionomy_cli::run` or `run_from` for the CLI,
an `axionomy_engine::Engine` for `AxionomyMcp::new` or `stateless_http_service`
for the MCP server, `StudioState::new` for the Studio server, and `catalog_of`
and `study_in` for Studio's browser Worker. The `axionomy` application serves
`registry()` (`axionomy mcp`, `axionomy studio`); an application of your own
serves your registry with the same calls.

## Examples

Each model has a matching runnable example:

```console
cargo run -p axionomy-problems --example maze
cargo run -p axionomy-problems --example sokoban
cargo run -p axionomy-problems --example exact_cover
cargo run -p axionomy-problems --example workshop
cargo run -p axionomy-problems --example scheduling
cargo run -p axionomy-problems --example rescue
cargo run -p axionomy-problems --example bridge
cargo run -p axionomy-problems --example amm
cargo run -p axionomy-problems --example marketplace
cargo run -p axionomy-problems --example logistics
cargo run -p axionomy-problems --example connect_four
cargo run -p axionomy-problems --example mission
cargo run -p axionomy-problems --example perishables
cargo run -p axionomy-problems --example work_league
cargo run -p axionomy-problems --example lending
```

Examples emit structured console events through `tracing`. `INFO` is the
default and explains model construction, strategy choice, encoded outcomes,
and replay verification. Set `RUST_LOG=debug` to include complete accepted
exchange traces and assessments:

```console
RUST_LOG=debug cargo run -p axionomy-problems --example maze
```

Logging is initialized only by these example binaries. Problem modules,
search, and kernel libraries never install a global subscriber.

Performance probes are Cargo bench targets rather than examples:

```console
cargo bench -p axionomy-problems --bench rollout_throughput
```
