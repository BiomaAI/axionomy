//! Source submissions build in isolation. Installation publishes a validated
//! release through the same transaction as loading a package file.
use super::*;
use crate::messages::{PackageBuildRequest, PackageBuilt};
use axionomy_engine::{
    PackagePath,
    source::{self, SourceFiles},
};

pub(super) fn read(engine: &Engine, name: &PackagePath) -> Result<SourceFiles, ToolError> {
    engine.package_source(name).map_err(|e| match e {
        EngineError::Undefined(_) => ToolError::new(
            FailureCode::NotFound,
            format!("no source for package {name}"),
        ),
        e => ToolError::internal(e),
    })
}

pub(super) fn build(
    engine: &Engine,
    gate: &std::sync::Mutex<()>,
    request: PackageBuildRequest,
) -> Result<PackageBuilt, ToolError> {
    let baseline = request
        .base
        .as_ref()
        .map(|name| read(engine, name))
        .transpose()?;
    let mut files = baseline.clone().unwrap_or_default();
    for (file, source) in request.files {
        if let Some(text) = source {
            files.insert(file, text);
        } else {
            files.remove(&file);
        }
    }
    let built = source::build(&files)
        .map_err(|e| ToolError::new(FailureCode::Invalid, e.to_string()).with_details(e))?;
    let dir = tempfile::tempdir().map_err(ToolError::internal)?;
    let path = dir.path().join("package.redb");
    let store = axionomy_store::Store::open(&path).map_err(ToolError::internal)?;
    let manifest = built.export(&store).map_err(ToolError::internal)?;
    // Finish producing the downloadable artifact before changing the live store.
    drop(store);
    let archive = if request.export {
        use base64::Engine as _;
        axionomy_store::Store::compact(&path).map_err(ToolError::internal)?;
        let bytes = axionomy_engine::zipped(&std::fs::read(&path).map_err(ToolError::internal)?)
            .map_err(ToolError::internal)?;
        Some(base64::engine::general_purpose::STANDARD.encode(bytes))
    } else {
        None
    };
    if request.install {
        let _lock = gate.lock().map_err(ToolError::internal)?;
        if let Some(base) = request.base
            && Some(read(engine, &base)?) != baseline
        {
            return Err(ToolError::new(
                FailureCode::Conflict,
                "base package changed during the build",
            ));
        }
        if engine
            .manifests()
            .map_err(ToolError::internal)?
            .iter()
            .any(|m| m.name == manifest.name && m.version == manifest.version)
            && engine.package_source(&manifest.name).ok().as_ref() != Some(&files)
        {
            return Err(ToolError::new(
                FailureCode::Conflict,
                "this version is already installed with different contents; change the source version to install a new release",
            ));
        }
        engine
            .load(&axionomy_store::Store::open(&path).map_err(ToolError::internal)?)
            .map_err(ToolError::internal)?;
    }
    Ok(PackageBuilt {
        name: manifest.name,
        version: PackageVersion(manifest.version),
        refs: manifest.refs,
        definitions: manifest.definitions,
        tests: built.tests,
        installed: request.install,
        archive,
    })
}
