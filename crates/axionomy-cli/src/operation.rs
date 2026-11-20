//! Evaluate an economic operation from the CLI with the shared Rhai host.
use crate::CliError;
use axionomy::AccountPath;
use axionomy_engine::{Engine, RhaiInput, ScriptCall, ScriptFunction, ScriptPath, ScriptState};
use axionomy_store::{Point, RefPath, Store};
use axionomy_studio_api::PointId;
use clap::{ArgGroup, Args};
use std::{fs, io, path::PathBuf, str::FromStr};

#[derive(Debug, Clone)]
pub(crate) enum At {
    Ref(RefPath),
    Point(PointId),
}
impl FromStr for At {
    type Err = String;
    fn from_str(id: &str) -> Result<Self, String> {
        id.parse()
            .map(Self::Point)
            .or_else(|_| id.parse().map(Self::Ref))
            .map_err(|e| format!("invalid ref or point `{id}`: {e}"))
    }
}

#[derive(Debug, Args)]
#[command(group(ArgGroup::new("input").required(true).args(["source", "file", "script"])))]
pub(crate) struct Operation {
    /// Package directory, archive or redb store. Applying needs a writable redb store.
    package: PathBuf,
    /// Starting ref or run point.
    at: At,
    /// Inline Rhai expression or program.
    #[arg(long)]
    source: Option<String>,
    /// Read the operation's Rhai source from this file.
    #[arg(long)]
    file: Option<PathBuf>,
    /// Call this package script's named function.
    #[arg(long, requires = "function")]
    script: Option<ScriptPath>,
    #[arg(long, requires = "script")]
    function: Option<ScriptFunction>,
    /// Limit the script's observation to this account. Repeat for several accounts.
    #[arg(long = "visible")]
    visible: Vec<AccountPath>,
}

pub(crate) enum Mode {
    Query,
    Assess,
    Apply,
}

impl Operation {
    pub(crate) fn run(self, mode: Mode) -> Result<String, CliError> {
        let input = match (self.source, self.file, self.script, self.function) {
            (Some(text), None, None, None) => RhaiInput::Source(text),
            (None, Some(file), None, None) => RhaiInput::Source(fs::read_to_string(file)?),
            (None, None, Some(script), Some(function)) => {
                RhaiInput::Function(ScriptCall::new(script, function))
            }
            _ => {
                return Err(
                    io::Error::other("specify an inline program, file or script function").into(),
                );
            }
        };
        let engine = if matches!(mode, Mode::Apply) {
            if !self.package.is_file() || self.package.extension().is_none_or(|ext| ext != "redb") {
                return Err(io::Error::other(
                    "apply needs an existing .redb store; build or load the package first",
                )
                .into());
            }
            Engine::new(Store::open(&self.package)?)?
        } else if self.package.is_dir() {
            crate::source::build(&self.package)?.engine
        } else {
            Engine::new(axionomy_engine::package(&fs::read(&self.package)?)?)?
        };
        let at: Point = match self.at {
            At::Ref(id) => engine.referred(&id)?,
            At::Point(id) => id.point(),
        };
        let reached = engine.reach(at)?;
        let mut state = ScriptState::new(reached.state.economy.clone());
        if !self.visible.is_empty() {
            state = state.observing(self.visible.into_iter().collect());
        }
        Ok(match mode {
            Mode::Query => serde_json::to_string_pretty(&engine.query(&input, state)?)?,
            Mode::Assess => {
                let proposed = engine.evaluate_exchange(&input, state)?;
                serde_json::to_string_pretty(&reached.state.economy.assess(&proposed))?
            }
            Mode::Apply => {
                let proposed = engine.evaluate_exchange(&input, state)?;
                PointId::from(engine.apply(at, &[proposed])?).to_string()
            }
        })
    }
}
