use axionomy_problems::maze_view::{self, MazeStrategy};
use std::{fs, path::PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = std::env::args_os().skip(1);
    let strategy_key = arguments
        .next()
        .and_then(|value| value.into_string().ok())
        .unwrap_or_else(|| "maze_pareto_energy".into());
    let strategy = MazeStrategy::from_key(&strategy_key)
        .ok_or_else(|| format!("unknown Maze strategy `{strategy_key}`"))?;
    let output = arguments
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("studio/public/examples/maze-pareto-energy.json"));
    let document = maze_view::document(strategy)?;
    let encoded = serde_json::to_string_pretty(&document)? + "\n";
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&output, encoded)?;
    println!("wrote {}", output.display());
    Ok(())
}
