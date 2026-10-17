//! Reading files no larger than a cap, and writing them so that a reader never sees half of one.
//!
//! A machine file copied to a USB stick while it is being written, or a write interrupted by a full disk,
//! must not leave a truncated file under the real name: a machine may accept it and sew part of a design.
//! So files are written next to their destination under a temporary name and renamed into place, which
//! replaces the destination in one step. A command that writes several files writes them all to their
//! temporary names first, and renames none into place unless every one was written (REQ-CLI-006).
//!
//! Each temporary name is the program's own: it is created new, so a file or a link already there under
//! that name is neither written through nor replaced, and a link planted in a shared folder cannot steer the
//! write to another file. Each is synced to the disk before it is renamed, and the folder after, so a
//! stick pulled out once `stitch` is done holds the whole file.
//!
//! Reads stop past a cap, whatever the file says of its size: a pipe or a device says nothing, and
//! `/dev/zero` never ends (REQ-CLI-005). The cap is [`MAX_READ`] bytes, and more for a bug-report bundle,
//! which can hold a design that large twice over, in hexadecimal.

use std::ffi::OsString;
use std::fs::{File, OpenOptions};
use std::io::{self, Read as _, Write as _};
use std::path::{Path, PathBuf};

/// The largest file `stitch` reads: far beyond any embroidery file (a 2,000,000-stitch design is under
/// 10 MB), small enough that a wrong path cannot make it read a disk image.
pub const MAX_READ: u64 = 64 * 1024 * 1024;

/// Reads `path`, refusing files larger than [`MAX_READ`].
pub fn read_capped(path: &Path) -> io::Result<Vec<u8>> {
    read_at_most(path, MAX_READ)
}

