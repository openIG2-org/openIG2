//! The player's own files: settings, save slots, replays, the statistics and fleet groups beside them. Natively
//! `std::fs`; in the browser build (ADR 0047 part 3), where `std::fs` is unsupported, a store in memory that the page
//! fills from its private storage before the start and that hands every change back to the page ([`set_sink`]).

use std::io;
use std::path::{Path, PathBuf};

#[cfg(not(target_arch = "wasm32"))]
pub use std::fs::{create_dir_all, read, read_to_string, remove_file, rename, write};

/// Whether `path` is a file.
pub fn exists(path: impl AsRef<Path>) -> bool {
    #[cfg(not(target_arch = "wasm32"))]
    return path.as_ref().is_file();
    #[cfg(target_arch = "wasm32")]
    return mem::FILES.lock().unwrap().contains_key(path.as_ref());
}

/// The paths of the files directly in `dir`.
pub fn list(dir: impl AsRef<Path>) -> io::Result<Vec<PathBuf>> {
    #[cfg(not(target_arch = "wasm32"))]
    return Ok(std::fs::read_dir(dir)?.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.is_file()).collect());
    #[cfg(target_arch = "wasm32")]
    return Ok(mem::FILES.lock().unwrap().keys().filter(|p| p.parent() == Some(dir.as_ref())).cloned().collect());
}

/// The first `n` bytes of `path` (fewer when it is shorter).
pub fn read_head(path: impl AsRef<Path>, n: usize) -> io::Result<Vec<u8>> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        use io::Read;
        let mut b = Vec::with_capacity(n);
        std::fs::File::open(path)?.take(n as u64).read_to_end(&mut b)?;
        Ok(b)
    }
    #[cfg(target_arch = "wasm32")]
    {
        let mut b = read(path)?;
        b.truncate(n);
        Ok(b)
    }
}

/// A new file to write bit by bit (a replay): on the page it is stored when dropped.
pub fn create(path: impl AsRef<Path>) -> io::Result<Box<dyn io::Write + Send>> {
    #[cfg(not(target_arch = "wasm32"))]
    return Ok(Box::new(std::fs::File::create(path)?));
    #[cfg(target_arch = "wasm32")]
    return Ok(Box::new(mem::Writer(path.as_ref().to_path_buf(), Vec::new())));
}

#[cfg(target_arch = "wasm32")]
pub use mem::*;

#[cfg(target_arch = "wasm32")]
mod mem {
    use std::collections::BTreeMap;
    use std::io;
    use std::path::{Path, PathBuf};
    use std::sync::{Mutex, OnceLock};

    pub(super) static FILES: Mutex<BTreeMap<PathBuf, Vec<u8>>> = Mutex::new(BTreeMap::new());
    /// Where the page's copy of every change goes: the new bytes, or `None` for a removed file.
    pub type Sink = fn(&Path, Option<&[u8]>);
    static SINK: OnceLock<Sink> = OnceLock::new();

    pub fn set_sink(sink: Sink) {
        let _ = SINK.set(sink);
    }

    /// A file the page had stored, before the start.
    pub fn insert(path: PathBuf, bytes: Vec<u8>) {
        FILES.lock().unwrap().insert(path, bytes);
    }

    fn sink(path: &Path, bytes: Option<&[u8]>) {
        if let Some(s) = SINK.get() {
            s(path, bytes);
        }
    }

    pub fn read(path: impl AsRef<Path>) -> io::Result<Vec<u8>> {
        FILES.lock().unwrap().get(path.as_ref()).cloned().ok_or_else(|| io::ErrorKind::NotFound.into())
    }

    pub fn read_to_string(path: impl AsRef<Path>) -> io::Result<String> {
        String::from_utf8(read(path)?).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    pub fn write(path: impl AsRef<Path>, bytes: impl AsRef<[u8]>) -> io::Result<()> {
        let (path, bytes) = (path.as_ref(), bytes.as_ref());
        FILES.lock().unwrap().insert(path.to_path_buf(), bytes.to_vec());
        sink(path, Some(bytes));
        Ok(())
    }

    pub fn remove_file(path: impl AsRef<Path>) -> io::Result<()> {
        FILES.lock().unwrap().remove(path.as_ref()).ok_or(io::ErrorKind::NotFound)?;
        sink(path.as_ref(), None);
        Ok(())
    }

    pub fn rename(from: impl AsRef<Path>, to: impl AsRef<Path>) -> io::Result<()> {
        let bytes = read(&from)?;
        write(to, bytes)?;
        remove_file(from)
    }

    /// Folders are implied by their files' paths.
    pub fn create_dir_all(_: impl AsRef<Path>) -> io::Result<()> {
        Ok(())
    }

    pub(super) struct Writer(pub PathBuf, pub Vec<u8>);

    impl io::Write for Writer {
        fn write(&mut self, b: &[u8]) -> io::Result<usize> {
            self.1.extend_from_slice(b);
            Ok(b.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    impl Drop for Writer {
        fn drop(&mut self) {
            let _ = write(&self.0, std::mem::take(&mut self.1));
        }
    }
}
