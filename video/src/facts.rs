//! What the video shows, read from the engine when the video is made: the
//! samples' packages written into a store in memory, their refs reached and
//! the ontologies their runs are of,
//! their experiments run, forks of a start played on, and the routes a DQN
//! learned (`data/learned`) replayed. No number on screen is typed in by
//! hand.

use axionomy::{AccountPath, Quantity, RatePath, path, paths};
use axionomy_engine::{
    Chooser, Ended, Engine, ExperimentPath, ObjectivePath, Results, Strategy, offered,
};
use axionomy_problems::packages;
use axionomy_rl::{Reward, Task};
use axionomy_store::Store;
use std::collections::BTreeSet;
use std::ops::ControlFlow;

/// A balance as the video writes it: the asset's path and its quantity.
#[derive(Debug, Clone, PartialEq)]
pub struct Balance {
    pub asset: String,
    pub quantity: u64,
}

/// An account and what it holds, in the assets' order.
#[derive(Debug, Clone)]
pub struct Holding {
    pub account: String,
    pub balances: Vec<Balance>,
}

/// One role of a rate instance: the account it is bound to and what it
/// consumes, preserves and produces there.
#[derive(Debug, Clone)]
pub struct Role {
    pub role: String,
    pub account: String,
    pub consumes: Vec<Balance>,
    pub preserves: Vec<Balance>,
    pub produces: Vec<Balance>,
}

/// A rate instance as an exchange applies it.
#[derive(Debug, Clone)]
pub struct Law {
    pub rate: String,
    pub roles: Vec<Role>,
}

/// What an account is short of for an exchange.
#[derive(Debug, Clone)]
pub struct Short {
    pub account: String,
    pub asset: String,
    pub missing: u64,
}

/// What an economy names of each of its ontology's four kinds: its
/// accounts, the assets they hold or its rates name, its rates, and the
/// rates' roles. Each in order.
#[derive(Debug, Clone)]
pub struct Members {
    pub accounts: Vec<String>,
    pub assets: Vec<String>,
    pub rates: Vec<String>,
    pub roles: Vec<String>,
}

/// The key-door maze's Micro: the ontology its run is of and what its start
/// names in it, the first move's rate, the move applied, and an exchange the
/// kernel refuses where the move leads.
#[derive(Debug, Clone)]
pub struct Ontology {
    pub point: String,
    pub name: String,
    pub members: Members,
    pub before: Vec<Holding>,
    pub rate: Law,
    pub after: Vec<Holding>,
    pub refused: Law,
    pub shortfalls: Vec<Short>,
}

/// The pool's reserves after each exchange that changed them.
#[derive(Debug, Clone)]
pub struct Reserves {
    pub run: String,
    pub pool: String,
    /// `(step, energy, credit)`, in order.
    pub steps: Vec<(usize, f64, f64)>,
    pub exchanges: usize,
}

/// Where a strategy's route ended on the maze Showcase's objectives, and
/// how many points it forked on the way: the states a search generated.
#[derive(Debug, Clone)]
pub struct Outcome {
    pub by: String,
    pub energy: u64,
    pub time: u64,
    pub exchanges: usize,
    pub forked: usize,
}

/// The forks of the maze Showcase's start, a few exchanges deep, and the
/// plan A* committed.
#[derive(Debug, Clone)]
pub struct Forks {
    /// Each level's points, each with the index of its parent one level up.
    pub levels: Vec<Vec<usize>>,
    /// The plan's point at each level it is drawn at: its index there.
    pub plan: Vec<usize>,
    /// The plan's exchanges, by rate.
    pub rates: Vec<String>,
}

/// What a DQN learned weighing one objective most: its route's outcome and
/// what its training episodes earned, smoothed.
#[derive(Debug, Clone)]
pub struct Learned {
    pub weighing: String,
    pub outcome: Outcome,
    pub reached: bool,
    pub episodes: usize,
    /// The mean reward of each window of episodes, in order.
    pub curve: Vec<f64>,
}

/// One exchange a start offers, applied on a fork of it, and where a
/// strategy played on from that fork ends.
#[derive(Debug, Clone)]
pub struct Fork {
    pub rate: String,
    pub energy: u64,
    pub time: u64,
}

/// The maze Showcase's start, each exchange it offers on a fork of its own,
/// and whether the start's ref still names the same point afterwards.
#[derive(Debug, Clone)]
pub struct Counterfactual {
    pub start: String,
    pub strategy: String,
    pub forks: Vec<Fork>,
    pub unchanged: bool,
}

