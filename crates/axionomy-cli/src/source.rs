use crate::CliError;
use axionomy_engine::source::{self, BuiltPackage, SourceFiles};
use std::{fs, path::Path};

pub(crate) fn build(directory: &Path) -> Result<BuiltPackage, CliError> {
    fn collect(root: &Path, dir: &Path, files: &mut SourceFiles) -> Result<(), CliError> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            let local = path.strip_prefix(root).expect("within source tree");
            if entry.file_name().to_string_lossy().starts_with('.')
                || (dir == root && entry.file_name() == "README.md")
            {
                continue;
            }
            let kind = entry.file_type()?;
            if kind.is_dir() {
                collect(root, &path, files)?;
            } else if kind.is_file() {
                let file = local
                    .iter()
                    .map(|s| s.to_string_lossy())
                    .collect::<Vec<_>>()
                    .join("/")
                    .parse()?;
                files.insert(
                    file,
                    fs::read_to_string(&path).map_err(|source| CliError::Read { path, source })?,
                );
            } else {
                return Err(std::io::Error::other(format!(
                    "{} is not a regular source file",
                    path.display()
                ))
                .into());
            }
        }
        Ok(())
    }
    let mut files = SourceFiles::new();
    collect(directory, directory, &mut files)?;
    Ok(source::build(&files)?)
}
