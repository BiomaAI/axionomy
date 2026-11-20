//! A portable package image. The same in-memory backend works in a browser worker
//! and on a native host. Finish closing redb before reading its file.
use super::BuiltPackage;
use crate::{EngineError, PackageError};
use axionomy_store::Store;
use redb::{StorageBackend, backends::InMemoryBackend};
use std::{io, sync::Arc};

#[derive(Debug, Clone, Default)]
struct MemoryFile(Arc<InMemoryBackend>);

impl StorageBackend for MemoryFile {
    fn len(&self) -> io::Result<u64> {
        self.0.len()
    }

    fn read(&self, offset: u64, out: &mut [u8]) -> io::Result<()> {
        self.0.read(offset, out)
    }

    fn write(&self, offset: u64, data: &[u8]) -> io::Result<()> {
        self.0.write(offset, data)
    }

    fn set_len(&self, len: u64) -> io::Result<()> {
        self.0.set_len(len)
    }

    fn sync_data(&self) -> io::Result<()> {
        self.0.sync_data()
    }
}

impl BuiltPackage {
    /// Export a loadable package archive without a filesystem.
    pub fn archive(&self) -> Result<Vec<u8>, EngineError> {
        let file = MemoryFile::default();
        let store = Store::on(file.clone())?;
        self.export(&store)?;
        drop(store);
        let archive_error = |error: io::Error| PackageError::Archive(error.to_string());
        let length = usize::try_from(file.len().map_err(archive_error)?)
            .map_err(|e| PackageError::Archive(e.to_string()))?;
        let mut bytes = vec![0; length];
        file.read(0, &mut bytes).map_err(archive_error)?;
        crate::zipped(&bytes)
    }
}
