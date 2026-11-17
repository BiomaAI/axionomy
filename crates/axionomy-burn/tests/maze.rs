//! The DQN over the key-door maze's Micro task: its network never prefers
//! an exchange the candidate set does not offer, and a short training run's
//! greedy episode only ever chooses offered ones.

use axionomy::path;
use axionomy_burn::{Observed, QNetwork, Settings, play, train};
use axionomy_engine::Engine;
use axionomy_problems::maze;
use axionomy_rl::{Episode, Reward, Task};
use axionomy_store::Store;
use burn::tensor::Device;
use std::sync::Arc;

fn task() -> Arc<Task> {
    let engine = Engine::new(Store::memory().unwrap()).unwrap();
    maze::package::write(&engine).unwrap();
    let reward = Reward {
        weights: vec![(path!("maze/energy"), 1.0), (path!("maze/time"), 1.0)],
        reached: 20.0,
    };
    Arc::new(Task::new(&engine, &path!("maze/micro"), &path!("maze/moves"), reward).unwrap())
}

#[test]
fn the_network_values_every_unoffered_exchange_below_every_offered_one() {
    let task = task();
    let device = Device::flex();
    let network = QNetwork::new(task.observed().len(), task.actions().len(), 32, 3, &device);
    let episode = Episode::new(Arc::clone(&task));
    let values = network
        .forward(Observed::new(episode.observation(), &device))
        .into_data()
        .try_to_vec::<f32>()
        .unwrap();
    let offered = &episode.observation().offered;
    let lowest_offered = values
        .iter()
        .zip(offered)
        .filter(|(_, offered)| **offered)
        .map(|(value, _)| *value)
        .fold(f32::INFINITY, f32::min);
    for (value, offered) in values.iter().zip(offered) {
        if !offered {
            assert!(*value < lowest_offered - 1.0e6, "{values:?}");
        }
    }
}

#[test]
fn a_short_training_run_only_ever_chooses_offered_exchanges() {
    let task = task();
    let settings = Settings {
        steps: 600,
        environments: 2,
        replay: 1_000,
        batch: 32,
        hidden: 32,
        ..Settings::default()
    };
    let artifacts = std::env::temp_dir().join("axionomy-burn-test-maze");
    let policy = train::<12>(&task, &settings, &artifacts);
    // `play` refuses an exchange that is not offered.
    let played = play(&task, &policy, 12).unwrap();
    assert!(!played.actions.is_empty());
    let (_, exchanges) = task.engine().history(played.point).unwrap();
    assert_eq!(exchanges.len(), played.actions.len());
}
