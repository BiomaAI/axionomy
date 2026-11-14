//! Scripts: the Rhai source a package carries for anything a generic
//! algorithm cannot know (D-049): candidates, policies, Nature responses,
//! heuristics, objectives, view logic.
//!
//! A script reads an economy through the host API and proposes exchanges; it
//! never changes one. Quantities are exact ([`Quantity`], with checked
//! arithmetic), and the only randomness is a [`Random`] the host seeds, which
//! draws the world's chance by named event, so a script gives the same answer
//! for the same arguments everywhere. Rhai's maps iterate in key order, and
//! scripts have no clock.
//!
//! A script imports another the store keeps by its path,
//! `import "maze/routes" as routes;`, and calls its functions as
//! `routes::shortest(...)`. Imports resolve over kept scripts only, never a
//! file or the network; each imported script is compiled and evaluated once,
//! as a module, and sees the same host API.

use crate::{EngineError, ScriptFunction, cache::Cache, definitions::definition};
use axionomy::{
    AccountPath, AssessmentStatus, AssetPath, ObservationKey, PathKind, Quantity, RatePath,
    RolePath, Rounding,
    path::Path,
    paths::{self, Paths},
};
use axionomy_search::sampling::{
    self, EventPath, EventTickets, SamplingError, SeededSampler, TicketSource, WeightedExchange,
    splitmix,
};
use axionomy_store::{Store, StoreError};
use rhai::{
    ASTNode, Array, Dynamic, EvalAltResult, Expr, ImmutableString, Map, Module, Position, Stmt,
};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque, btree_map::Entry},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
};

/// The version of the host API this engine gives scripts. A package records
/// the version its scripts were written against. It stays 1 while Axionomy is
/// in development, whatever the API gains or changes; it counts from the
/// first release that packages are published against.
pub const HOST_API: u64 = 1;

/// The most operations one call of a script may run before it is stopped.
const MAX_OPERATIONS: u64 = 50_000_000;
/// Scripts kept compiled in memory, and imported scripts kept evaluated.
const COMPILED: usize = 32;
/// The deepest a script's expressions may nest at its top level; half that
/// inside its functions.
const MAX_DEPTH: usize = 256;
/// The deepest function calls may nest, a call into an imported script among
/// them. Rhai's default is 8 in a debug build and 64 in a release one; a
/// script runs alike in both.
const MAX_CALL_LEVELS: usize = 64;

/// The kind of path a script is named by.
pub mod kind {
    use axionomy::PathKind;

    /// A script a package carries.
    pub enum Script {}

    impl PathKind for Script {
        const SCHEMA: &'static str = "ScriptPath";
        const EXAMPLE: &'static str = "maze/heuristic";
    }
}

/// A script's name, such as `maze/heuristic`.
pub type ScriptPath = Path<kind::Script>;

type Failure = Box<EvalAltResult>;

/// An economy as a script reads it: all of it, or only the accounts an actor
/// may see.
#[derive(Clone)]
pub struct ScriptState {
    economy: paths::Economy,
    visible: Option<Arc<BTreeSet<AccountPath>>>,
}

impl ScriptState {
    pub fn new(economy: paths::Economy) -> Self {
        Self {
            economy,
            visible: None,
        }
    }

    pub fn economy(&self) -> &paths::Economy {
        &self.economy
    }

    /// What this state shows: the observation of the accounts a view sees,
    /// or of every account of the whole economy. Two states showing the same
    /// observation answer every question a script may ask of them alike.
    pub fn observation(&self) -> ObservationKey<Paths> {
        match &self.visible {
            Some(visible) => self.economy.view(visible.iter().cloned()),
            None => self
                .economy
                .view(self.economy.accounts().map(|(account, _)| account.clone())),
        }
        .observation_key()
    }

    fn sees(&self, account: &AccountPath) -> bool {
        self.visible
            .as_ref()
            .is_none_or(|visible| visible.contains(account))
    }

    /// The whole economy, refusing a view: whether an exchange applies may
    /// depend on accounts an actor cannot see.
    fn whole(&self, asked: &str) -> Result<&paths::Economy, Failure> {
        match self.visible {
            None => Ok(&self.economy),
            Some(_) => Err(format!("{asked} is not observable from a view").into()),
        }
    }
}