/// How a trial ended, as the video colors it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ending {
    Reached,
    /// The policy proposed an exchange the kernel refused.
    Refused,
    Other,
}

/// An experiment of many seeded trials: each arm's endings, in seed order.
#[derive(Debug, Clone)]
pub struct Rollouts {
    pub experiment: String,
    pub arms: Vec<(String, Vec<Ending>)>,
}

/// A kind of strategy the engine runs, and the samples that run it.
#[derive(Debug, Clone)]
pub struct Kind {
    pub name: &'static str,
    pub samples: BTreeSet<String>,
}

/// The kinds of strategy the samples' experiments run: searches over a
/// candidate set, choosers in games, and scripts.
#[derive(Debug, Clone, Default)]
pub struct Kinds {
    pub searches: Vec<Kind>,
    pub choosers: Vec<Kind>,
    pub scripts: Vec<Kind>,
}

/// An experiment as a learner meets it through `axionomy-rl`.
#[derive(Debug, Clone)]
pub struct Environment {
    pub experiment: String,
    pub actions: usize,
    pub observed: usize,
}

/// A recorded run's end, and the same run replayed from its start.
#[derive(Debug, Clone)]
pub struct Replay {
    pub run: String,
    pub exchanges: usize,
    pub recorded: u64,
    pub replayed: u64,
}

/// A theorem of the specification, by its file and name.
#[derive(Debug, Clone)]
pub struct Theorem {
    pub file: &'static str,
    pub name: &'static str,
}

/// The theorems the video names, each checked to be in its file.
fn theorems() -> Vec<Theorem> {
    let files = [
        (
            "Exchange.lean",
            include_str!("../../verification/lean/Axionomy/Exchange.lean"),
        ),
        (
            "Replay.lean",
            include_str!("../../verification/lean/Axionomy/Replay.lean"),
        ),
        (
            "MulDiv.lean",
            include_str!("../../verification/lean/Axionomy/MulDiv.lean"),
        ),
    ];
    [
        ("Exchange.lean", "accepted_exchange_conserves"),
        ("Exchange.lean", "rejection_is_atomic"),
        ("Replay.lean", "deterministic"),
        ("MulDiv.lean", "floor_bounds"),
    ]
    .into_iter()
    .map(|(file, name)| {
        let (_, source) = files
            .iter()
            .find(|(named, _)| *named == file)
            .expect("a file");
        assert!(
            source
                .lines()
                .any(|line| line.starts_with(&format!("theorem {name} "))),
            "{file} proves no `{name}`"
        );
        Theorem { file, name }
    })
    .collect()
}

/// The theorems `verification/lean/Axionomy` proves, counted from its files.
fn proved() -> usize {
    let dir =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../verification/lean/Axionomy");
    std::fs::read_dir(&dir)
        .expect("the specification's directory")
        .map(|entry| entry.expect("an entry").path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "lean")
        })
        .map(|path| {
            std::fs::read_to_string(&path)
                .expect("a Lean file")
                .lines()
                .filter(|line| line.starts_with("theorem "))
                .count()
        })
        .sum()
}

#[derive(Debug, Clone)]
pub struct Facts {
    /// The sample packages, by name.
    pub samples: Vec<String>,
    /// The ontologies the samples' refs are of: each package's, in order.
    pub ontologies: Vec<String>,
    /// How many theorems the specification proves.
    pub proved: usize,
    pub ontology: Ontology,
    pub reserves: Reserves,
    pub searched: Vec<Outcome>,
    pub counterfactual: Counterfactual,
    pub rollouts: Rollouts,
    pub forks: Forks,
    pub kinds: Kinds,
    pub environment: Environment,
    pub learned: Vec<Learned>,
    pub replays: Vec<Replay>,
    pub theorems: Vec<Theorem>,
}

/// The routes and rewards the `maze` example of `crates/axionomy-burn`
/// wrote: `(weighing, route, rewards)`.
const LEARNED: [(&str, &str, &str); 2] = [
    (
        "energy",
        include_str!("../data/learned/learned-energy.txt"),
        include_str!("../data/learned/rewards-energy.txt"),
    ),
    (
        "time",
        include_str!("../data/learned/learned-time.txt"),
        include_str!("../data/learned/rewards-time.txt"),
    ),
];

fn count(quantity: &Quantity) -> u64 {
    u64::try_from(*quantity).expect("a count the video can write")
}

