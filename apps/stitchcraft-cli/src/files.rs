//! Writing files so that a reader never sees half of one.
//!
//! A machine file copied to a USB stick while it is being written, or a write interrupted by a full disk,
//! must not leave a truncated file under the real name: a machine may accept it and sew part of a design.
//! So files are written next to their destination under a temporary name and renamed into place, which
//! replaces the destination in one step.

use std::io;
use std::path::Path;

/// Writes `bytes` to `path`, replacing it in one step.
pub fn write_atomically(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut temporary = path.as_os_str().to_owned();
    temporary.push(".partial");
    let temporary = Path::new(&temporary);
    std::fs::write(temporary, bytes).and_then(|()| std::fs::rename(temporary, path)).inspect_err(|_| {
        let _ = std::fs::remove_file(temporary);
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_the_destination_and_leaves_no_partial_file() {
        let dir = std::env::temp_dir().join(format!("stitchcraft-files-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("a.pes");
        std::fs::write(&path, b"old").unwrap();
        write_atomically(&path, b"new").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"new");
        assert!(!dir.join("a.pes.partial").exists());
        assert!(write_atomically(&dir.join("missing-dir/a.pes"), b"x").is_err());
    }
}