/// The host's seeded random numbers: the only randomness a script has. It
/// draws two kinds of chance.
///
/// The world's chance is drawn by named event: `random.next(event, bound)`
/// and `random.sample(event, weights)`, where `event` is a path naming the
/// occurrence the draw is for, such as `weather/direct-out/12`. The same
/// event drawn with the same seed gives the same outcome whatever else was
/// drawn before it, and different events are drawn independently, so a run
/// that makes another choice meets the same luck wherever it meets the same
/// event. A name must tell each occurrence apart: the weather of a road at
/// each time it is travelled, not the road alone.
///
/// A decision-maker's own randomness, such as a rollout exploring, is drawn
/// in sequence: `random.next(bound)` and `random.sample(weights)` take the
/// next of their sequences, so they depend on everything drawn before. Draws
/// by event never move a sequence. Clones share their sequences, so a
/// generator passed to a script call after call keeps drawing where it left
/// off.
#[derive(Debug, Clone)]
pub struct Random {
    /// The seed of the world whose events this generator draws: the run's.
    world: u64,
    /// The seed of this generator's own sequences.
    seed: u64,
    drawn: Arc<AtomicU64>,
    /// The tickets weighted samples in sequence draw, a sequence of their
    /// own: those `axionomy_search`'s sampler of the same seed draws, so a
    /// script samples weighted outcomes as Rust code seeded alike does.
    tickets: Arc<Mutex<SeededSampler>>,
}

impl Random {
    /// The generator of a run seeded `seed`: the world's events and its own
    /// sequences are drawn from that seed.
    pub fn new(seed: u64) -> Self {
        Self::apart(seed, seed)
    }

    /// A generator in the world of a run seeded `world`, drawing its events
    /// as every generator of that run does, whose own sequences are seeded
    /// `seed`: a decision-maker's randomness kept apart from Nature's.
    pub fn apart(world: u64, seed: u64) -> Self {
        Self {
            world,
            seed,
            drawn: Arc::new(AtomicU64::new(0)),
            tickets: Arc::new(Mutex::new(SeededSampler::new(seed))),
        }
    }

    /// What `draw` gives with this generator while its weighted samples
    /// draw `tickets`' tickets, as Rust code drawing from `tickets` would,
    /// leaving `tickets` where the draws stopped: how a search lends its
    /// sampler to the scripts it calls.
    pub(crate) fn lent<T>(&self, tickets: &mut SeededSampler, draw: impl FnOnce(Self) -> T) -> T {
        let swap = |tickets: &mut SeededSampler| {
            let mut held = self
                .tickets
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            std::mem::swap(&mut *held, tickets);
        };
        swap(tickets);
        let drawn = draw(self.clone());
        swap(tickets);
        drawn
    }

    /// A number below `bound`, the next of this generator's sequence.
    fn below(&self, bound: u64) -> u64 {
        let value = splitmix(self.seed, self.drawn.fetch_add(1, Ordering::Relaxed));
        // The high half of a 128-bit product: uniform enough, and exact.
        ((u128::from(value) * u128::from(bound)) >> 64) as u64
    }

    /// The number below `bound` keyed by `key`: value 1 of the SplitMix64
    /// sequence that starts at the seed XOR `key`, modulo `bound`, the
    /// ticket the samples draw for what they sample (a job's attempt). The
    /// same key gets the same number whatever else is drawn, so runs that
    /// differ elsewhere meet the same chances: common random numbers.
    fn keyed(&self, key: u64, bound: u64) -> u64 {
        splitmix(self.seed ^ key, 1) % bound
    }

    /// The tickets of the event `event` in this generator's world.
    fn event(&self, event: &str) -> Result<EventTickets, Failure> {
        let event: EventPath = event.parse().map_err(|error| {
            Failure::from(format!(
                "`{event}` is not an event: {error}; name an event by a path such as `{}`",
                <sampling::kind::Event as PathKind>::EXAMPLE
            ))
        })?;
        Ok(EventTickets::new(self.world, &event))
    }

    /// The number below `bound` the event `event` draws: the same for the
    /// same event and seed, whatever else is drawn. It is the index
    /// `sample(event, weights)` draws from `bound` equal weights.
    fn below_for(&self, event: &str, bound: u64) -> Result<u64, Failure> {
        Ok(self.event(event)?.ticket(bound))
    }

    /// The index of one of `weights`, integers or quantities, drawn by the
    /// next ticket as [`sampling::sample`] draws one of weighted outcomes.
    fn sample(&self, weights: Array) -> Result<i64, Failure> {
        let outcomes = weighted(weights)?;
        let mut tickets = self
            .tickets
            .lock()
            .map_err(|_| Failure::from("the sampler failed while drawing"))?;
        drawn(sampling::sample(&outcomes, &mut *tickets))
    }