fn balances<'a>(
    basket: impl IntoIterator<Item = (&'a axionomy::Pattern<axionomy::AssetPath>, &'a Quantity)>,
) -> Vec<Balance> {
    basket
        .into_iter()
        .filter_map(|(asset, quantity)| {
            Some(Balance {
                asset: asset.as_literal()?.to_string(),
                quantity: count(quantity),
            })
        })
        .collect()
}

fn holding(economy: &paths::Economy, account: &AccountPath) -> Holding {
    let held = economy.account(account).expect("an account of the economy");
    Holding {
        account: account.to_string(),
        balances: held
            .balances()
            .iter_sorted()
            .map(|(asset, quantity)| Balance {
                asset: asset.to_string(),
                quantity: count(quantity),
            })
            .collect(),
    }
}

fn law(economy: &paths::Economy, exchange: &paths::Exchange) -> Law {
    let instance = economy.instance(exchange.rate()).expect("an instance");
    Law {
        rate: exchange.rate().to_string(),
        roles: instance
            .roles()
            .map(|role| Role {
                role: role.to_string(),
                account: exchange
                    .bindings()
                    .get(role)
                    .map_or_else(String::new, ToString::to_string),
                consumes: instance
                    .consumed(role)
                    .map_or_else(Vec::new, |b| balances(b.iter())),
                preserves: instance
                    .preserved(role)
                    .map_or_else(Vec::new, |b| balances(b.iter())),
                produces: instance
                    .produced(role)
                    .map_or_else(Vec::new, |b| balances(b.iter())),
            })
            .collect(),
    }
}

fn exchange(economy: &paths::Economy, rate: &str) -> paths::Exchange {
    let rate: RatePath = rate.parse().expect("a rate path");
    economy
        .exchange_of(&rate)
        .expect("an instance of the economy")
}

/// What `economy` names of each kind.
fn members(economy: &paths::Economy) -> Members {
    let mut assets = BTreeSet::new();
    let mut roles = BTreeSet::new();
    for (_, account) in economy.accounts() {
        for (asset, _) in account.balances().iter_sorted() {
            assets.insert(asset.to_string());
        }
    }
    for (_, rate) in economy.rates() {
        for role in rate.roles() {
            roles.insert(role.to_string());
            for basket in [
                rate.consumed(role),
                rate.preserved(role),
                rate.produced(role),
            ]
            .into_iter()
            .flatten()
            {
                assets.extend(balances(basket.iter()).into_iter().map(|b| b.asset));
            }
        }
    }
    Members {
        accounts: economy.accounts().map(|(id, _)| id.to_string()).collect(),
        assets: assets.into_iter().collect(),
        rates: economy.rate_ids().map(ToString::to_string).collect(),
        roles: roles.into_iter().collect(),
    }
}

fn ontology(engine: &Engine) -> Ontology {
    let start = engine.referred(&path!("maze/micro")).expect("maze/micro");
    let economy = engine.reach(start).expect("reached").state.economy.clone();
    let accounts: Vec<AccountPath> = economy.accounts().map(|(id, _)| id.clone()).collect();
    let first = exchange(&economy, "move/start/key-room/2/false");
    let rate = law(&economy, &first);
    let mut moved = economy.clone();
    moved.apply(first).expect("the first move applies");
    let unlock = exchange(&moved, "unlock-door/key-room");
    let assessment = moved.assess(&unlock);
    let shortfalls = assessment
        .shortfalls()
        .iter()
        .flat_map(|short| {
            short
                .missing()
                .iter_sorted()
                .map(|(asset, quantity)| Short {
                    account: short.account().to_string(),
                    asset: asset.to_string(),
                    missing: count(quantity),
                })
        })
        .collect();
    Ontology {
        point: "maze/micro".into(),
        name: engine
            .ontology(start)
            .expect("the run's ontology")
            .to_string(),
        members: members(&economy),
        before: accounts.iter().map(|id| holding(&economy, id)).collect(),
        rate,
        after: accounts.iter().map(|id| holding(&moved, id)).collect(),
        refused: law(&moved, &unlock),
        shortfalls,
    }
}

