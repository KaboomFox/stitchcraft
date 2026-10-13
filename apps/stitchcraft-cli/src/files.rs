//! Reading files no larger than a cap, and writing them so that a reader never sees half of one.
//!
//! A machine file copied to a USB stick while it is being written, or a write interrupted by a full disk,
//! must not leave a truncated file under the real name: a machine may accept it and sew part of a design.
//! So files are written next to their destination under a temporary name and renamed into place, which
//! replaces the destination in one step. A command that writes several files writes them all to their
//! temporary names first, and renames none into place unless every one was written (REQ-CLI-006).
//!
//! Reads stop past [`MAX_READ`] bytes, whatever the file says of its size: a pipe or a device says nothing,
//! and `/dev/zero` never ends (REQ-CLI-005).

use std::ffi::OsString;
use std::io::{self, Read as _};
use std::path::{Path, PathBuf};

/// The largest file `stitch` reads: far beyond any embroidery file (a 2,000,000-stitch design is under
/// 10 MB), small enough that a wrong path cannot make it read a disk image.
pub const MAX_READ: u64 = 64 * 1024 * 1024;

/// Reads `path`, refusing files larger than [`MAX_READ`].
pub fn read_capped(path: &Path) -> io::Result<Vec<u8>> {
    let too_large = |size: String| io::Error::new(io::ErrorKind::InvalidData, format!("the file is {size} bytes; stitch reads at most {MAX_READ}"));
    let size = std::fs::metadata(path)?.len();
    if size > MAX_READ {
        return Err(too_large(size.to_string()));
    }
    let mut bytes = Vec::new();
    std::fs::File::open(path)?.take(MAX_READ + 1).read_to_end(&mut bytes)?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_READ {
        return Err(too_large(format!("more than {MAX_READ}")));
    }
    Ok(bytes)
}

/// Writes `bytes` to `path`, replacing it in one step.
pub fn write_atomically(path: &Path, bytes: &[u8]) -> io::Result<()> {
    write_all(&[(path, bytes)]).map_err(|unwritten| unwritten.error)
}

/// Why [`write_all`] did not write every file.
#[derive(Debug)]
pub struct Unwritten {
    /// The file that could not be written.
    pub path: PathBuf,
    /// Why.
    pub error: io::Error,
    /// The files put in place before it, if renaming one failed: none when writing one did.
    pub written: Vec<PathBuf>,
}

/// Writes each of `files`, a path and its bytes, every one whole or none: each goes to a temporary file next
/// to it, and they are renamed into place once every one is written.
pub fn write_all(files: &[(&Path, &[u8])]) -> Result<(), Unwritten> {
    let mut staged: Vec<(OsString, &Path)> = Vec::new();
    let unwritten = |path: &Path, error, written| Unwritten { path: path.to_path_buf(), error, written };
    for &(path, bytes) in files {
        let mut temporary = path.as_os_str().to_owned();
        temporary.push(".partial");
        if let Err(error) = std::fs::write(&temporary, bytes) {
            let _ = std::fs::remove_file(&temporary);
            for (left, _) in &staged {
                let _ = std::fs::remove_file(left);
            }
            return Err(unwritten(path, error, Vec::new()));
        }
        staged.push((temporary, path));
    }
    for (i, (temporary, path)) in staged.iter().enumerate() {
        if let Err(error) = std::fs::rename(temporary, path) {
            for (left, _) in staged.iter().skip(i) {
                let _ = std::fs::remove_file(left);
            }
            return Err(unwritten(path, error, staged.iter().take(i).map(|(_, p)| p.to_path_buf()).collect()));
        }
    }
    Ok(())
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

    /// REQ-CLI-006: every file is written, or none is.
    #[test]
    fn req_cli_006_files_are_written_all_or_none() {
        let dir = std::env::temp_dir().join(format!("stitchcraft-files-all-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (a, b) = (dir.join("a.pes"), dir.join("a.png"));
        write_all(&[(&a, b"a"), (&b, b"b")]).unwrap();
        assert_eq!((std::fs::read(&a).unwrap(), std::fs::read(&b).unwrap()), (b"a".to_vec(), b"b".to_vec()));
        // One in a folder that does not exist: the others are not put in place, and no partial file stays.
        let (c, missing) = (dir.join("c.pes"), dir.join("missing").join("c.png"));
        let failed = write_all(&[(&c, b"c"), (&missing, b"x")]).unwrap_err();
        assert_eq!((&failed.path, &failed.written), (&missing, &Vec::new()));
        assert!(!c.exists() && !dir.join("c.pes.partial").exists());
        // One that cannot be renamed into place, over a folder: the files before it stay, and are named.
        let (d, folder) = (dir.join("d.pes"), dir.join("folder"));
        std::fs::create_dir_all(folder.join("inside")).unwrap();
        let failed = write_all(&[(&d, b"d"), (&folder, b"x"), (&dir.join("e.pes"), b"e")]).unwrap_err();
        assert_eq!((&failed.path, &failed.written), (&folder, &vec![d.clone()]));
        assert!(d.exists() && !dir.join("folder.partial").exists() && !dir.join("e.pes").exists() && !dir.join("e.pes.partial").exists());
    }

    /// REQ-CLI-005: a file of the cap's size is read, and a larger one refused by the size it says it has.
    #[test]
    fn req_cli_005_files_up_to_the_cap_are_read() {
        let dir = std::env::temp_dir().join(format!("stitchcraft-cap-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (at, past) = (dir.join("at.svg"), dir.join("past.svg"));
        std::fs::File::create(&at).unwrap().set_len(MAX_READ).unwrap();
        std::fs::File::create(&past).unwrap().set_len(MAX_READ + 1).unwrap();
        assert_eq!(read_capped(&at).map(|bytes| u64::try_from(bytes.len()).unwrap()).unwrap(), MAX_READ);
        assert_eq!(read_capped(&past).unwrap_err().to_string(), format!("the file is {} bytes; stitch reads at most {MAX_READ}", MAX_READ + 1));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// REQ-CLI-005: a pipe or a device says nothing of its size, so the cap counts what is read. A pipe
    /// that holds one byte more than the cap is refused. `mkfifo` makes the pipe, on Unix.
    #[cfg(unix)]
    #[test]
    fn req_cli_005_a_pipe_is_read_no_further_than_the_cap() {
        use std::io::Write as _;
        let dir = std::env::temp_dir().join(format!("stitchcraft-pipe-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let pipe = dir.join("design.svg");
        let _ = std::fs::remove_file(&pipe);
        assert!(std::process::Command::new("mkfifo").arg(&pipe).status().unwrap().success());
        let writer = {
            let pipe = pipe.clone();
            std::thread::spawn(move || {
                let mut to = std::fs::OpenOptions::new().write(true).open(&pipe).unwrap();
                let chunk = vec![b' '; 1 << 20];
                let mut left = MAX_READ + 1;
                while left > 0 {
                    let n = left.min(1 << 20);
                    if to.write_all(&chunk[..usize::try_from(n).unwrap()]).is_err() {
                        break;
                    }
                    left -= n;
                }
            })
        };
        let read = read_capped(&pipe);
        writer.join().unwrap();
        std::fs::remove_file(&pipe).unwrap();
        assert_eq!(
            read.map(|bytes| bytes.len()).map_err(|e| e.to_string()),
            Err(format!("the file is more than {MAX_READ} bytes; stitch reads at most {MAX_READ}"))
        );
    }
}