    /// The index of one of `weights`, integers or quantities, that the event
    /// `event` draws, as [`sampling::sample`] draws one of weighted outcomes
    /// with the event's tickets.
    fn sample_for(&self, event: &str, weights: Array) -> Result<i64, Failure> {
        let mut tickets = self.event(event)?;
        drawn(sampling::sample(&weighted(weights)?, &mut tickets))
    }
}

/// `weights`, integers or quantities, as the outcomes of their indices.
fn weighted(weights: Array) -> Result<Vec<WeightedExchange<usize>>, Failure> {
    weights
        .into_iter()
        .enumerate()
        .map(|(index, weight)| Ok(WeightedExchange::new(index, weight_of(weight)?)))
        .collect()
}

/// The index a weighted sample drew, or why it drew none.
fn drawn(sampled: Result<&usize, SamplingError>) -> Result<i64, Failure> {
    let index = sampled.map_err(|error| Failure::from(error.to_string()))?;
    i64::try_from(*index).map_err(|_| "an index out of range".into())
}

/// `bound` as a bound a draw is below: a positive number.
fn bound_of(bound: i64) -> Result<u64, Failure> {
    u64::try_from(bound)
        .ok()
        .filter(|bound| *bound > 0)
        .ok_or_else(|| Failure::from(format!("{bound} is not a positive bound")))
}

/// A weight a script gives: a non-negative integer or a quantity.
fn weight_of(weight: Dynamic) -> Result<u64, Failure> {
    if weight.is::<Quantity>() {
        let quantity = weight.cast::<Quantity>();
        return u64::try_from(quantity)
            .map_err(|_| format!("weight {quantity} is too large").into());
    }
    weight
        .as_int()
        .ok()
        .and_then(|weight| u64::try_from(weight).ok())
        .ok_or_else(|| format!("{weight} is not a weight").into())
}

/// What `rate` fixes for `role` to consume, produce or preserve, as
/// `select` reads it, asset by asset; computed amounts are not fixed.
fn effect(
    state: &ScriptState,
    rate: &str,
    role: &str,
    select: impl for<'a> Fn(&'a paths::Rate, &RolePath) -> Option<&'a paths::Basket>,
) -> Result<Map, Failure> {
    let rate: RatePath = parsed(rate)?;
    let role: RolePath = parsed(role)?;
    let definition = state
        .economy
        .rate(&rate)
        .ok_or_else(|| Failure::from(format!("the economy has no rate `{rate}`")))?;
    Ok(select(definition, &role)
        .into_iter()
        .flat_map(|basket| basket.iter())
        .map(|(asset, quantity)| (asset.as_str().into(), Dynamic::from(*quantity)))
        .collect())
}

/// The Rhai engine with the host API registered, shared by every call,
/// whose scripts import the scripts `scripts` keeps.
pub(crate) fn host(scripts: Scripts) -> rhai::Engine {
    let mut rhai = rhai::Engine::new();
    rhai.set_module_resolver(Imports(scripts));
    rhai.set_max_operations(MAX_OPERATIONS);
    // Rhai's defaults reject the nesting an ordinary policy reaches.
    rhai.set_max_expr_depths(MAX_DEPTH, MAX_DEPTH / 2);
    rhai.set_max_call_levels(MAX_CALL_LEVELS);
    rhai.on_print(|_| {});
    rhai.on_debug(|_, _, _| {});
    quantities(&mut rhai);
    states(&mut rhai);
    exchanges(&mut rhai);
    crate::view::register(&mut rhai);
    let number =
        |drawn: u64| i64::try_from(drawn).map_err(|_| Failure::from("a number out of range"));
    rhai.register_type_with_name::<Random>("Random")
        .register_fn("next", move |random: &mut Random, bound: i64| {
            number(random.below(bound_of(bound)?))
        })
        .register_fn(
            "next",
            move |random: &mut Random, event: &str, bound: i64| {
                number(random.below_for(event, bound_of(bound)?)?)
            },
        )
        .register_fn(
            "keyed",
            |random: &mut Random, key: i64, bound: i64| -> Result<i64, Failure> {
                let key = u64::try_from(key).map_err(|_| {
                    Failure::from(format!("{key} is not a key: keys are not negative"))
                })?;
                i64::try_from(random.keyed(key, bound_of(bound)?))
                    .map_err(|_| "a number out of range".into())
            },
        )
        .register_fn("sample", |random: &mut Random, weights: Array| {
            random.sample(weights)
        })
        .register_fn(
            "sample",
            |random: &mut Random, event: &str, weights: Array| random.sample_for(event, weights),
        );
    rhai.register_fn("distances", distances);
    rhai
}

