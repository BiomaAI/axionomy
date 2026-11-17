//! What the video shows, read from the engine when the video is made: the
//! samples' packages written into a store in memory, their refs reached,
//! their experiments run, and the routes a DQN learned (`data/learned`)
//! replayed. No number on screen is typed in by hand.

use axionomy::{AccountPath, Quantity, RatePath, path, paths};
use axionomy_engine::{Engine, ExperimentPath, ObjectivePath, Results, offered};
use axionomy_problems::packages;
use axionomy_store::Store;
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

/// The key-door maze's Micro: its start, the first move's rate, the move
/// applied, and an exchange the kernel refuses where the move leads.
#[derive(Debug, Clone)]
pub struct Model {
    pub point: String,
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

/// Where a strategy's route ended on the maze Showcase's objectives.
#[derive(Debug, Clone)]
pub struct Outcome {
    pub by: String,
    pub energy: u64,
    pub time: u64,
    pub exchanges: usize,
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

/// One sample package as its Showcase start holds it.
#[derive(Debug, Clone)]
pub struct Package {
    pub name: String,
    pub accounts: usize,
    pub rates: usize,
    pub instances: usize,
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

#[derive(Debug, Clone)]
pub struct Facts {
    pub model: Model,
    pub reserves: Reserves,
    pub searched: Vec<Outcome>,
    pub forks: Forks,
    pub learned: Vec<Learned>,
    pub packages: Vec<Package>,
    pub replay: Replay,
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

fn model(engine: &Engine) -> Model {
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
    Model {
        point: "maze/micro".into(),
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

fn searched(engine: &Engine, maze: &Results) -> Vec<Outcome> {
    maze.arms
        .iter()
        .map(|arm| {
            let trial = &arm.trials[0];
            let (_, exchanges) = engine.history(trial.end).expect("its history");
            Outcome {
                by: arm.arm.strategy.to_string(),
                energy: count(&trial.values[0]),
                time: count(&trial.values[1]),
                exchanges: exchanges.len(),
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
                },
                reached: economy.matches(&goal),
                episodes: rewards.len(),
                curve,
            }
        })
        .collect()
}

fn packages_shown(engine: &Engine) -> Vec<Package> {
    [
        "connect-four",
        "work-league",
        "logistics",
        "perishables",
        "amm",
        "lending",
    ]
    .into_iter()
    .map(|name| {
        let start = engine
            .referred(&format!("{name}/showcase").parse().expect("a ref path"))
            .expect("a showcase ref");
        let economy = engine.reach(start).expect("reached").state.economy.clone();
        Package {
            name: name.into(),
            accounts: economy.accounts().count(),
            rates: economy.rate_ids().count(),
            instances: economy
                .rate_ids()
                .map(|key| economy.instances_of(key).count())
                .sum(),
        }
    })
    .collect()
}

fn replay(engine: &Engine, maze: &Results) -> Replay {
    let arm = maze
        .arms
        .iter()
        .find(|arm| arm.arm.strategy.as_str() == "maze/dijkstra")
        .expect("a Dijkstra arm");
    let end = arm.trials[0].end;
    let recorded = engine
        .reach(end)
        .expect("reached")
        .state
        .economy
        .state_fingerprint()
        .get();
    let (mut economy, exchanges) = engine.history(end).expect("its history");
    for exchange in &exchanges {
        economy
            .apply(exchange.clone())
            .expect("a recorded exchange replays");
    }
    Replay {
        run: arm.arm.strategy.to_string(),
        exchanges: exchanges.len(),
        recorded,
        replayed: economy.state_fingerprint().get(),
    }
}

impl Facts {
    /// Every sample package written into a store in memory, and what the
    /// video shows read from it.
    pub fn read() -> Self {
        let engine = Engine::new(Store::memory().expect("a store")).expect("an engine");
        for (_, write) in packages() {
            write(&engine).expect("a sample package writes");
        }
        let maze = engine
            .run_experiment(&path!("maze/showcase"), &mut |_, _| {
                ControlFlow::Continue(())
            })
            .expect("the maze's showcase runs");
        Self {
            model: model(&engine),
            reserves: reserves(&engine),
            searched: searched(&engine, &maze),
            forks: forks(&engine, &maze, 3),
            learned: learned(&engine),
            packages: packages_shown(&engine),
            replay: replay(&engine, &maze),
            theorems: theorems(),
        }
    }
}
