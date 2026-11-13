#![doc = include_str!("../README.md")]

mod package;
mod serve;

use axionomy_engine::EngineError;
use axionomy_store::StoreError;
use clap::{Parser, Subcommand};
use std::{net::SocketAddr, path::PathBuf};
use thiserror::Error;

#[derive(Debug, Parser)]
#[command(
    name = "axionomy",
    version,
    about = "Make, load and serve packages of closed economic models, over MCP and to Studio"
)]
pub struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Serve MCP over streamable HTTP at `/mcp`, with `/health`.
    Mcp {
        #[arg(long, env = "AXIONOMY_MCP_BIND", default_value = "127.0.0.1:8000")]
        bind: SocketAddr,
        /// A store file that keeps economies and their handles across
        /// restarts; in memory, until the server stops, when omitted.
        #[arg(long, env = "AXIONOMY_MCP_STORE")]
        store: Option<PathBuf>,
        /// A package file, zipped or not, to merge into the store before
        /// serving; repeat for more. A package that fails the checks loading
        /// makes stops the server. Without `--package`, the files
        /// `AXIONOMY_MCP_PACKAGES` lists, separated as `PATH` separates them.
        #[arg(long = "package", value_name = "FILE")]
        packages: Vec<PathBuf>,
    },
    /// Make, load and look into packages: store files holding economies,
    /// refs and what uses them, zipped when the file name ends in `.zip`.
    Package {
        #[command(subcommand)]
        action: package::Action,
    },
    /// Serve Studio's API, and its built frontend when there is one.
    Studio {
        #[arg(long, env = "AXIONOMY_STUDIO_BIND", default_value = "127.0.0.1:3000")]
        bind: SocketAddr,
        /// The built frontend to serve.
        #[arg(long, env = "AXIONOMY_STUDIO_DIR", default_value = "studio/dist")]
        frontend: PathBuf,
        /// A store file whose points and refs the API serves, such as one a
        /// package was loaded into; in memory, until the server stops, when
        /// omitted.
        #[arg(long, env = "AXIONOMY_STUDIO_STORE")]
        store: Option<PathBuf>,
        /// A package file, zipped or not, to merge into the store before
        /// serving; repeat for more. A package that fails the checks loading
        /// makes stops the server. Without `--package`, the files
        /// `AXIONOMY_STUDIO_PACKAGES` lists, separated as `PATH` separates
        /// them.
        #[arg(long = "package", value_name = "FILE")]
        packages: Vec<PathBuf>,
    },
}

#[derive(Debug, Error)]
pub enum CliError {
    #[error(transparent)]
    Arguments(#[from] clap::Error),
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
    #[error("could not read `{path}`: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("`{0}` exists; a package is written to a new file")]
    Exists(PathBuf),
    #[error("could not load package `{path}`: {source}")]
    Package { path: PathBuf, source: EngineError },
}

/// Runs `cli`, and gives the text it prints, if any. Serving runs until the
/// server is stopped.
pub fn run(cli: Cli) -> Result<Option<String>, CliError> {
    match cli.command {
        Command::Mcp {
            bind,
            store,
            packages,
        } => {
            let packages = packages_to_load(packages, std::env::var_os(MCP_PACKAGES));
            serve::mcp(bind, store, &packages).map(|()| None)
        }
        Command::Package { action } => package::run(action).map(Some),
        Command::Studio {
            bind,
            frontend,
            store,
            packages,
        } => {
            let packages = packages_to_load(packages, std::env::var_os(STUDIO_PACKAGES));
            serve::studio(bind, &frontend, store, &packages).map(|()| None)
        }
    }
}

/// The environment variables listing the package files `axionomy mcp` and
/// `axionomy studio` load when no `--package` names one.
const MCP_PACKAGES: &str = "AXIONOMY_MCP_PACKAGES";
const STUDIO_PACKAGES: &str = "AXIONOMY_STUDIO_PACKAGES";

/// The package files to load: those `given` on the command line, or else
/// those `listed` names, separated as `PATH` separates them.
fn packages_to_load(given: Vec<PathBuf>, listed: Option<std::ffi::OsString>) -> Vec<PathBuf> {
    if !given.is_empty() {
        return given;
    }
    listed
        .map(|listed| {
            std::env::split_paths(&listed)
                .filter(|file| !file.as_os_str().is_empty())
                .collect()
        })
        .unwrap_or_default()
}

/// Parses `arguments` and runs them.
pub fn run_from<I, T>(arguments: I) -> Result<Option<String>, CliError>
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    run(Cli::try_parse_from(arguments)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mcp_loads_the_packages_given_or_else_those_the_environment_lists() {
        let given = vec![PathBuf::from("maze.zip")];
        let listed = std::env::join_paths(["sokoban.zip", "work.redb"]).unwrap();
        assert_eq!(packages_to_load(given.clone(), Some(listed.clone())), given);
        assert_eq!(
            packages_to_load(Vec::new(), Some(listed)),
            [PathBuf::from("sokoban.zip"), PathBuf::from("work.redb")]
        );
        assert!(packages_to_load(Vec::new(), None).is_empty());
        let Cli {
            command: Command::Mcp { packages, .. },
        } = Cli::try_parse_from([
            "axionomy",
            "mcp",
            "--package",
            "maze.zip",
            "--package",
            "sokoban.zip",
        ])
        .unwrap()
        else {
            panic!("`mcp` serves MCP");
        };
        assert_eq!(
            packages,
            [PathBuf::from("maze.zip"), PathBuf::from("sokoban.zip")]
        );
    }

    #[test]
    fn studio_loads_the_packages_given() {
        let Cli {
            command: Command::Studio { packages, .. },
        } = Cli::try_parse_from(["axionomy", "studio", "--package", "maze.zip"]).unwrap()
        else {
            panic!("`studio` serves Studio");
        };
        assert_eq!(packages, [PathBuf::from("maze.zip")]);
    }
}