/// A kept script, compiled, and the scripts it imports.
pub(crate) struct Compiled {
    pub(crate) ast: rhai::AST,
    /// The scripts its `import` statements name, each once, in the order
    /// they first appear.
    pub(crate) imports: Vec<ScriptPath>,
}

/// The scripts a store keeps, each compiled once, its imports checked, and
/// evaluated once as a module when a script imports it. Clones share both.
///
/// Each is cached under the generation it was read in, and every change to
/// the scripts the store keeps starts a new one once it is committed, so a
/// script or module read from the store before a change, and cached after
/// it, is never used again.
#[derive(Clone)]
pub(crate) struct Scripts {
    store: Store,
    generation: Arc<AtomicU64>,
    compiled: Cache<(u64, ScriptPath), Compiled>,
    modules: Cache<(u64, ScriptPath), Module>,
}

/// What checking a script's imports has visited so far.
#[derive(Default)]
struct Walk {
    /// The scripts from the one checked to the one being visited.
    trail: Vec<ScriptPath>,
    /// The scripts whose imports were all visited.
    finished: BTreeSet<ScriptPath>,
    /// Each function a visited script defines, by its name and number of
    /// parameters, with that script.
    functions: BTreeMap<(ScriptFunction, usize), ScriptPath>,
}

impl Scripts {
    pub(crate) fn new(store: Store) -> Self {
        Self {
            store,
            generation: Arc::default(),
            compiled: Cache::new(COMPILED),
            modules: Cache::new(COMPILED),
        }
    }

    /// The generation of scripts a read begins in: read before what it
    /// reads from the store.
    fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    /// The source of the script at `path`.
    pub(crate) fn source(&self, path: &ScriptPath) -> Result<Option<String>, EngineError> {
        let bytes = self.store.read()?.definition(&definition(path))?;
        bytes
            .map(|bytes| {
                String::from_utf8(bytes).map_err(|_| {
                    EngineError::Store(StoreError::Malformed("a script that is not text"))
                })
            })
            .transpose()
    }

    /// `source` compiled by `rhai` as the script at `path`, refusing it
    /// unless its imports check out: each names a script by a literal path,
    /// the store keeps every script they reach, none of those imports itself
    /// through others, and no two of them, nor the script and one of them,
    /// define a function of the same name and number of parameters. Rhai
    /// resolves a function by its name and number of parameters once per
    /// call, for the script called and every script it imports alike, so two
    /// such functions could be called one for the other.
    pub(crate) fn checked(
        &self,
        rhai: &rhai::Engine,
        path: &ScriptPath,
        source: &str,
    ) -> Result<Compiled, EngineError> {
        let compiled = self.compile(rhai, path, source)?;
        self.visit(rhai, path, &compiled, &mut Walk::default())?;
        Ok(compiled)
    }

    /// The script at `path`, compiled by `rhai` and its imports checked.
    pub(crate) fn compiled(
        &self,
        rhai: &rhai::Engine,
        path: &ScriptPath,
    ) -> Result<Arc<Compiled>, EngineError> {
        let key = (self.generation(), path.clone());
        if let Some(compiled) = self.compiled.get(&key) {
            return Ok(compiled);
        }
        let source = self
            .source(path)?
            .ok_or_else(|| EngineError::UnknownScript(path.clone()))?;
        let compiled = self.checked(rhai, path, &source)?;
        Ok(self.compiled.insert(key, compiled))
    }

    /// Remembers `compiled` as the script the store now keeps at `path`.
    /// Every other script compiled and module evaluated is forgotten: any of
    /// them may import the script replaced, and is checked again with the
    /// new one when next called.
    pub(crate) fn kept(&self, path: &ScriptPath, compiled: Compiled) {
        let generation = self.forget();
        self.compiled.insert((generation, path.clone()), compiled);
    }

    /// Forgets every script compiled and module evaluated, once the scripts
    /// the store keeps have changed, such as when a package's version
    /// replaced another's: each is read again when next called. Gives the
    /// generation that begins.
    pub(crate) fn forget(&self) -> u64 {
        let generation = self.generation.fetch_add(1, Ordering::AcqRel) + 1;
        self.compiled.clear();
        self.modules.clear();
        generation
    }

