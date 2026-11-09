use axionomy_service::{ProblemPath, ReferenceService, RunRequest};
use std::{fs, path::PathBuf};

/// Writes the default artifact of one problem, or of every problem and the
/// catalog when the problem is `all`, as `<problem>.json`.
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
    let all = problem == "all";
    let problems = if all {
        service.catalog()
    } else {
        let key: ProblemPath = problem.parse()?;
        vec![
            service
                .problem(&key)
                .ok_or_else(|| format!("unknown problem `{key}`"))?,
        ]
    };
    fs::create_dir_all(&output)?;
    if all {
        let catalog_path = output.join("catalog.json");
        fs::write(&catalog_path, serde_json::to_vec(&service.catalog())?)?;
        println!("wrote {}", catalog_path.display());
    }
    for descriptor in problems {
        let path = output.join(format!("{}.json", descriptor.key));
        let artifact = service.run(RunRequest::new(descriptor.key))?;
        fs::write(&path, serde_json::to_vec(&artifact)?)?;
        println!("wrote {}", path.display());
    }
    Ok(())
}
