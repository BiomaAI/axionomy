#![doc = include_str!("../README.md")]

mod serve;

use axionomy_engine::EngineError;
use axionomy_store::StoreError;
use axionomy_study::{
    InstancePath, ProblemPath, Registry, StrategyPath, Study, StudyError, StudyRequest,
};
use clap::{Parser, Subcommand, ValueEnum};
use std::{fs, net::SocketAddr, path::PathBuf};
use thiserror::Error;

#[derive(Debug, Parser)]
#[command(
    name = "axionomy",
    version,
    about = "Study closed economic problem models, and serve them over MCP and to Studio"
)]
pub struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    #[command(flatten)]
    Report(Report),
    /// Write the default study of a problem, or of every problem and the
    /// catalog, as `<problem>.json`: what Studio's static playback loads.
    ExportStudies {
        /// The directory to write into.
        directory: PathBuf,
        /// One problem; every problem and the catalog when omitted.
        #[arg(long)]
        problem: Option<ProblemPath>,
    },
    /// Serve MCP over streamable HTTP at `/mcp`, with `/health`.
    Mcp {
        #[arg(long, env = "AXIONOMY_MCP_BIND", default_value = "127.0.0.1:8000")]
        bind: SocketAddr,
        /// A store file that keeps economies and their handles across
        /// restarts; in memory, until the server stops, when omitted.
        #[arg(long, env = "AXIONOMY_MCP_STORE")]
        store: Option<PathBuf>,
    },
    /// Serve Studio's API, and its built frontend when there is one.
    Studio {
        #[arg(long, env = "AXIONOMY_STUDIO_BIND", default_value = "127.0.0.1:3000")]
        bind: SocketAddr,
        /// The built frontend to serve.
        #[arg(long, env = "AXIONOMY_STUDIO_DIR", default_value = "studio/dist")]
        frontend: PathBuf,
    },
}