    /// `source` compiled by `rhai` as the script at `path`, refusing an
    /// import that does not name its script by a literal path.
    fn compile(
        &self,
        rhai: &rhai::Engine,
        path: &ScriptPath,
        source: &str,
    ) -> Result<Compiled, EngineError> {
        let failed = |message| EngineError::Script {
            path: path.clone(),
            message,
        };
        let ast = rhai
            .compile(source)
            .map_err(|error| failed(error.to_string()))?;
        let imports = imports(&ast).map_err(failed)?;
        Ok(Compiled { ast, imports })
    }

    /// Visits `compiled`, the script at `script`, and then the scripts it
    /// imports, depth first.
    fn visit(
        &self,
        rhai: &rhai::Engine,
        script: &ScriptPath,
        compiled: &Compiled,
        walk: &mut Walk,
    ) -> Result<(), EngineError> {
        // Closures, which Rhai names after what they are, are left out.
        let functions = compiled.ast.iter_functions().filter_map(|function| {
            let name = function.name.parse::<ScriptFunction>().ok()?;
            Some((name, function.params.len()))
        });
        for (function, parameters) in functions {
            match walk.functions.entry((function, parameters)) {
                Entry::Vacant(vacant) => {
                    vacant.insert(script.clone());
                }
                Entry::Occupied(occupied) => {
                    let ((function, parameters), first) = occupied.remove_entry();
                    return Err(EngineError::SameFunction {
                        function,
                        parameters,
                        first,
                        second: script.clone(),
                    });
                }
            }
        }
        walk.trail.push(script.clone());
        for import in &compiled.imports {
            if let Some(start) = walk.trail.iter().position(|on| on == import) {
                let mut cycle = walk.trail.split_off(start);
                cycle.push(import.clone());
                return Err(EngineError::ImportCycle(cycle));
            }
            if walk.finished.contains(import) {
                continue;
            }
            match self.compiled.get(&(self.generation(), import.clone())) {
                Some(cached) => self.visit(rhai, import, &cached, walk)?,
                None => {
                    let source =
                        self.source(import)?
                            .ok_or_else(|| EngineError::UnknownImport {
                                script: script.clone(),
                                import: import.clone(),
                            })?;
                    let compiled = self.compile(rhai, import, &source)?;
                    self.visit(rhai, import, &compiled, walk)?;
                }
            }
        }
        walk.trail.pop();
        walk.finished.insert(script.clone());
        Ok(())
    }

    /// The script at `path` evaluated by `rhai` as a module.
    fn module(&self, rhai: &rhai::Engine, path: &ScriptPath) -> Result<Arc<Module>, EngineError> {
        let key = (self.generation(), path.clone());
        if let Some(module) = self.modules.get(&key) {
            return Ok(module);
        }
        let compiled = self.compiled(rhai, path)?;
        let module =
            Module::eval_ast_as_new(rhai::Scope::new(), &compiled.ast, rhai).map_err(|error| {
                EngineError::Script {
                    path: path.clone(),
                    message: error.to_string(),
                }
            })?;
        Ok(self.modules.insert(key, module))
    }
}

/// How the host resolves an import: as the script the store keeps at that
/// path, and nothing else, neither a file nor anything on a network.
struct Imports(Scripts);

impl rhai::ModuleResolver for Imports {
    fn resolve(
        &self,
        rhai: &rhai::Engine,
        _: Option<&str>,
        path: &str,
        position: Position,
    ) -> Result<Arc<Module>, Failure> {
        let script: ScriptPath = path
            .parse()
            .map_err(|_| EvalAltResult::ErrorModuleNotFound(path.to_owned(), position))?;
        self.0
            .module(rhai, &script)
            .map_err(|error| EvalAltResult::ErrorRuntime(error.to_string().into(), position).into())
    }
}

/// The scripts `ast` imports, each once, in the order they first appear. An
/// import names its script by a literal path: a package carries the scripts
/// its scripts import, so each must be known before any of them runs.
fn imports(ast: &rhai::AST) -> Result<Vec<ScriptPath>, String> {
    let mut imports: Vec<ScriptPath> = Vec::new();
    let mut refused = None;
    ast.walk(&mut |nodes: &[ASTNode]| {
        let Some(ASTNode::Stmt(Stmt::Import(import, position))) = nodes.last() else {
            return true;
        };
        let named = match &import.0 {
            Expr::StringConstant(text, _) => text.parse::<ScriptPath>().map_err(|_| {
                format!(
                    "`{text}` is not a {} ({position})",
                    <kind::Script as PathKind>::SCHEMA
                )
            }),
            _ => Err(format!(
                "an import names its script by a literal path ({position})"
            )),
        };
        match named {
            Ok(path) => {
                if !imports.contains(&path) {
                    imports.push(path);
                }
                true
            }
            Err(message) => {
                refused = Some(message);
                false
            }
        }
    });
    refused.map_or(Ok(imports), Err)
}

