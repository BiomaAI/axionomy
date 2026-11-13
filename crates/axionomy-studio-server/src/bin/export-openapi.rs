use axionomy_engine::Engine;
use axionomy_store::Store;
use axionomy_studio_server::{StudioState, api};
use std::{fs, path::PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("studio/openapi.json"));
    // The contract is the same whatever the store holds.
    let engine = Engine::new(Store::memory()?)?;
    let (_, openapi) = api(StudioState::new(engine));
    let encoded = serde_json::to_string_pretty(&openapi)? + "\n";
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&output, encoded)?;
    println!("wrote {}", output.display());
    Ok(())
}