fn reserves(engine: &Engine) -> Reserves {
    let experiment: ExperimentPath = path!("amm/showcase");
    let results = engine
        .run_experiment(&experiment, &mut |_, _| ControlFlow::Continue(()))
        .expect("the AMM's showcase runs");
    let arm = &results.arms[0];
    let (mut economy, exchanges) = engine.history(arm.trials[0].end).expect("its history");
    let pool: AccountPath = path!("pool");
    let reserve = |economy: &paths::Economy| {
        let held = |asset: &str| {
            let asset = asset.parse().expect("an asset path");
            f64::from(economy.balance(&pool, &asset).value())
        };
        (held("energy"), held("credit"))
    };
    let mut steps = vec![{
        let (energy, credit) = reserve(&economy);
        (0, energy, credit)
    }];
    for (index, exchange) in exchanges.iter().enumerate() {
        economy
            .apply(exchange.clone())
            .expect("a recorded exchange replays");
        let (energy, credit) = reserve(&economy);
        let last = steps.last().expect("the start");
        if (energy, credit) != (last.1, last.2) {
            steps.push((index + 1, energy, credit));
        }
    }
    Reserves {
        run: arm.arm.strategy.to_string(),
        pool: pool.to_string(),
        steps,
        exchanges: exchanges.len(),
    }
}

fn searched(engine: &Engine, maze: &Results, forked: &[usize]) -> Vec<Outcome> {
    maze.arms
        .iter()
        .zip(forked)
        .map(|(arm, forked)| {
            let trial = &arm.trials[0];
            let (_, exchanges) = engine.history(trial.end).expect("its history");
            Outcome {
                by: arm.arm.strategy.to_string(),
                energy: count(&trial.values[0]),
                time: count(&trial.values[1]),
                exchanges: exchanges.len(),
                forked: *forked,
            }
        })
        .collect()
}

/// The exchanges that apply to `economy`, of every rate.
fn applying(economy: &paths::Economy) -> Vec<paths::Exchange> {
    offered(economy, None)
        .filter(|exchange| economy.is_applicable(exchange))
        .map(std::borrow::Cow::into_owned)
        .collect()
}

fn forks(engine: &Engine, maze: &Results, depth: usize) -> Forks {
    let star = maze
        .arms
        .iter()
        .find(|arm| arm.arm.strategy.as_str() == "maze/a-star")
        .expect("an A* arm");
    let (root, plan) = engine.history(star.trials[0].end).expect("its history");
    let mut levels: Vec<Vec<usize>> = vec![vec![0]];
    let mut on_plan = vec![0];
    let mut frontier = vec![(root, true)];
    for planned in plan.iter().take(depth) {
        let mut next = Vec::new();
        let mut parents = Vec::new();
        let mut planned_at = None;
        for (parent, (economy, is_plan)) in frontier.iter().enumerate() {
            for exchange in applying(economy) {
                let follows = *is_plan && exchange == *planned;
                if follows {
                    planned_at = Some(next.len());
                }
                let mut child = economy.clone();
                child.apply(exchange).expect("an applying exchange applies");
                parents.push(parent);
                next.push((child, follows));
            }
        }
        levels.push(parents);
        on_plan.push(planned_at.expect("the plan's step is among the forks"));
        frontier = next;
    }
    Forks {
        levels,
        plan: on_plan,
        rates: plan
            .iter()
            .map(|exchange| exchange.rate().to_string())
            .collect(),
    }
}

fn learned(engine: &Engine) -> Vec<Learned> {
    let start = engine
        .referred(&path!("maze/showcase"))
        .expect("maze/showcase");
    let initial = engine.reach(start).expect("reached").state.economy.clone();
    let objectives: [ObjectivePath; 2] = [path!("maze/energy"), path!("maze/time")];
    LEARNED
        .iter()
        .map(|(weighing, route, rewards)| {
            let mut economy = initial.clone();
            let mut exchanges = 0;
            for rate in route.lines().filter(|line| !line.is_empty()) {
                let step = exchange(&economy, rate);
                economy.apply(step).expect("the learned route replays");
                exchanges += 1;
            }
            let measured = |objective: &ObjectivePath| {
                count(
                    &engine
                        .measure_economy(objective, &economy)
                        .expect("measured"),
                )
            };
            let goal = engine.goal(&path!("maze/solved")).expect("the goal");
            let rewards: Vec<f64> = rewards
                .lines()
                .filter_map(|line| line.trim().parse().ok())
                .collect();
            let window = (rewards.len() / 120).max(1);
            let curve = rewards
                .chunks(window)
                .map(|chunk| chunk.iter().sum::<f64>() / chunk.len() as f64)
                .collect();
            Learned {
                weighing: (*weighing).into(),
                outcome: Outcome {
                    by: format!("DQN, weighing {weighing}"),
                    energy: measured(&objectives[0]),
                    time: measured(&objectives[1]),
                    exchanges,
                    forked: 0,
                },
                reached: economy.matches(&goal),
                episodes: rewards.len(),
                curve,
            }
        })
        .collect()
}