fn quantity_of(value: i64) -> Result<Quantity, Failure> {
    u64::try_from(value)
        .map(Quantity::new)
        .map_err(|_| format!("{value} is negative, and quantities are not").into())
}

fn checked(value: Option<Quantity>, what: &str) -> Result<Quantity, Failure> {
    value.ok_or_else(|| format!("{what} is out of range").into())
}

fn rounding(name: &str) -> Result<Rounding, Failure> {
    match name {
        "floor" => Ok(Rounding::Floor),
        "ceil" => Ok(Rounding::Ceil),
        other => Err(format!("rounding `{other}` is neither `floor` nor `ceil`").into()),
    }
}

fn quantities(rhai: &mut rhai::Engine) {
    rhai.register_type_with_name::<Quantity>("Quantity")
        .register_fn("quantity", quantity_of)
        .register_fn("quantity", |text: &str| -> Result<Quantity, Failure> {
            text.parse()
                .map_err(|_| format!("`{text}` is not a quantity").into())
        })
        .register_fn("to_string", |q: &mut Quantity| q.to_string())
        .register_fn("to_debug", |q: &mut Quantity| q.to_string())
        .register_fn("is_zero", |q: &mut Quantity| q.is_zero())
        .register_fn("to_int", |q: &mut Quantity| -> Result<i64, Failure> {
            i64::try_from(u64::try_from(*q).map_err(|_| Failure::from("too large"))?)
                .map_err(|_| format!("{q} does not fit an integer").into())
        })
        .register_fn(
            "mul_div",
            |a: Quantity, b: Quantity, c: Quantity, round: &str| {
                checked(a.checked_mul_div(b, c, rounding(round)?), "a mul_div")
            },
        )
        .register_fn("min", |a: Quantity, b: Quantity| a.min(b))
        .register_fn("max", |a: Quantity, b: Quantity| a.max(b));
    macro_rules! arithmetic {
        ($($operator:literal => $method:ident),*) => {$(
            rhai.register_fn($operator, |a: Quantity, b: Quantity| {
                checked(a.$method(b), concat!("a quantity ", $operator))
            });
            rhai.register_fn($operator, |a: Quantity, b: i64| {
                checked(a.$method(quantity_of(b)?), concat!("a quantity ", $operator))
            });
            rhai.register_fn($operator, |a: i64, b: Quantity| {
                checked(quantity_of(a)?.$method(b), concat!("a quantity ", $operator))
            });
        )*};
    }
    arithmetic!("+" => checked_add, "-" => checked_sub, "*" => checked_mul);
    macro_rules! comparisons {
        ($($operator:literal => $compare:expr),*) => {$(
            rhai.register_fn($operator, |a: Quantity, b: Quantity| $compare(a, b));
            rhai.register_fn($operator, |a: Quantity, b: i64| -> Result<bool, Failure> {
                Ok($compare(a, quantity_of(b)?))
            });
            rhai.register_fn($operator, |a: i64, b: Quantity| -> Result<bool, Failure> {
                Ok($compare(quantity_of(a)?, b))
            });
        )*};
    }
    comparisons!(
        "==" => |a: Quantity, b: Quantity| a == b,
        "!=" => |a: Quantity, b: Quantity| a != b,
        "<" => |a: Quantity, b: Quantity| a < b,
        "<=" => |a: Quantity, b: Quantity| a <= b,
        ">" => |a: Quantity, b: Quantity| a > b,
        ">=" => |a: Quantity, b: Quantity| a >= b
    );
}

fn parsed<K: PathKind>(text: &str) -> Result<Path<K>, Failure> {
    text.parse()
        .map_err(|_| format!("`{text}` is not a {}", K::SCHEMA).into())
}

