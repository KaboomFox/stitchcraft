//! Writing files so that a reader never sees half of one.
//!
//! A machine file copied to a USB stick while it is being written, or a write interrupted by a full disk,
//! must not leave a truncated file under the real name: a machine may accept it and sew part of a design.
//! So files are written next to their destination under a temporary name and renamed into place, which
//! replaces the destination in one step.

use std::io;
use std::path::Path;

/// The largest file `stitch` reads: far beyond any embroidery file (a 2,000,000-stitch design is under
/// 10 MB), small enough that a wrong path cannot make it read a disk image.
pub const MAX_READ: u64 = 64 * 1024 * 1024;

/// Reads `path`, refusing files larger than [`MAX_READ`].
pub fn read_capped(path: &Path) -> io::Result<Vec<u8>> {
    let size = std::fs::metadata(path)?.len();
    if size > MAX_READ {
        return Err(io::Error::new(io::ErrorKind::InvalidData, format!("the file is {size} bytes; stitch reads at most {MAX_READ}")));
    }
    std::fs::read(path)
}

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
