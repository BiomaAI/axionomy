//! Shipped package sources and their compiled store representation.

/// The UTF-8 source that a shipped sample builds from, also available to
/// binaries installed outside a checkout and to the Studio worker.
pub fn source(package: &axionomy_engine::PackagePath) -> axionomy_engine::source::SourceFiles {
    crate::fixtures::GALLERY
        .iter()
        .filter(|(name, _, _)| *name == package.as_str())
        .map(|(_, file, text)| {
            (
                file.parse().expect("a gallery source file"),
                (*text).to_owned(),
            )
        })
        .collect()
}

pub(crate) fn write(
    engine: &axionomy_engine::Engine,
    name: axionomy_engine::PackagePath,
) -> Result<(), axionomy_engine::EngineError> {
    let built = axionomy_engine::source::build(&source(&name))
        .map_err(|e| axionomy_engine::PackageError::Source(Box::new(e)))?;
    let package = axionomy_store::Store::memory()?;
    built.export(&package)?;
    engine.load(&package)?;
    Ok(())
}
