#![doc = include_str!("../README.md")]

mod cache;
pub mod script;

pub use rhai;
pub use script::{HOST_API, Random, ScriptPath, ScriptState, exchanges_of};

use axionomy::paths::{self, Paths};
use axionomy_store::{Checkpoints, DefinitionPath, Durability, Point, Reader, Store, StoreError};
use cache::Cache;
use std::{num::NonZeroU64, sync::Arc};
use thiserror::Error;

/// A state the store holds, written as paths: the economy, and its canonical
/// tree, whose hash is the state's root.
pub type State = axionomy_store::State<Paths>;

/// States a run of applied exchanges stores besides its first and last:
/// reaching any of its steps replays at most this many exchanges.
const CHECKPOINT_EVERY: NonZeroU64 = NonZeroU64::new(64).expect("nonzero");
/// Points kept in memory with their states and receipts.
const CACHED: usize = 32;

#[derive(Debug, Clone, Error)]
pub enum EngineError {
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error("step {} of run {} is not a point of this store", .0.step, .0.run)]
    UnknownPoint(Point),
    #[error("step {step} is not on the history of step {} of run {}", .at.step, .at.run)]
    NotOnHistory { at: Point, step: u64 },
    /// An exchange of those applied together did not apply, so none was
    /// recorded. `index` counts from zero, and `assessment` explains the
    /// failure against the economy that exchange met.
    #[error("exchange {index} does not apply: it is {:?}", .assessment.status())]
    Refused {
        index: usize,
        assessment: Box<paths::Assessment>,
    },
    #[error("the store has no script `{0}`")]
    UnknownScript(ScriptPath),
    /// A script that does not compile, or failed while it ran; `message` is
    /// Rhai's, with the position it names.
    #[error("script `{path}`: {message}")]
    Script { path: ScriptPath, message: String },
    #[error("a script returned {found} where {expected} was expected")]
    Returned {
        expected: &'static str,
        found: String,
    },
}

/// A point as the engine serves it: the state there, the point its run
/// started from, if it forked, and the receipts of the steps its run took to
/// reach it, in order.
#[derive(Debug)]
pub struct Reached {
    pub point: Point,
    pub state: State,
    /// The point the run forked from, whose state is the run's first; none for
    /// a run that starts from an economy of its own.
    pub from: Option<Point>,
    pub receipts: Vec<paths::Receipt>,
}

/// The work every front end shares, over a store and nothing registered.
#[derive(Clone)]
pub struct Engine {
    store: Store,
    reached: Cache<Point, Reached>,
    host: Arc<rhai::Engine>,
    scripts: Cache<ScriptPath, rhai::AST>,
}

impl Engine {
    /// An engine keeping what it records in `store`.
    pub fn new(store: Store) -> Result<Self, EngineError> {
        Ok(Self {
            store,
            reached: Cache::new(CACHED),
            host: Arc::new(script::host()),
            scripts: Cache::new(CACHED),
        })
    }

    /// Keeps `source` as the script at `path`, replacing any script there,
    /// and refuses one that does not compile.
    pub fn keep_script(&self, path: &ScriptPath, source: &str) -> Result<(), EngineError> {
        let ast = self.compile(path, source)?;
        let name = definition("script", path);
        let bytes = source.as_bytes().to_vec();
        self.store
            .transact(Durability::Immediate, move |txn| {
                txn.define(&name, Some(&bytes))
            })
            .wait()?;
        self.scripts.insert(path.clone(), ast);
        Ok(())
    }

    /// The source of the script at `path`.
    pub fn script(&self, path: &ScriptPath) -> Result<Option<String>, EngineError> {
        let bytes = self.store.read()?.definition(&definition("script", path))?;
        bytes
            .map(|bytes| {
                String::from_utf8(bytes).map_err(|_| {
                    EngineError::Store(StoreError::Malformed("a script that is not text"))
                })
            })
            .transpose()
    }

