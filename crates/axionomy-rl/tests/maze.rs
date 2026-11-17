//! A task over the key-door maze's Micro experiment: its actions are every
//! maze rate's instance, its observations the balances they touch, and an
//! episode earns the objectives' change and the goal, as a recorded run.

use axionomy::{RatePath, path};
use axionomy_engine::Engine;
use axionomy_problems::{bridge, maze};
use axionomy_rl::{Episode, Reward, RlError, Task};
use axionomy_store::Store;
use std::sync::Arc;

fn engine() -> Engine {
    let engine = Engine::new(Store::memory().unwrap()).unwrap();
    maze::package::write(&engine).unwrap();
    engine
}

/// Energy weighs twice what time does, so a mix-up shows.
fn reward() -> Reward {
    Reward {
        weights: vec![(path!("maze/energy"), 1.0), (path!("maze/time"), 0.5)],
        reached: 10.0,
    }
}

fn task(engine: &Engine) -> Arc<Task> {
    Arc::new(Task::new(engine, &path!("maze/micro"), &path!("maze/moves"), reward()).unwrap())
}

/// The action applying the instance `rate` names.
fn action(task: &Task, rate: &str) -> usize {
    let rate: RatePath = rate.parse().unwrap();
    task.actions()
        .iter()
        .position(|exchange| *exchange.rate() == rate)
        .unwrap_or_else(|| panic!("no action applies {rate}"))
}

/// The key route: the least energy the Micro maze is solved for.
const KEY_ROUTE: [&str; 6] = [
    "move/start/key-room/2/false",
    "take-key/key-room",
    "unlock-door/key-room",
    "move/key-room/gate/2/true",
    "move/gate/exit/2/false",
    "finish",
];

#[test]
fn a_task_lists_every_instance_and_observes_what_they_touch() {
    let engine = engine();
    let task = task(&engine);
    let rates: Vec<String> = task
        .actions()
        .iter()
        .map(|exchange| exchange.rate().to_string())
        .collect();
    assert_eq!(
        rates,
        [
            "finish",
            "move/detour/exit/5/false",
            "move/gate/exit/2/false",
            "move/key-room/gate/2/true",
            "move/start/detour/4/false",
            "move/start/key-room/2/false",
            "take-key/key-room",
            "unlock-door/key-room",
        ]
    );
    for exchange in task.actions() {
        let bound: Vec<String> = exchange
            .bindings()
            .iter()
            .map(|(role, account)| format!("{role}={account}"))
            .collect();
        assert_eq!(bound, ["actor=agent", "environment=world"]);
    }
    let observed: Vec<String> = task
        .observed()
        .iter()
        .map(|(account, asset)| format!("{account}/{asset}"))
        .collect();
    // What the start holds, and what only an exchange makes.
    for balance in [
        "agent/at/start",
        "agent/energy",
        "world/key",
        "world/locked",
    ] {
        assert!(observed.iter().any(|seen| seen == balance), "{balance}");
    }
    for made in ["agent/at/exit", "agent/key", "agent/solved", "world/open"] {
        assert!(observed.iter().any(|seen| seen == made), "{made}");
    }
    let episode = Episode::new(Arc::clone(&task));
    let offered: Vec<usize> = (0..task.actions().len())
        .filter(|&action| episode.observation().offered[action])
        .collect();
    assert_eq!(
        offered,
        [
            action(&task, "move/start/detour/4/false"),
            action(&task, "move/start/key-room/2/false")
        ]
    );
    let energy = task
        .observed()
        .iter()
        .position(|(account, asset)| account.as_str() == "agent" && asset.as_str() == "energy")
        .unwrap();
    assert_eq!(episode.observation().balances[energy], 9.0);
}

#[test]
fn an_episode_earns_the_objectives_change_and_the_goal_as_a_recorded_run() {
    let engine = engine();
    let task = task(&engine);
    let mut episode = Episode::new(Arc::clone(&task));
    let mut earned = 0.0;
    for (index, rate) in KEY_ROUTE.iter().enumerate() {
        let step = episode.step(action(&task, rate)).unwrap();
        let last = index + 1 == KEY_ROUTE.len();
        assert_eq!(step.reached, last, "{rate}");
        assert!(!step.stuck, "{rate}");
        assert_eq!(step.ended(), last, "{rate}");
        assert_eq!(step.point, episode.point());
        earned += step.reward;
    }
    let end = episode.point();
    let energy = engine.measure(end, &path!("maze/energy")).unwrap().to_u64();
    let time = engine.measure(end, &path!("maze/time")).unwrap().to_u64();
    assert_eq!(energy, 6, "the key route's energy");
    // Both objectives are minimized from zero, so the steps' rewards add up
    // to the route's whole cost, charged, and the goal's reward.
    assert_eq!(earned, -(energy as f64) - 0.5 * time as f64 + 10.0);
    // Nothing is offered once the maze is solved.
    assert!(!episode.observation().offered.contains(&true));

    // The episode is a run of the engine's: its history is the route.
    let (_, exchanges) = engine.history(end).unwrap();
    let taken: Vec<String> = exchanges
        .iter()
        .map(|exchange| exchange.rate().to_string())
        .collect();
    assert_eq!(taken, KEY_ROUTE);
    assert!(engine.holds(end, task.goal()).unwrap());

    episode.reset();
    assert_eq!(episode.point(), task.start());
    assert_eq!(episode.observation(), Episode::new(task).observation());
}

#[test]
fn an_action_not_offered_is_refused_and_changes_nothing() {
    let engine = engine();
    let task = task(&engine);
    let mut episode = Episode::new(Arc::clone(&task));
    let take_key = action(&task, "take-key/key-room");
    assert!(matches!(
        episode.step(take_key),
        Err(RlError::NotOffered { action, .. }) if action == take_key
    ));
    let actions = task.actions().len();
    assert!(matches!(
        episode.step(actions),
        Err(RlError::NoSuchAction { action, actions: listed }) if action == actions && listed == actions
    ));
    assert_eq!(episode.point(), task.start());
}

#[test]
fn a_set_a_script_proposes_has_no_fixed_list() {
    let engine = Engine::new(Store::memory().unwrap()).unwrap();
    bridge::package::write(&engine).unwrap();
    let refused = Task::new(
        &engine,
        &path!("bridge/micro"),
        &path!("bridge/mechanisms"),
        Reward {
            weights: Vec::new(),
            reached: 1.0,
        },
    );
    assert!(matches!(refused, Err(RlError::Scripted(set)) if set.as_str() == "bridge/mechanisms"));
}
