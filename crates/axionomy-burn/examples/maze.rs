//! Trains a DQN on the key-door maze's Showcase twice, once weighing energy
//! and once time, and sets each learned route beside the routes the
//! experiment's searches find.
//!
//! `cargo run --release --example maze`, a few minutes for each weighing.
//! Given a directory, `-- <dir>`, it also writes there what each weighing
//! learned: `learned-<weighing>.txt`, the route's exchanges by rate, one a
//! line, and `rewards-<weighing>.txt`, what each training episode earned, one
//! a line, in order.

use axionomy::path;
use axionomy_burn::{Settings, play, train};
use axionomy_engine::{Engine, ExperimentPath, ObjectivePath};
use axionomy_problems::maze;
use axionomy_rl::{Reward, Task};
use axionomy_store::Store;
use std::ops::ControlFlow;
use std::sync::Arc;

/// Steps an episode may take before it is cut off.
const MAX_STEPS: usize = 40;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out = std::env::args_os().nth(1).map(std::path::PathBuf::from);
    let engine = Engine::new(Store::memory()?)?;
    maze::package::write(&engine)?;
    let experiment: ExperimentPath = path!("maze/showcase");
    let objectives: [ObjectivePath; 2] = [path!("maze/energy"), path!("maze/time")];

    println!("{experiment}, searched:");
    let results = engine.run_experiment(&experiment, &mut |_, _| ControlFlow::Continue(()))?;
    let mut searched = Vec::new();
    for arm in &results.arms {
        let trial = &arm.trials[0];
        let values: Vec<u64> = trial.values.iter().map(|value| value.to_u64()).collect();
        println!(
            "  {:<24} energy {:>2}  time {:>2}",
            arm.arm.strategy, values[0], values[1]
        );
        searched.push(values);
    }

    for (name, energy, time) in [("energy", 1.0, 0.1), ("time", 0.1, 1.0)] {
        let reward = Reward {
            weights: vec![
                (objectives[0].clone(), energy),
                (objectives[1].clone(), time),
            ],
            reached: 50.0,
        };
        let task = Arc::new(Task::new(
            &engine,
            &experiment,
            &path!("maze/moves"),
            reward,
        )?);
        let artifacts = std::env::temp_dir().join(format!("axionomy-burn-maze-{name}"));
        // Burn adds to the logs it finds, so each run starts without them.
        if artifacts.exists() {
            std::fs::remove_dir_all(&artifacts)?;
        }
        let settings = Settings {
            steps: 150_000,
            epsilon_decay: 50_000.0,
            ..Settings::default()
        };
        let policy = train::<MAX_STEPS>(&task, &settings, &artifacts);
        let played = play(&task, &policy, MAX_STEPS)?;
        let values: Vec<u64> = objectives
            .iter()
            .map(|objective| Ok(engine.measure(played.point, objective)?.to_u64()))
            .collect::<Result<_, axionomy_engine::EngineError>>()?;
        let dominated = searched
            .iter()
            .any(|other| other[0] <= values[0] && other[1] <= values[1] && *other != values);
        println!(
            "learned, weighing {name}: energy {:>2}  time {:>2}  in {} exchanges, {}{}",
            values[0],
            values[1],
            played.actions.len(),
            if played.reached {
                "solved"
            } else {
                "not solved"
            },
            if dominated {
                ", beaten by a search"
            } else {
                ""
            },
        );
        if let Some(out) = &out {
            let route: String = played
                .actions
                .iter()
                .map(|&action| format!("{}\n", task.actions()[action].rate()))
                .collect();
            std::fs::write(out.join(format!("learned-{name}.txt")), route)?;
            // Burn logs each episode as `reward,epoch`.
            let log = std::fs::read_to_string(artifacts.join("train/epoch-1/Cum._Reward.log"))?;
            let rewards: String = log
                .lines()
                .filter_map(|line| line.split(',').next())
                .map(|reward| format!("{reward}\n"))
                .collect();
            std::fs::write(out.join(format!("rewards-{name}.txt")), rewards)?;
        }
    }
    Ok(())
}