fn states(rhai: &mut rhai::Engine) {
    rhai.register_type_with_name::<ScriptState>("State")
        .register_fn(
            "balance",
            |state: &mut ScriptState, account: &str, asset: &str| -> Result<Quantity, Failure> {
                let account: AccountPath = parsed(account)?;
                let asset: AssetPath = parsed(asset)?;
                Ok(if state.sees(&account) {
                    state.economy.balance(&account, &asset)
                } else {
                    Quantity::ZERO
                })
            },
        )
        .register_fn(
            "has_account",
            |state: &mut ScriptState, account: &str| -> Result<bool, Failure> {
                let account: AccountPath = parsed(account)?;
                Ok(state.sees(&account) && state.economy.account(&account).is_some())
            },
        )
        .register_fn(
            "accounts",
            |state: &mut ScriptState, prefix: &str| -> Array {
                state
                    .economy
                    .accounts()
                    .map(|(account, _)| account)
                    .filter(|account| state.sees(account) && account.as_str().starts_with(prefix))
                    .map(|account| Dynamic::from(account.to_string()))
                    .collect()
            },
        )
        .register_fn(
            "balances",
            |state: &mut ScriptState, account: &str| -> Result<Map, Failure> {
                let account: AccountPath = parsed(account)?;
                let Some(holder) = state
                    .economy
                    .account(&account)
                    .filter(|_| state.sees(&account))
                else {
                    return Ok(Map::new());
                };
                Ok(holder
                    .balances()
                    .iter()
                    .map(|(asset, quantity)| (asset.as_str().into(), Dynamic::from(*quantity)))
                    .collect())
            },
        )
        .register_fn("rates", |state: &mut ScriptState, prefix: &str| -> Array {
            state
                .economy
                .rate_ids()
                .filter(|rate| rate.as_str().starts_with(prefix))
                .map(|rate| Dynamic::from(rate.to_string()))
                .collect()
        })
        .register_fn(
            "consumed",
            |state: &mut ScriptState, rate: &str, role: &str| -> Result<Map, Failure> {
                effect(state, rate, role, |rate, role| rate.consumed(role))
            },
        )
        .register_fn(
            "produced",
            |state: &mut ScriptState, rate: &str, role: &str| -> Result<Map, Failure> {
                effect(state, rate, role, |rate, role| rate.produced(role))
            },
        )
        .register_fn(
            "preserved",
            |state: &mut ScriptState, rate: &str, role: &str| -> Result<Map, Failure> {
                effect(state, rate, role, |rate, role| rate.preserved(role))
            },
        )
        .register_fn(
            "roles",
            |state: &mut ScriptState, rate: &str| -> Result<Array, Failure> {
                let rate: RatePath = parsed(rate)?;
                let definition = state
                    .economy
                    .rate(&rate)
                    .ok_or_else(|| Failure::from(format!("the economy has no rate `{rate}`")))?;
                Ok(definition
                    .roles()
                    .map(|role| Dynamic::from(role.to_string()))
                    .collect())
            },
        )
        .register_fn(
            "view",
            |state: &mut ScriptState, accounts: Array| -> Result<ScriptState, Failure> {
                let visible = accounts
                    .into_iter()
                    .map(|account| parsed(&text(account)?))
                    .collect::<Result<BTreeSet<_>, _>>()?;
                Ok(ScriptState {
                    economy: state.economy.clone(),
                    visible: Some(Arc::new(visible)),
                })
            },
        )
        .register_fn(
            "applicable",
            |state: &mut ScriptState, exchange: paths::Exchange| -> Result<bool, Failure> {
                Ok(state
                    .whole("whether an exchange applies")?
                    .is_applicable(&exchange))
            },
        )
        .register_fn(
            "assess",
            |state: &mut ScriptState, exchange: paths::Exchange| -> Result<Map, Failure> {
                let assessment = state.whole("an assessment")?.assess(&exchange);
                let status = match assessment.status() {
                    AssessmentStatus::Applicable => "applicable",
                    AssessmentStatus::Infeasible => "infeasible",
                    AssessmentStatus::Invalid => "invalid",
                };
                let missing: Array = assessment
                    .shortfalls()
                    .iter()
                    .flat_map(|shortfall| {
                        shortfall.missing().iter().map(move |(asset, quantity)| {
                            let mut line = Map::new();
                            line.insert("account".into(), shortfall.account().to_string().into());
                            line.insert("asset".into(), asset.to_string().into());
                            line.insert("quantity".into(), Dynamic::from(*quantity));
                            Dynamic::from(line)
                        })
                    })
                    .collect();
                let mut result = Map::new();
                result.insert("status".into(), status.into());
                result.insert("missing".into(), missing.into());
                Ok(result)
            },
        );
}

fn text(value: Dynamic) -> Result<String, Failure> {
    value
        .into_immutable_string()
        .map(|text| text.to_string())
        .map_err(|kind| format!("expected a path, found {kind}").into())
}

