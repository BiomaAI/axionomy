use axionomy_service::{ProblemPath, StudyRequest, registry};
use std::{fs, path::PathBuf};

/// Writes the default study of one problem, or of every problem and the
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
        .unwrap_or_else(|| PathBuf::from("studio/public/studies"));
    let registry = registry();
    let all = problem == "all";
    let problems = if all {
        registry.catalog()
    } else {
        let key: ProblemPath = problem.parse()?;
        vec![
            registry
                .problem(&key)
                .ok_or_else(|| format!("unknown problem `{key}`"))?
                .clone(),
        ]
    };
    fs::create_dir_all(&output)?;
    if all {
        let catalog_path = output.join("catalog.json");
        fs::write(&catalog_path, serde_json::to_vec(&registry.catalog())?)?;
        println!("wrote {}", catalog_path.display());
    }
    for descriptor in problems {
        let path = output.join(format!("{}.json", descriptor.key));
        let study = registry.study(StudyRequest::new(descriptor.key))?;
        fs::write(&path, serde_json::to_vec(&study)?)?;
        println!("wrote {}", path.display());
    }
    Ok(())
}
