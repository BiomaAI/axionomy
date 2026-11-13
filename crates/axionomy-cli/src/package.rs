//! `axionomy package`: export a package from a store, load one into a store,
//! and say what one holds.

use crate::CliError;
use axionomy_engine::{
    Contents, Engine, Manifest, PackagePath, Version, package as read_package, zipped,
};
use axionomy_store::{DefinitionPath, RefPath, Store, Target};
use clap::Subcommand;
use std::{fs, path::PathBuf};

#[derive(Debug, Subcommand)]
pub(crate) enum Action {
    /// Copy what a package needs from a store into a new package file:
    /// everything named under the package's name, or the refs and
    /// definitions given, with all they reach.
    Export {
        /// The store to copy from.
        store: PathBuf,
        /// The package file to write; zipped when it ends in `.zip`.
        output: PathBuf,
        #[arg(long)]
        name: PackagePath,
        /// `major.minor.patch`.
        #[arg(long)]
        version: Version,
        /// A ref to carry; repeat for more.
        #[arg(long = "ref")]
        refs: Vec<RefPath>,
        /// A definition to carry, such as `experiment/maze/routes`; repeat
        /// for more.
        #[arg(long = "definition")]
        definitions: Vec<DefinitionPath>,
    },
    /// Merge a package file into a store, creating the store if needed.
    Load { store: PathBuf, package: PathBuf },
    /// Say what a package file holds.
    Inspect { package: PathBuf },
    /// Write every sample's package, zipped, as `<sample>.zip`.
    Samples {
        /// The directory to write into.
        directory: PathBuf,
    },
}

pub(crate) fn run(action: Action) -> Result<String, CliError> {
    match action {
        Action::Export {
            store,
            output,
            name,
            version,
            refs,
            definitions,
        } => export(&store, &output, &name, version, refs, definitions),
        Action::Load { store, package } => {
            let engine = Engine::new(Store::open(&store)?)?;
            let loaded = engine.load(&read_package(&read(&package)?)?)?;
            let mut lines: Vec<String> = loaded.manifests.iter().map(describe).collect();
            lines.extend(
                loaded.kept.refs.iter().map(|reference| {
                    format!("kept ref {reference}: the store points it elsewhere")
                }),
            );
            lines.extend(
                loaded
                    .kept
                    .definitions
                    .iter()
                    .map(|definition| format!("kept {definition}: the store defines it otherwise")),
            );
            Ok(lines.join("\n"))
        }
        Action::Samples { directory } => {
            fs::create_dir_all(&directory).map_err(|source| CliError::Write {
                path: directory.clone(),
                source,
            })?;
            let version: Version = env!("CARGO_PKG_VERSION")
                .parse()
                .expect("the crate's version is a version");
            let mut lines = Vec::new();
            for (name, write_sample) in axionomy_problems::packages() {
                let engine = Engine::new(Store::memory()?)?;
                write_sample(&engine)?;
                let contents = engine.named_under(&name)?;
                let output = directory.join(format!("{name}.zip"));
                lines.push(written(&engine, &name, version, contents, &output)?);
            }
            Ok(lines.join("\n"))
        }
        Action::Inspect { package } => {
            let engine = Engine::new(read_package(&read(&package)?)?)?;
            let reader = engine.store().read()?;
            let mut lines = Vec::new();
            for manifest in engine.manifests()? {
                lines.push(describe(&manifest));
                for reference in &manifest.refs {
                    let target = match reader.reference(reference)? {
                        Some(Target::Step(point)) => {
                            format!("step {} of {}", point.step, point.run)
                        }
                        Some(Target::Run(run)) => format!("{run}"),
                        Some(Target::Group(group)) => format!("{group}"),
                        None => "nothing".to_owned(),
                    };
                    lines.push(format!("  ref {reference} → {target}"));
                }
                lines.extend(
                    manifest
                        .definitions
                        .iter()
                        .map(|definition| format!("  {definition}")),
                );
            }
            Ok(lines.join("\n"))
        }
    }
}

fn export(
    store: &PathBuf,
    output: &PathBuf,
    name: &PackagePath,
    version: Version,
    refs: Vec<RefPath>,
    definitions: Vec<DefinitionPath>,
) -> Result<String, CliError> {
    let engine = Engine::new(Store::open(store)?)?;
    let contents = if refs.is_empty() && definitions.is_empty() {
        engine.named_under(name)?
    } else {
        Contents { refs, definitions }
    };
    written(&engine, name, version, contents, output)
}

