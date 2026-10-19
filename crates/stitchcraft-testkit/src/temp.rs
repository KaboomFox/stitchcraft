//! Folders of a test's own under the system's temporary folder, removed when the test is done with them.
//!
//! The command line's tests write files and read them back. Each test binary used to make one folder for a
//! run and leave it behind: 14 folders and about 65 MB a run, most of it a bundle of 64 MiB that a replay
//! must read whole. Mutation testing runs the tests hundreds of times, and 5.7 GB had built up. A
//! [`TempDir`] is removed, with everything in it, when it is dropped at the end of the test that made it,
//! whether the test passed or not. Tests run at once in one process, so a folder is named after the
//! process, a count and the name a test gives it.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

/// How many folders this process has made: the count that tells its folders apart.
static MADE: AtomicUsize = AtomicUsize::new(0);

/// A folder of one test's own, removed with everything in it when dropped.
#[derive(Debug)]
pub struct TempDir {
    path: PathBuf,
}

impl TempDir {
    /// A new, empty folder named `stitchcraft-NAME-PROCESS-COUNT` under the system's temporary folder.
    // A test's folder is on the file system on purpose, which clippy.toml keeps out of library code.
    #[allow(clippy::disallowed_methods)]
    pub fn new(name: &str) -> TempDir {
        let count = MADE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("stitchcraft-{name}-{}-{count}", std::process::id()));
        // An earlier process with the same id may have left one: start empty.
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        TempDir { path }
    }

    /// The folder itself.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The path of `name` in the folder.
    pub fn join(&self, name: impl AsRef<Path>) -> PathBuf {
        self.path.join(name)
    }
}

impl AsRef<Path> for TempDir {
    fn as_ref(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    #[allow(clippy::disallowed_methods)]
    fn drop(&mut self) {
        // What a test leaves behind is clutter, not a failure: a folder that will not go stays.
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(clippy::disallowed_methods)]
    fn a_folder_is_new_and_its_own_and_goes_with_everything_in_it() {
        let (one, two) = (TempDir::new("temp"), TempDir::new("temp"));
        assert_ne!(one.path(), two.path(), "folders made at once are apart");
        std::fs::create_dir_all(one.join("inner")).unwrap();
        std::fs::write(one.join("inner/file.txt"), b"x").unwrap();
        let path = one.path().to_path_buf();
        drop(one);
        assert!(!path.exists(), "removed with what is in it");
        assert!(two.path().is_dir(), "the other stays until it is dropped");
    }
}
