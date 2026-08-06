use axionomy_studio_server::{StudioState, api};
use std::{fs, path::PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("studio/openapi.json"));
    let (_, openapi) = api(StudioState::default());
    let encoded = serde_json::to_string_pretty(&openapi)? + "\n";
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&output, encoded)?;
    println!("wrote {}", output.display());
    Ok(())
}
