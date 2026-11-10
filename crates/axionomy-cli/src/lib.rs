use axionomy_service::{
    InstanceProfile, ProblemPath, ReferenceService, ServiceError, StrategyPath, Study, StudyRequest,
};
use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Parser)]
#[command(
    name = "axionomy",
    version,
    about = "Inspect and study closed economic problem models"
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
        problem: ProblemPath,
        #[arg(long)]
        json: bool,
    },
    /// Study a problem and emit a summary or the portable study as JSON.
    Study {
        problem: ProblemPath,
        /// Select the micro, showcase or stress instance.
        #[arg(long)]
        instance: Option<InstanceProfile>,
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
                            instance.profile,
                            instance.label,
                            if instance.profile == descriptor.default_instance {
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
                    "{} [{}]\n{}\n\nCapabilities: {}\n\nInstances:\n{}\n\nStrategies:\n{}",
                    descriptor.title,
                    descriptor.key,
                    descriptor.summary,
                    descriptor
                        .capabilities
                        .iter()
                        .map(|capability| format!("{capability:?}"))
                        .collect::<Vec<_>>()
                        .join(", "),
                    instances,
                    strategies
                )
            };
            Ok(CliOutput { text, output: None })
        }
        Command::Study {
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
            let study = service.study(request)?;
            let text = match format {
                OutputFormat::Json => serde_json::to_string_pretty(&study)?,
                OutputFormat::Summary => summary(&study),
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

fn summary(study: &Study) -> String {
    let selected = study
        .selected()
        .expect("service studies always name an included document");
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
        study.instance.profile,
        selected.id,
        study.documents.len().saturating_sub(1),
        selected.frames.len(),
        objectives,
        study.assessed_proposals.len(),
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
    fn a_json_study_is_the_shared_service_study() {
        let output = run_from([
            "axionomy",
            "study",
            "maze",
            "--strategy",
            "a-star",
            "--format",
            "json",
        ])
        .unwrap();
        let cli_study: Study = serde_json::from_str(&output.text).unwrap();
        let service_study = ReferenceService
            .study(
                StudyRequest::new("maze".parse().unwrap()).with_strategy("a-star".parse().unwrap()),
            )
            .unwrap();
        assert_eq!(cli_study, service_study);
    }

    #[test]
    fn catalog_is_human_readable_by_default() {
        let output = run_from(["axionomy", "catalog"]).unwrap();
        assert!(output.text.contains("maze"));
        assert!(output.text.contains("perishables"));
        assert!(output.text.contains("work-league"));
    }

    #[test]
    fn identifiers_are_checked_where_they_are_read() {
        for arguments in [
            ["axionomy", "study", "work_league"],
            ["axionomy", "study", "Maze"],
        ] {
            assert!(run_from(arguments).is_err());
        }
        assert!(run_from(["axionomy", "study", "maze", "--instance", "huge"]).is_err());
        assert!(run_from(["axionomy", "study", "maze", "--strategy", "a_star"]).is_err());
    }
}