fn counterfactual(engine: &Engine) -> Counterfactual {
    let experiment = engine
        .experiment(&path!("maze/showcase"))
        .expect("the maze's showcase");
    let start = engine.referred(&experiment.start).expect("its start");
    let strategy = path!("maze/dijkstra");
    let dijkstra = engine.strategy(&strategy).expect("Dijkstra's strategy");
    let forks = engine
        .candidates(start, &path!("maze/moves"))
        .expect("the moves offered")
        .into_iter()
        .map(|exchange| {
            let fork = engine
                .apply(start, std::slice::from_ref(&exchange))
                .expect("an offered move applies");
            let ran = engine
                .run_strategy(
                    fork,
                    &dijkstra,
                    &experiment.goal,
                    experiment.effort,
                    &mut |_| ControlFlow::Continue(()),
                )
                .expect("Dijkstra runs from the fork");
            let measured = |objective: &ObjectivePath| {
                count(&engine.measure(ran.end, objective).expect("measured"))
            };
            Fork {
                rate: exchange.rate().to_string(),
                energy: measured(&experiment.objectives[0]),
                time: measured(&experiment.objectives[1]),
            }
        })
        .collect();
    Counterfactual {
        start: experiment.start.to_string(),
        strategy: strategy.to_string(),
        forks,
        unchanged: engine.referred(&experiment.start).expect("its start") == start,
    }
}

fn rollouts(engine: &Engine) -> Rollouts {
    let experiment: ExperimentPath = path!("rescue/micro/monte-carlo");
    let results = engine
        .run_experiment(&experiment, &mut |_, _| ControlFlow::Continue(()))
        .expect("the rescue's Monte Carlo runs");
    Rollouts {
        experiment: experiment.to_string(),
        arms: results
            .arms
            .iter()
            .map(|arm| {
                let endings = arm
                    .trials
                    .iter()
                    .map(|trial| match trial.ended {
                        Ended::Reached => Ending::Reached,
                        Ended::Refused { .. } => Ending::Refused,
                        _ => Ending::Other,
                    })
                    .collect();
                (arm.arm.strategy.to_string(), endings)
            })
            .collect(),
    }
}

/// Adds `sample` to the kind named `name`, in the order kinds are met.
fn tally(kinds: &mut Vec<Kind>, name: &'static str, sample: &str) {
    match kinds.iter_mut().find(|kind| kind.name == name) {
        Some(kind) => {
            kind.samples.insert(sample.into());
        }
        None => kinds.push(Kind {
            name,
            samples: BTreeSet::from([sample.into()]),
        }),
    }
}

fn kinds(engine: &Engine) -> Kinds {
    let mut kinds = Kinds::default();
    let mut seen = BTreeSet::new();
    for path in engine.experiments().expect("the experiments") {
        let experiment = engine.experiment(&path).expect("an experiment");
        for arm in experiment.arms {
            if !seen.insert(arm.strategy.clone()) {
                continue;
            }
            let sample = arm.strategy.to_string();
            let sample = sample.split('/').next().expect("a first segment");
            match engine.strategy(&arm.strategy).expect("a strategy") {
                Strategy::Policy { .. } => tally(&mut kinds.scripts, "policies", sample),
                Strategy::Plan { .. } => tally(&mut kinds.scripts, "plans", sample),
                Strategy::BreadthFirst { .. } => {
                    tally(&mut kinds.searches, "breadth-first", sample);
                }
                Strategy::Cheapest { heuristic, .. } => tally(
                    &mut kinds.searches,
                    if heuristic.is_some() {
                        "A*"
                    } else {
                        "Dijkstra"
                    },
                    sample,
                ),
                Strategy::BranchAndBound { .. } => {
                    tally(&mut kinds.searches, "branch and bound", sample);
                }
                Strategy::Beam { .. } => tally(&mut kinds.searches, "beam", sample),
                Strategy::BestFirst { .. } => tally(&mut kinds.searches, "best-first", sample),
                Strategy::Pareto { .. } => tally(&mut kinds.searches, "Pareto", sample),
                Strategy::Game { players, .. } => {
                    for player in players {
                        let name = match player {
                            Chooser::Policy(_) => "scripted players",
                            Chooser::Minimax { .. } => "minimax",
                            Chooser::Mcts { .. } => "MCTS",
                            Chooser::Ismcts { .. } => "ISMCTS",
                        };
                        tally(&mut kinds.choosers, name, sample);
                    }
                }
            }
        }
    }
    const ORDER: [&str; 13] = [
        "breadth-first",
        "Dijkstra",
        "A*",
        "best-first",
        "branch and bound",
        "beam",
        "Pareto",
        "minimax",
        "MCTS",
        "ISMCTS",
        "scripted players",
        "policies",
        "plans",
    ];
    for kinds in [&mut kinds.searches, &mut kinds.choosers, &mut kinds.scripts] {
        kinds.sort_by_key(|kind| ORDER.iter().position(|name| *name == kind.name));
    }
    kinds
}

