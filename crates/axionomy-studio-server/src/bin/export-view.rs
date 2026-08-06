use axionomy_service::{ReferenceService, RunRequest};
use std::{fs, path::PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = std::env::args_os().skip(1);
    let problem = arguments
        .next()
        .and_then(|value| value.into_string().ok())
        .unwrap_or_else(|| "all".into());
    let output = arguments
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("studio/public/artifacts"));
    let service = ReferenceService;
    let problems = if problem == "all" {
        service.catalog()
    } else {
        vec![
            service
                .problem(&problem)
                .ok_or_else(|| format!("unknown problem `{problem}`"))?,
        ]
    };
    fs::create_dir_all(&output)?;
    if problem == "all" {
        let catalog_path = output.join("catalog.json");
        fs::write(&catalog_path, serde_json::to_vec(&service.catalog())?)?;
        println!("wrote {}", catalog_path.display());
    }
    for descriptor in problems {
        let artifact = service.run(RunRequest::new(&descriptor.key))?;
        let path = output.join(format!("{}.json", descriptor.key));
        fs::write(&path, serde_json::to_vec(&artifact)?)?;
        println!("wrote {}", path.display());
    }
    Ok(())
}