/// The commands that report on the problems, as text or JSON.
#[derive(Debug, Subcommand)]
enum Report {
    /// List every problem and its default strategy.
    Catalog {
        #[arg(long)]
        json: bool,
    },
    /// Explain one problem: its family, capabilities, instances and
    /// strategies.
    Describe {
        problem: ProblemPath,
        #[arg(long)]
        json: bool,
    },
    /// Study a problem and emit a summary or the portable study as JSON.
    Study {
        problem: ProblemPath,
        /// Select one of the problem's instances, such as `micro`; `describe`
        /// lists them. The problem's default instance when omitted.
        #[arg(long)]
        instance: Option<InstancePath>,
        #[arg(long)]
        strategy: Option<StrategyPath>,
        #[arg(long, default_value_t = 17)]
        seed: u64,
        #[arg(long, default_value_t = 128)]
        budget: u64,
        #[arg(long, value_enum, default_value_t = OutputFormat::Summary)]
        format: OutputFormat,
        /// Write the result to a file instead of stdout.
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum OutputFormat {
    Summary,
    Json,
}

#[derive(Debug, Error)]
pub enum CliError {
    #[error(transparent)]
    Arguments(#[from] clap::Error),
    #[error(transparent)]
    Study(#[from] StudyError),
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error(transparent)]
    Engine(#[from] EngineError),
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("could not encode JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("could not write `{path}`: {source}")]
    Write {
        path: PathBuf,
        source: std::io::Error,
    },
}

struct CliOutput {
    text: String,
    output: Option<PathBuf>,
}

/// Runs `cli` over the problems of `registry`, and gives the text it prints,
/// if any. Serving runs until the server is stopped.
pub fn run(registry: Registry, cli: Cli) -> Result<Option<String>, CliError> {
    match cli.command {
        Command::Report(report) => execute(&registry, report).and_then(write_output),
        Command::ExportStudies { directory, problem } => {
            export_studies(&registry, problem, &directory).map(Some)
        }
        Command::Mcp { bind, store } => serve::mcp(registry, bind, store).map(|()| None),
        Command::Studio { bind, frontend } => {
            serve::studio(registry, bind, &frontend).map(|()| None)
        }
    }
}

/// Parses `arguments` and runs them over the problems of `registry`.
pub fn run_from<I, T>(registry: Registry, arguments: I) -> Result<Option<String>, CliError>
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    run(registry, Cli::try_parse_from(arguments)?)
}

fn execute(registry: &Registry, report: Report) -> Result<CliOutput, CliError> {
    match report {
        Report::Catalog { json } => {
            let catalog = registry.catalog();
            let text = if json {
                serde_json::to_string_pretty(&catalog)?
            } else {
                catalog
                    .into_iter()
                    .map(|problem| {
                        format!(
                            "{:<16} {:<24} instance={} strategy={}\n  {}",
                            problem.key.as_str(),
                            problem.title,
                            problem.default_instance,
                            problem.default_strategy,
                            problem.summary
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            };
            Ok(CliOutput { text, output: None })
        }
        Report::Describe { problem, json } => {
            let descriptor = registry
                .problem(&problem)
                .ok_or_else(|| StudyError::UnknownProblem(problem.clone()))?;
            let text = if json {
                serde_json::to_string_pretty(&descriptor)?
            } else {
                let strategies = descriptor
                    .strategies
                    .iter()
                    .map(|strategy| {
                        format!(
                            "  {:<20} {}{}\n    {}",
                            strategy.key.as_str(),
                            strategy.label,
                            if strategy.key == descriptor.default_strategy {
                                " (default)"
                            } else {
                                ""
                            },
                            strategy.description
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                let instances = descriptor
                    .instances
                    .iter()
                    .map(|instance| {
                        format!(
                            "  {:<20} {}{}\n    {}",
                            instance.key.as_str(),
                            instance.label,
                            if instance.key == descriptor.default_instance {
                                " (default)"
                            } else {
                                ""
                            },
                            instance.description
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                format!(
                    "{} [{}]\n{}\n\nFamily: {}\nCapabilities: {}\n\nInstances:\n{}\n\nStrategies:\n{}",
                    descriptor.title,
                    descriptor.key,
                    descriptor.summary,
                    descriptor.family.label,
                    descriptor
                        .capabilities
                        .iter()
                        .map(|capability| capability.label.as_str())
                        .collect::<Vec<_>>()
                        .join(", "),
                    instances,
                    strategies
                )
            };
            Ok(CliOutput { text, output: None })
        }
        Report::Study {
            problem,
            instance,
            strategy,
            seed,
            budget,
            format,
            output,
        } => {
            let mut request = StudyRequest::new(problem);
            request.instance = instance;
            request.strategy = strategy;
            request.seed = seed;
            request.budget = budget;
            let study = registry.study(request)?;
            let text = match format {
                OutputFormat::Json => serde_json::to_string_pretty(&study)?,
                OutputFormat::Summary => summary(&study),
            };
            Ok(CliOutput { text, output })
        }
    }
}

/// Writes the default study of `problem`, or of every problem and the
/// catalog, into `directory`, and says what it wrote.
fn export_studies(
    registry: &Registry,
    problem: Option<ProblemPath>,
    directory: &std::path::Path,
) -> Result<String, CliError> {
    let all = problem.is_none();
    let problems = match problem {
        Some(key) => vec![
            registry
                .problem(&key)
                .ok_or(StudyError::UnknownProblem(key.clone()))?
                .clone(),
        ],
        None => registry.catalog(),
    };
    fs::create_dir_all(directory)?;
    let mut wrote = Vec::new();
    if all {
        let path = directory.join("catalog.json");
        fs::write(&path, serde_json::to_vec(&registry.catalog())?)?;
        wrote.push(format!("wrote {}", path.display()));
    }
    for descriptor in problems {
        let path = directory.join(format!("{}.json", descriptor.key));
        let study = registry.study(StudyRequest::new(descriptor.key))?;
        fs::write(&path, serde_json::to_vec(&study)?)?;
        wrote.push(format!("wrote {}", path.display()));
    }
    Ok(wrote.join("\n"))
}

fn summary(study: &Study) -> String {
    let selected = study
        .selected()
        .expect("studies always name an included document");
    let objectives = if selected.objectives.is_empty() {
        "none".into()
    } else {
        selected
            .objectives
            .iter()
            .map(|objective| format!("{}={}", objective.label, objective.written()))
            .collect::<Vec<_>>()
            .join(", ")
    };
    format!(
        "{}\n{}\nstudy: {}\ninstance: {}\nselected: {}\nalternatives: {}\nexchanges: {}\nobjectives: {}\nassessed proposals: {}",
        selected.title,
        selected.description,
        study.key,
        study.instance.key,
        selected.id,
        study.documents.len().saturating_sub(1),
        selected.frames.len(),
        objectives,
        study.assessed_proposals.len(),
    )
}

fn write_output(output: CliOutput) -> Result<Option<String>, CliError> {
    if let Some(path) = output.output {
        std::fs::write(&path, format!("{}\n", output.text))
            .map_err(|source| CliError::Write { path, source })?;
        Ok(None)
    } else {
        Ok(Some(output.text))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axionomy_problems::registry;

    /// What running `arguments` over the sample problems prints.
    fn printed<const N: usize>(arguments: [&str; N]) -> Result<String, CliError> {
        run_from(registry(), arguments).map(|text| text.expect("a report prints"))
    }

    #[test]
    fn a_json_study_is_the_registry_study() {
        let text = printed([
            "axionomy",
            "study",
            "maze",
            "--strategy",
            "a-star",
            "--format",
            "json",
        ])
        .unwrap();
        let cli_study: Study = serde_json::from_str(&text).unwrap();
        let registry_study = registry()
            .study(
                StudyRequest::new("maze".parse().unwrap()).with_strategy("a-star".parse().unwrap()),
            )
            .unwrap();
        assert_eq!(cli_study, registry_study);
    }

    #[test]
    fn catalog_is_human_readable_by_default() {
        let text = printed(["axionomy", "catalog"]).unwrap();
        assert!(text.contains("maze"));
        assert!(text.contains("perishables"));
        assert!(text.contains("work-league"));
    }

    #[test]
    fn identifiers_are_checked_where_they_are_read() {
        for arguments in [
            ["axionomy", "study", "work_league"],
            ["axionomy", "study", "Maze"],
        ] {
            assert!(printed(arguments).is_err());
        }
        for arguments in [
            // An instance the problem does not declare, and one that is not a path.
            ["axionomy", "study", "maze", "--instance", "huge"],
            ["axionomy", "study", "maze", "--instance", "Micro"],
            ["axionomy", "study", "maze", "--strategy", "a_star"],
        ] {
            assert!(printed(arguments).is_err());
        }
    }

    #[test]
    fn an_instance_is_one_its_problem_declares() {
        let text = printed(["axionomy", "study", "maze", "--instance", "micro"]).unwrap();
        assert!(text.contains("instance: micro"));
        let described = printed(["axionomy", "describe", "maze"]).unwrap();
        assert!(described.contains("Family: Pathfinding"));
        assert!(described.contains("showcase             Showcase (default)"));
    }

    #[test]
    fn exported_studies_are_the_registry_studies() {
        let directory = std::env::temp_dir().join(format!("axionomy-cli-{}", std::process::id()));
        let _ = fs::remove_dir_all(&directory);
        let written = directory.to_str().unwrap().to_owned();
        printed(["axionomy", "export-studies", &written, "--problem", "maze"]).unwrap();
        let exported: Study =
            serde_json::from_slice(&fs::read(directory.join("maze.json")).unwrap()).unwrap();
        let studied = registry()
            .study(StudyRequest::new("maze".parse().unwrap()))
            .unwrap();
        assert_eq!(exported, studied);
        assert!(!directory.join("catalog.json").exists());
        fs::remove_dir_all(&directory).unwrap();
    }
}