    /// Calls `function` of the script at `path` with `arguments`, Rhai values
    /// such as a [`ScriptState`], a [`Random`] or a [`axionomy::Quantity`],
    /// and returns what it returns, which must be a `T`.
    pub fn call<T: Clone + Send + Sync + 'static>(
        &self,
        path: &ScriptPath,
        function: &str,
        arguments: impl rhai::FuncArgs,
    ) -> Result<T, EngineError> {
        let ast = match self.scripts.get(path) {
            Some(ast) => ast,
            None => {
                let source = self
                    .script(path)?
                    .ok_or_else(|| EngineError::UnknownScript(path.clone()))?;
                let ast = self.compile(path, &source)?;
                self.scripts.insert(path.clone(), ast)
            }
        };
        let returned: rhai::Dynamic = self
            .host
            .call_fn(&mut rhai::Scope::new(), &ast, function, arguments)
            .map_err(|error| EngineError::Script {
                path: path.clone(),
                message: error.to_string(),
            })?;
        let found = returned.type_name();
        returned
            .try_cast::<T>()
            .ok_or_else(|| EngineError::Returned {
                expected: std::any::type_name::<T>(),
                found: found.to_owned(),
            })
    }

    fn compile(&self, path: &ScriptPath, source: &str) -> Result<rhai::AST, EngineError> {
        self.host
            .compile(source)
            .map_err(|error| EngineError::Script {
                path: path.clone(),
                message: error.to_string(),
            })
    }

    pub fn store(&self) -> &Store {
        &self.store
    }

    /// Records `economy` as the first point of a run of its own.
    pub fn author(&self, economy: paths::Economy) -> Result<Point, EngineError> {
        let state = State::new(economy);
        let point = Point {
            run: self.store.mint_run(),
            step: 0,
        };
        let tree = state.tree.clone();
        self.store
            .transact(Durability::Immediate, move |txn| {
                txn.create_run::<Paths>(point.run, None, 0, &tree, Checkpoints::Ends)?;
                txn.pin(point, None)
            })
            .wait()?;
        self.reached.insert(
            point,
            Reached {
                point,
                state,
                from: None,
                receipts: Vec::new(),
            },
        );
        Ok(point)
    }

    /// Applies `exchanges` at `at`, all of them or none, and records them as a
    /// run forked from `at`, returning the point they reach. The point `at`
    /// never changes. Applying no exchanges reaches `at` itself.
    pub fn apply(&self, at: Point, exchanges: &[paths::Exchange]) -> Result<Point, EngineError> {
        if exchanges.is_empty() {
            return Ok(at);
        }
        let source = self.reach(at)?;
        let mut state = source.state.clone();
        let mut steps = Vec::with_capacity(exchanges.len());
        for (index, exchange) in exchanges.iter().enumerate() {
            let Ok(receipt) = state.apply(exchange.clone()) else {
                return Err(EngineError::Refused {
                    index,
                    assessment: Box::new(state.economy.assess(exchange)),
                });
            };
            let step = at.step + index as u64 + 1;
            steps.push((receipt, checkpoint(step).then(|| state.tree.clone())));
        }
        let run = self.store.mint_run();
        let last = Point {
            run,
            step: at.step + steps.len() as u64,
        };
        let receipts = steps.iter().map(|(receipt, _)| receipt.clone()).collect();
        let first = source.state.tree.clone();
        let last_tree = state.tree.clone();
        self.store
            .transact(Durability::Immediate, move |txn| {
                txn.create_run::<Paths>(
                    run,
                    Some(at),
                    at.step,
                    &first,
                    Checkpoints::Every(CHECKPOINT_EVERY),
                )?;
                for (index, (receipt, tree)) in steps.into_iter().enumerate() {
                    let point = Point {
                        run,
                        step: at.step + index as u64 + 1,
                    };
                    txn.put_step::<Paths>(point, &receipt, tree.as_ref())?;
                }
                txn.pin(last, Some(&last_tree))
            })
            .wait()?;
        self.reached.insert(
            last,
            Reached {
                point: last,
                state,
                from: Some(at),
                receipts,
            },
        );
        Ok(last)
    }

    /// The state at `at`, with the receipts of the steps its run took to
    /// reach it, each replayed and checked against what was recorded.
    pub fn reach(&self, at: Point) -> Result<Arc<Reached>, EngineError> {
        if let Some(reached) = self.reached.get(&at) {
            return Ok(reached);
        }
        let reader = self.store.read()?;
        let run = reader
            .run_of::<Paths>(at.run)?
            .ok_or(EngineError::UnknownPoint(at))?;
        let last = reader
            .last_step(at.run)?
            .ok_or(EngineError::UnknownPoint(at))?;
        if !(run.first..=last).contains(&at.step) {
            return Err(EngineError::UnknownPoint(at));
        }
        let mut state = first_state(&reader, at, run.first)?;
        let receipts = reader.replay(&mut state, at.run, run.first, at.step)?;
        Ok(self.reached.insert(
            at,
            Reached {
                point: at,
                state,
                from: run.from,
                receipts,
            },
        ))
    }

    /// The point at step `step` of `at`'s history, its state stored so it is
    /// reached as directly as `at`. The history of a run that forked continues
    /// into the run it forked from.
    pub fn ancestor(&self, at: Point, step: u64) -> Result<Point, EngineError> {
        if step > at.step {
            return Err(EngineError::NotOnHistory { at, step });
        }
        let reader = self.store.read()?;
        let mut on = at;
        let point = loop {
            let run = reader
                .run_of::<Paths>(on.run)?
                .ok_or(EngineError::UnknownPoint(on))?;
            if step >= run.first {
                break Point { run: on.run, step };
            }
            on = run.from.ok_or(EngineError::NotOnHistory { at, step })?;
        };
        let state = reader
            .state::<Paths>(point)?
            .ok_or(EngineError::UnknownPoint(point))?;
        self.store
            .transact(Durability::Immediate, move |txn| {
                txn.pin(point, Some(&state.tree))
            })
            .wait()?;
        Ok(point)
    }

    /// Whether `exchange` would apply at the economy `before` reaches from
    /// `at`, and what it would move or what it lacks. Nothing is recorded.
    pub fn assess(
        &self,
        at: Point,
        before: &[paths::Exchange],
        exchange: &paths::Exchange,
    ) -> Result<paths::Assessment, EngineError> {
        let reached = self.reach(at)?;
        let mut economy = reached.state.economy.clone();
        for (index, earlier) in before.iter().enumerate() {
            if economy.apply(earlier.clone()).is_err() {
                return Err(EngineError::Refused {
                    index,
                    assessment: Box::new(economy.assess(earlier)),
                });
            }
        }
        Ok(economy.assess(exchange))
    }
}

/// The first state of `at`'s run, which starts at step `first`.
fn first_state(reader: &Reader, at: Point, first: u64) -> Result<State, EngineError> {
    let point = Point {
        run: at.run,
        step: first,
    };
    reader
        .state::<Paths>(point)?
        .ok_or(EngineError::UnknownPoint(point))
}

/// The name a definition of `kind` at `path` is kept under, such as
/// `script/maze/heuristic`.
fn definition<K>(kind: &str, path: &axionomy::path::Path<K>) -> DefinitionPath {
    format!("{kind}/{path}")
        .parse()
        .expect("a kind and a path make a path")
}

/// Whether a run of applied exchanges stores the state after step `step`
/// before its last.
fn checkpoint(step: u64) -> bool {
    step.is_multiple_of(CHECKPOINT_EVERY.get())
}