/// Writes `engine`'s package of `contents` to `output`, a new file, zipped
/// when it ends in `.zip`, and says what it wrote.
fn written(
    engine: &Engine,
    name: &PackagePath,
    version: Version,
    contents: Contents,
    output: &PathBuf,
) -> Result<String, CliError> {
    if output.exists() {
        return Err(CliError::Exists(output.clone()));
    }
    let zip = output
        .extension()
        .is_some_and(|extension| extension == "zip");
    let file = if zip {
        output.with_extension("redb.partial")
    } else {
        output.clone()
    };
    let manifest = engine.export(name, version, contents, &Store::open(&file)?)?;
    Store::compact(&file)?;
    if zip {
        let bytes = read(&file)?;
        write(output, &zipped(&bytes)?)?;
        fs::remove_file(&file).map_err(|source| CliError::Write {
            path: file.clone(),
            source,
        })?;
    }
    let size = fs::metadata(output)
        .map_err(|source| CliError::Write {
            path: output.clone(),
            source,
        })?
        .len();
    Ok(format!(
        "{}\nwrote {} ({size} bytes)",
        describe(&manifest),
        output.display()
    ))
}

fn describe(manifest: &Manifest) -> String {
    format!(
        "{} {} · format {} · script host API {} · {}, {}",
        manifest.name,
        manifest.version,
        manifest.format,
        manifest.host_api,
        counted(manifest.refs.len(), "ref"),
        counted(manifest.definitions.len(), "definition")
    )
}

fn counted(count: usize, thing: &str) -> String {
    if count == 1 {
        format!("1 {thing}")
    } else {
        format!("{count} {thing}s")
    }
}

fn read(path: &PathBuf) -> Result<Vec<u8>, CliError> {
    fs::read(path).map_err(|source| CliError::Read {
        path: path.clone(),
        source,
    })
}

fn write(path: &PathBuf, bytes: &[u8]) -> Result<(), CliError> {
    fs::write(path, bytes).map_err(|source| CliError::Write {
        path: path.clone(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use crate::run_from;
    use axionomy_engine::{Engine, ScriptCall, Strategy};
    use axionomy_problems::registry;
    use axionomy_store::Store;

    /// A scratch directory, removed when the test ends.
    struct Scratch(std::path::PathBuf);

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn printed(arguments: &[&str]) -> String {
        run_from(registry(), arguments.iter().copied())
            .unwrap()
            .expect("a package command prints")
    }

    #[test]
    fn a_package_is_exported_inspected_and_loaded() {
        let scratch = Scratch(
            std::env::temp_dir().join(format!("axionomy-cli-package-{}", std::process::id())),
        );
        std::fs::create_dir_all(&scratch.0).unwrap();
        let file = |name: &str| scratch.0.join(name).display().to_string();
        {
            let engine = Engine::new(Store::open(file("work.redb")).unwrap()).unwrap();
            let economy = axionomy::paths::EconomyBuilder::new()
                .account("bank".parse().unwrap(), axionomy::basket([]).into())
                .build()
                .unwrap();
            let start = engine.author(economy).unwrap();
            engine.refer(&"walk/start".parse().unwrap(), start).unwrap();
            engine.refer(&"elsewhere".parse().unwrap(), start).unwrap();
            engine
                .keep_script(&"walk".parse().unwrap(), "fn stay(state, random) { () }")
                .unwrap();
            engine
                .keep_goal(
                    &"walk/never".parse().unwrap(),
                    &axionomy::paths::Goal::new(),
                )
                .unwrap();
            engine
                .keep_strategy(
                    &"walk/stay".parse().unwrap(),
                    &Strategy::Policy {
                        policy: ScriptCall::new("walk".parse().unwrap(), "stay".parse().unwrap()),
                        goal: "walk/never".parse().unwrap(),
                        horizon: 1,
                    },
                )
                .unwrap();
        }
        let exported = printed(&[
            "axionomy",
            "package",
            "export",
            &file("work.redb"),
            &file("walk.zip"),
            "--name",
            "walk",
            "--version",
            "0.1.0",
        ]);
        // Everything under `walk`, and the script its strategy calls.
        assert!(
            exported
                .starts_with("walk 0.1.0 · format 1 · script host API 1 · 1 ref, 3 definitions"),
            "{exported}"
        );
        assert!(!std::path::Path::new(&file("walk.redb.partial")).exists());
        let inspected = printed(&["axionomy", "package", "inspect", &file("walk.zip")]);
        assert!(
            inspected.contains("  ref walk/start → step 0 of run-"),
            "{inspected}"
        );
        assert!(inspected.contains("  strategy/walk/stay"), "{inspected}");
        let loaded = printed(&[
            "axionomy",
            "package",
            "load",
            &file("fresh.redb"),
            &file("walk.zip"),
        ]);
        assert!(loaded.starts_with("walk 0.1.0"), "{loaded}");
        let fresh = Engine::new(Store::open(file("fresh.redb")).unwrap()).unwrap();
        assert!(fresh.referred(&"walk/start".parse().unwrap()).is_ok());
        assert!(fresh.referred(&"elsewhere".parse().unwrap()).is_err());
        // A package is written to a new file only.
        assert!(
            run_from(
                registry(),
                [
                    "axionomy",
                    "package",
                    "export",
                    &file("work.redb"),
                    &file("walk.zip"),
                    "--name",
                    "walk",
                    "--version",
                    "0.1.0",
                ],
            )
            .is_err()
        );
    }
}
