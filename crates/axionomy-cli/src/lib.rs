use axionomy_service::{ReferenceService, RunArtifact, RunRequest, ServiceError};
use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Parser)]
#[command(
    name = "axionomy",
    version,
    about = "Inspect and run closed economic problem models"
)]
pub struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// List every canonical problem and its default strategy.
    Catalog {
        #[arg(long)]
        json: bool,
    },
    /// Explain one problem's strategies and capabilities.
    Describe {
        problem: String,
        #[arg(long)]
        json: bool,
    },
    /// Run a problem and emit a summary or portable artifact JSON.
    Run {
        problem: String,
        #[arg(long)]
        strategy: Option<String>,
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
    Service(#[from] ServiceError),
    #[error("could not encode JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("could not write `{path}`: {source}")]
    Write {
        path: PathBuf,
        source: std::io::Error,
    },
}

pub struct CliOutput {
    pub text: String,
    pub output: Option<PathBuf>,
}

pub fn execute(cli: Cli) -> Result<CliOutput, CliError> {
    let service = ReferenceService;
    match cli.command {
        Command::Catalog { json } => {
            let catalog = service.catalog();
            let text = if json {
                serde_json::to_string_pretty(&catalog)?
            } else {
                catalog
                    .into_iter()
                    .map(|problem| {
                        format!(
                            "{:<16} {:<24} default={}\n  {}",
                            problem.key, problem.title, problem.default_strategy, problem.summary
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            };
            Ok(CliOutput { text, output: None })
        }
        Command::Describe { problem, json } => {
            let descriptor = service
                .problem(&problem)
                .ok_or_else(|| ServiceError::UnknownProblem(problem.clone()))?;
            let text = if json {
                serde_json::to_string_pretty(&descriptor)?
            } else {
                let strategies = descriptor
                    .strategies
                    .iter()
                    .map(|strategy| {
                        format!(
                            "  {:<20} {}{}\n    {}",
                            strategy.key,
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
                format!(
                    "{} [{}]\n{}\n\nCapabilities: {}\n\nStrategies:\n{}",
                    descriptor.title,
                    descriptor.key,
                    descriptor.summary,
                    descriptor
                        .capabilities
                        .iter()
                        .map(|capability| format!("{capability:?}"))
                        .collect::<Vec<_>>()
                        .join(", "),
                    strategies
                )
            };
            Ok(CliOutput { text, output: None })
        }
        Command::Run {
            problem,
            strategy,
            seed,
            budget,
            format,
            output,
        } => {
            let mut request = RunRequest::new(problem);
            request.strategy = strategy;
            request.seed = seed;
            request.budget = budget;
            let artifact = service.run(request)?;
            let text = match format {
                OutputFormat::Json => serde_json::to_string_pretty(&artifact)?,
                OutputFormat::Summary => summary(&artifact),
            };
            Ok(CliOutput { text, output })
        }
    }
}

pub fn run_from<I, T>(arguments: I) -> Result<CliOutput, clap::Error>
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    Cli::try_parse_from(arguments)
        .map(execute)
        .and_then(|result| {
            result.map_err(|error| clap::Error::raw(clap::error::ErrorKind::Io, error.to_string()))
        })
}

fn summary(artifact: &RunArtifact) -> String {
    let selected = artifact
        .selected_document()
        .expect("service artifacts always name an included document");
    let objectives = if selected.objectives.is_empty() {
        "none".into()
    } else {
        selected
            .objectives
            .iter()
            .map(|objective| format!("{}={}", objective.label, objective.value))
            .collect::<Vec<_>>()
            .join(", ")
    };
    format!(
        "{}\n{}\nartifact: {}\nselected: {}\nalternatives: {}\nexchanges: {}\nobjectives: {}\nassessed proposals: {}",
        selected.title,
        selected.description,
        artifact.id,
        selected.id,
        artifact.documents.len().saturating_sub(1),
        selected.frames.len(),
        objectives,
        artifact.assessed_proposals.len(),
    )
}

pub fn write_output(output: CliOutput) -> Result<Option<String>, CliError> {
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

    #[test]
    fn json_run_is_the_shared_service_artifact() {
        let output = run_from([
            "axionomy",
            "run",
            "maze",
            "--strategy",
            "a_star",
            "--format",
            "json",
        ])
        .unwrap();
        let cli_artifact: RunArtifact = serde_json::from_str(&output.text).unwrap();
        let service_artifact = ReferenceService
            .run(RunRequest::new("maze").with_strategy("a_star"))
            .unwrap();
        assert_eq!(cli_artifact, service_artifact);
    }

    #[test]
    fn catalog_is_human_readable_by_default() {
        let output = run_from(["axionomy", "catalog"]).unwrap();
        assert!(output.text.contains("maze"));
        assert!(output.text.contains("perishables"));
    }
}