/// Reads `path`, refusing files larger than `cap` bytes.
pub fn read_at_most(path: &Path, cap: u64) -> io::Result<Vec<u8>> {
    let too_large = |size: String| io::Error::new(io::ErrorKind::InvalidData, format!("the file is {size} bytes; stitch reads at most {cap}"));
    let size = std::fs::metadata(path)?.len();
    if size > cap {
        return Err(too_large(size.to_string()));
    }
    let mut bytes = Vec::new();
    std::fs::File::open(path)?.take(cap.saturating_add(1)).read_to_end(&mut bytes)?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > cap {
        return Err(too_large(format!("more than {cap}")));
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

/// How many names [`staged`] tries before it gives up: as many temporary files as it would take a crashed
/// run after crashed run to leave behind.
const NAMES: u32 = 100;

/// Writes each of `files`, a path and its bytes, every one whole or none: each goes to a temporary file next
/// to it, and they are renamed into place once every one is written.
pub fn write_all(files: &[(&Path, &[u8])]) -> Result<(), Unwritten> {
    let mut staged_files: Vec<(OsString, &Path)> = Vec::new();
    let unwritten = |path: &Path, error, written| Unwritten { path: path.to_path_buf(), error, written };
    for &(path, bytes) in files {
        match staged(path, bytes) {
            Ok(temporary) => staged_files.push((temporary, path)),
            Err(error) => {
                for (left, _) in &staged_files {
                    let _ = std::fs::remove_file(left);
                }
                return Err(unwritten(path, error, Vec::new()));
            }
        }
    }
    for (i, (temporary, path)) in staged_files.iter().enumerate() {
        if let Err(error) = std::fs::rename(temporary, path) {
            for (left, _) in staged_files.iter().skip(i) {
                let _ = std::fs::remove_file(left);
            }
            return Err(unwritten(path, error, staged_files.iter().take(i).map(|(_, p)| p.to_path_buf()).collect()));
        }
    }
    for (_, path) in &staged_files {
        sync_folder(path);
    }
    Ok(())
}

/// `bytes` written, and synced, to a new temporary file next to `path`, named `path` followed by
/// `.<process>-<n>.partial`; its name. Nothing already there is opened: a name in use is passed over.
fn staged(path: &Path, bytes: &[u8]) -> io::Result<OsString> {
    let mut last = io::Error::new(io::ErrorKind::AlreadyExists, format!("{NAMES} temporary names next to it are in use"));
    // WASI gives a program no process id, and its `std::process::id` panics, so there the names count from
    // 0: a name another run is using is passed over all the same. `cargo xtask wasm --goldens` runs `stitch`
    // as WebAssembly.
    #[cfg(target_os = "wasi")]
    let process = 0;
    #[cfg(not(target_os = "wasi"))]
    let process = std::process::id();
    for n in 0..NAMES {
        let mut temporary = path.as_os_str().to_owned();
        temporary.push(format!(".{process}-{n}.partial"));
        let mut file = match OpenOptions::new().write(true).create_new(true).open(&temporary) {
            Ok(file) => file,
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                last = e;
                continue;
            }
            Err(e) => return Err(e),
        };
        return match file.write_all(bytes).and_then(|()| file.sync_all()) {
            Ok(()) => Ok(temporary),
            Err(e) => {
                drop(file);
                let _ = std::fs::remove_file(&temporary);
                Err(e)
            }
        };
    }
    Err(last)
}

/// Syncs the folder `path` is in ([`folder`]), so that the rename into place is on the disk too. Only some
/// platforms open a folder as a file (Unix does, Windows does not), and a folder that cannot be synced
/// leaves the file written all the same.
fn sync_folder(path: &Path) {
    if let Ok(folder) = File::open(folder(path)) {
        let _ = folder.sync_all();
    }
}

/// The folder `path` is in: the current one for a bare file name.
fn folder(path: &Path) -> &Path {
    path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."))
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
        assert_eq!(partial(&dir), Vec::<String>::new());
        assert!(write_atomically(&dir.join("missing-dir/a.pes"), b"x").is_err());
    }

    /// The temporary files left in `dir`.
    fn partial(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .filter_map(|entry| entry.ok()?.file_name().into_string().ok())
            .filter(|name| name.ends_with(".partial"))
            .collect();
        names.sort();
        names
    }

    /// REQ-CLI-006: a temporary name is created new, so a link planted under it, as anyone who can write in a
    /// shared folder could plant one, is not written through, and a file left there is not replaced.
    #[cfg(unix)]
    #[test]
    fn req_cli_006_a_link_planted_where_a_temporary_file_goes_is_not_followed() {
        let dir = std::env::temp_dir().join(format!("stitchcraft-files-link-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let (victim, out) = (dir.join("victim.txt"), dir.join("out.pes"));
        std::fs::write(&victim, b"precious notes").unwrap();
        // Links where the temporary file went once, and where it goes first now.
        let planted = [dir.join("out.pes.partial"), dir.join(format!("out.pes.{}-0.partial", std::process::id()))];
        for link in &planted {
            std::os::unix::fs::symlink(&victim, link).unwrap();
        }
        write_atomically(&out, b"#PES0001").unwrap();
        assert_eq!((std::fs::read(&victim).unwrap(), std::fs::read(&out).unwrap()), (b"precious notes".to_vec(), b"#PES0001".to_vec()));
        assert!(std::fs::symlink_metadata(&out).unwrap().file_type().is_file(), "the file itself, not a link");
        // The planted links are left as they were, and no temporary file of the write stays.
        let mut names: Vec<String> = planted.iter().map(|link| link.file_name().unwrap().to_str().unwrap().to_string()).collect();
        names.sort();
        assert_eq!(partial(&dir), names);
        assert!(planted.iter().all(|link| std::fs::symlink_metadata(link).unwrap().file_type().is_symlink()));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_folder_synced_is_the_one_the_file_is_in() {
        assert_eq!(
            [folder(Path::new("x.pes")), folder(Path::new("out/x.pes")), folder(Path::new("/x.pes"))],
            [Path::new("."), Path::new("out"), Path::new("/")]
        );
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
        assert!(!c.exists() && partial(&dir).is_empty());
        // One that cannot be renamed into place, over a folder: the files before it stay, and are named.
        let (d, folder) = (dir.join("d.pes"), dir.join("folder"));
        std::fs::create_dir_all(folder.join("inside")).unwrap();
        let failed = write_all(&[(&d, b"d"), (&folder, b"x"), (&dir.join("e.pes"), b"e")]).unwrap_err();
        assert_eq!((&failed.path, &failed.written), (&folder, &vec![d.clone()]));
        assert!(d.exists() && !dir.join("e.pes").exists() && partial(&dir).is_empty());
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