fn environment(engine: &Engine) -> Environment {
    let experiment: ExperimentPath = path!("maze/showcase");
    // What a learner chooses among and observes does not depend on how it
    // is rewarded.
    let reward = Reward {
        weights: Vec::new(),
        reached: 0.0,
    };
    let task = Task::new(engine, &experiment, &path!("maze/moves"), reward)
        .expect("the maze's showcase as a task");
    Environment {
        experiment: experiment.to_string(),
        actions: task.actions().len(),
        observed: task.observed().len(),
    }
}

/// The longest run of each experiment, recorded and replayed.
fn replays(engine: &Engine) -> Vec<Replay> {
    [
        "maze/showcase",
        "lending/showcase",
        "logistics/showcase",
        "marketplace/showcase",
        "mission/showcase",
        "amm/showcase",
    ]
    .into_iter()
    .map(|experiment| {
        let results = engine
            .run_experiment(
                &experiment.parse().expect("an experiment path"),
                &mut |_, _| ControlFlow::Continue(()),
            )
            .expect("a showcase runs");
        let (run, end, initial, exchanges) = results
            .arms
            .iter()
            .map(|arm| {
                let end = arm.trials[0].end;
                let (initial, exchanges) = engine.history(end).expect("its history");
                (arm.arm.strategy.to_string(), end, initial, exchanges)
            })
            .max_by_key(|(_, _, _, exchanges)| exchanges.len())
            .expect("an arm");
        let recorded = engine
            .reach(end)
            .expect("reached")
            .state
            .economy
            .state_fingerprint()
            .get();
        let mut economy = initial;
        for exchange in &exchanges {
            economy
                .apply(exchange.clone())
                .expect("a recorded exchange replays");
        }
        Replay {
            run,
            exchanges: exchanges.len(),
            recorded,
            replayed: economy.state_fingerprint().get(),
        }
    })
    .collect()
}

impl Facts {
    /// Every sample package written into a store in memory, and what the
    /// video shows read from it.
    pub fn read() -> Self {
        let engine = Engine::new(Store::memory().expect("a store")).expect("an engine");
        let mut samples = Vec::new();
        let mut ontologies = Vec::new();
        for (name, write) in packages() {
            write(&engine).expect("a sample package writes");
            let mut of = BTreeSet::new();
            for reference in engine.named_under(&name).expect("its refs").refs {
                let point = engine.referred(&reference).expect("a ref's point");
                of.insert(
                    engine
                        .ontology(point)
                        .expect("its run's ontology")
                        .to_string(),
                );
            }
            ontologies.extend(of);
            samples.push(name.to_string());
        }
        // Each arm's search reports the points it has generated as it goes.
        let mut forked = Vec::new();
        let maze = engine
            .run_experiment(&path!("maze/showcase"), &mut |playing, progress| {
                forked.resize(forked.len().max(playing.arm + 1), 0);
                forked[playing.arm] = progress.generated();
                ControlFlow::Continue(())
            })
            .expect("the maze's showcase runs");
        Self {
            samples,
            ontologies,
            proved: proved(),
            ontology: ontology(&engine),
            reserves: reserves(&engine),
            searched: searched(&engine, &maze, &forked),
            counterfactual: counterfactual(&engine),
            rollouts: rollouts(&engine),
            forks: forks(&engine, &maze, 3),
            kinds: kinds(&engine),
            environment: environment(&engine),
            learned: learned(&engine),
            replays: replays(&engine),
            theorems: theorems(),
        }
    }
}