fn exchanges(rhai: &mut rhai::Engine) {
    fn exchange(rate: &str, bindings: Map, units: Quantity) -> Result<paths::Exchange, Failure> {
        let mut exchange = paths::Exchange::new(parsed(rate)?, units);
        for (role, account) in bindings {
            let role: RolePath = parsed(role.as_str())?;
            exchange = exchange.bind(role, parsed(&text(account)?)?);
        }
        Ok(exchange)
    }
    rhai.register_type_with_name::<paths::Exchange>("Exchange")
        .register_fn("exchange", exchange)
        .register_fn("exchange", |rate: &str, bindings: Map, units: i64| {
            exchange(rate, bindings, quantity_of(units)?)
        })
        .register_fn(
            "parameter",
            |exchange: paths::Exchange, name: &str, value: Quantity| -> Result<_, Failure> {
                Ok(exchange.parameter(parsed(name)?, value))
            },
        )
        .register_fn("rate", |exchange: &mut paths::Exchange| {
            ImmutableString::from(exchange.rate().as_str())
        })
        .register_fn("units", |exchange: &mut paths::Exchange| exchange.units())
        .register_fn(
            "account",
            |exchange: &mut paths::Exchange, role: &str| -> Result<Dynamic, Failure> {
                let role: RolePath = parsed(role)?;
                Ok(exchange
                    .bindings()
                    .get(&role)
                    .map_or(Dynamic::UNIT, |account| account.to_string().into()))
            },
        )
        .register_fn("to_string", |exchange: &mut paths::Exchange| {
            exchange.rate().to_string()
        })
        .register_fn("==", |a: paths::Exchange, b: paths::Exchange| a == b)
        .register_fn("!=", |a: paths::Exchange, b: paths::Exchange| a != b);
}

/// The fewest edges from `from` to every node it reaches, over undirected
/// `edges` given as pairs of names.
fn distances(edges: Array, from: &str) -> Result<Map, Failure> {
    let mut neighbours: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for edge in edges {
        let pair: Array = edge
            .try_cast()
            .ok_or_else(|| Failure::from("an edge is not a pair"))?;
        let [a, b]: [Dynamic; 2] = pair
            .try_into()
            .map_err(|_| Failure::from("an edge is not a pair"))?;
        let (a, b) = (text(a)?, text(b)?);
        neighbours.entry(a.clone()).or_default().push(b.clone());
        neighbours.entry(b).or_default().push(a);
    }
    let mut reached = Map::new();
    reached.insert(from.into(), Dynamic::from(0_i64));
    let mut pending = VecDeque::from([(from.to_owned(), 0_i64)]);
    while let Some((at, distance)) = pending.pop_front() {
        for next in neighbours.get(&at).into_iter().flatten() {
            if !reached.contains_key(next.as_str()) {
                reached.insert(next.as_str().into(), Dynamic::from(distance + 1));
                pending.push_back((next.clone(), distance + 1));
            }
        }
    }
    Ok(reached)
}

/// The weighted outcomes in what a script returned: `[exchange, weight]`
/// pairs, each weight an integer or a quantity.
pub(crate) fn weighted_of(
    returned: Array,
) -> Result<Vec<WeightedExchange<paths::Exchange>>, EngineError> {
    returned
        .into_iter()
        .map(|value| {
            let found = value.type_name();
            let returned = |expected| EngineError::Returned {
                expected,
                found: found.to_owned(),
            };
            let [exchange, weight]: [Dynamic; 2] = value
                .try_cast::<Array>()
                .and_then(|pair| pair.try_into().ok())
                .ok_or_else(|| returned("an [exchange, weight] pair"))?;
            let exchange = exchange
                .try_cast::<paths::Exchange>()
                .ok_or_else(|| returned("an exchange and its weight"))?;
            let weight = weight_of(weight).map_err(|_| returned("an exchange and its weight"))?;
            Ok(WeightedExchange::new(exchange, weight))
        })
        .collect()
}

/// The exchanges in what a script returned.
pub fn exchanges_of(returned: Array) -> Result<Vec<paths::Exchange>, EngineError> {
    returned
        .into_iter()
        .map(|value| {
            let kind = value.type_name();
            value
                .try_cast::<paths::Exchange>()
                .ok_or_else(|| EngineError::Returned {
                    expected: "an exchange",
                    found: kind.to_owned(),
                })
        })
        .collect()
}
