//! A redb backend over a file of the browser's origin private file system
//! (D-047), so the Worker's store stays across reloads.
//!
//! A `FileSystemSyncAccessHandle` reads and writes at an offset, truncates,
//! flushes and gives its size synchronously, which is what redb asks of a
//! backend; it exists only in dedicated Workers, and only one is open on a
//! file at a time. The Worker opens it, since opening is asynchronous, and
//! hands it over; redb closes it when the store closes.

use redb::StorageBackend;
use std::io;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{FileSystemReadWriteOptions, FileSystemSyncAccessHandle};

/// The largest offset a JavaScript number carries exactly: 2^53.
const EXACT: u64 = 1 << 53;

#[derive(Debug)]
pub(crate) struct Opfs {
    file: FileSystemSyncAccessHandle,
}

impl Opfs {
    pub(crate) fn new(file: FileSystemSyncAccessHandle) -> Self {
        Self { file }
    }
}

// SAFETY: without the atomics feature a wasm32 module has one thread, so the
// handle, a JavaScript object, is never touched from another; redb asks
// every backend to be `Send` and `Sync` for the threads it would have
// elsewhere.
#[cfg(not(target_feature = "atomics"))]
unsafe impl Send for Opfs {}
#[cfg(not(target_feature = "atomics"))]
unsafe impl Sync for Opfs {}

impl StorageBackend for Opfs {
    fn len(&self) -> io::Result<u64> {
        let size = self.file.get_size().map_err(failed)?;
        // A size is a whole number of bytes below 2^53.
        Ok(size as u64)
    }

    fn read(&self, offset: u64, out: &mut [u8]) -> io::Result<()> {
        let read = self
            .file
            .read_with_u8_array_and_options(out, &at(offset)?)
            .map_err(failed)?;
        whole(read, out.len(), "read past the end of the store")
    }

    fn set_len(&self, len: u64) -> io::Result<()> {
        self.file.truncate_with_f64(exact(len)?).map_err(failed)
    }

    fn sync_data(&self) -> io::Result<()> {
        self.file.flush().map_err(failed)
    }

    fn write(&self, offset: u64, data: &[u8]) -> io::Result<()> {
        let written = self
            .file
            .write_with_u8_array_and_options(data, &at(offset)?)
            .map_err(failed)?;
        whole(written, data.len(), "wrote less than asked")
    }

    fn close(&self) -> io::Result<()> {
        self.file.close();
        Ok(())
    }
}

/// Options reading or writing at `offset`.
fn at(offset: u64) -> io::Result<FileSystemReadWriteOptions> {
    let options = FileSystemReadWriteOptions::new();
    options.set_at(exact(offset)?);
    Ok(options)
}

/// `value` as a JavaScript number, refused where it would round.
fn exact(value: u64) -> io::Result<f64> {
    if value > EXACT {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "the origin private file system addresses at most 2^53 bytes",
        ));
    }
    Ok(value as f64)
}

/// Whether a read or write moved all `wanted` bytes.
fn whole(moved: f64, wanted: usize, short: &str) -> io::Result<()> {
    // Both are whole numbers of bytes below 2^53.
    if moved as u64 == wanted as u64 {
        Ok(())
    } else {
        Err(io::Error::new(io::ErrorKind::UnexpectedEof, short))
    }
}

/// What the browser said went wrong.
fn failed(error: JsValue) -> io::Error {
    let message = match error.dyn_ref::<js_sys::Error>() {
        Some(error) => String::from(error.message()),
        None => format!("{error:?}"),
    };
    io::Error::other(message)
}
