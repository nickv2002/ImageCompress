//! Undo for a finished job: before a source file is replaced or removed, the engine keeps a backup of it
//! (a hard link when the backup folder is on the same volume, otherwise a copy that keeps extended attributes).
//! The `Undo` record in the `finished` event is everything needed to put things back.
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Restore {
    /// Where the original file lived.
    pub path: PathBuf,
    /// The kept copy.
    pub backup: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Undo {
    /// The file the job wrote.
    pub output: PathBuf,
    pub out_size: u64,
    /// blake3 of the written bytes: undo refuses if the file was edited since.
    pub out_hash: String,
    /// Files to bring back (empty when the job only added a new file).
    pub restore: Vec<Restore>,
}

static COUNTER: AtomicU64 = AtomicU64::new(0);

pub fn hash_bytes(b: &[u8]) -> String {
    blake3::hash(b).to_hex().to_string()
}

fn hash_file(p: &Path) -> Result<String, String> {
    let mut h = blake3::Hasher::new();
    h.update_reader(fs::File::open(p).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    Ok(h.finalize().to_hex().to_string())
}

/// Copy keeping mode, times and extended attributes (Finder tags) where the platform can.
pub fn copy_file(src: &Path, dst: &Path) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let st = std::process::Command::new("/bin/cp").arg("-p").arg(src).arg(dst).output().map_err(|e| e.to_string())?;
        if !st.status.success() {
            return Err(String::from_utf8_lossy(&st.stderr).trim().to_string());
        }
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    {
        fs::copy(src, dst).map(|_| ()).map_err(|e| e.to_string())
    }
}

/// Keep `src` under `dir`: a hard link if possible (free, and the old inode survives a rename over `src`), else a copy.
pub fn backup(dir: &Path, src: &Path) -> Result<PathBuf, String> {
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let slot = dir.join(format!("{nanos:x}-{}", COUNTER.fetch_add(1, Ordering::Relaxed)));
    fs::create_dir_all(&slot).map_err(|e| format!("backup folder: {e}"))?;
    let dst = slot.join(src.file_name().unwrap_or_default());
    if fs::hard_link(src, &dst).is_err() {
        copy_file(src, &dst).map_err(|e| format!("backup copy: {e}"))?;
        let (a, b) = (fs::metadata(src).map(|m| m.len()), fs::metadata(&dst).map(|m| m.len()));
        if a.is_err() || a.as_ref().ok() != b.as_ref().ok() {
            let _ = fs::remove_file(&dst);
            return Err("backup copy came out a different size".into());
        }
    }
    Ok(dst)
}

/// Copy extended attributes (Finder tags, where-from, ...) from one file to another. Best effort.
#[cfg(target_os = "macos")]
pub fn copy_xattrs(src: &Path, dst: &Path) {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;
    let (Ok(s), Ok(d)) = (CString::new(src.as_os_str().as_bytes()), CString::new(dst.as_os_str().as_bytes())) else { return };
    unsafe {
        let n = libc::listxattr(s.as_ptr(), std::ptr::null_mut(), 0, 0);
        if n <= 0 {
            return;
        }
        let mut names = vec![0u8; n as usize];
        let n = libc::listxattr(s.as_ptr(), names.as_mut_ptr() as *mut libc::c_char, names.len(), 0);
        if n <= 0 {
            return;
        }
        for name in names[..n as usize].split(|&b| b == 0).filter(|x| !x.is_empty()) {
            let Ok(c) = CString::new(name) else { continue };
            let len = libc::getxattr(s.as_ptr(), c.as_ptr(), std::ptr::null_mut(), 0, 0, 0);
            if len < 0 {
                continue;
            }
            let mut v = vec![0u8; len as usize];
            if libc::getxattr(s.as_ptr(), c.as_ptr(), v.as_mut_ptr() as *mut libc::c_void, v.len(), 0, 0) >= 0 {
                libc::setxattr(d.as_ptr(), c.as_ptr(), v.as_ptr() as *const libc::c_void, v.len(), 0, 0);
            }
        }
    }
}
#[cfg(not(target_os = "macos"))]
pub fn copy_xattrs(_src: &Path, _dst: &Path) {}

/// Moves `tmp` to `dest`, never replacing a file already at `dest`. Uses the atomic exclusive rename where
/// the volume has it; some network shares (SMB) do not, and there the name is first claimed with an
/// exclusive create, which they do support, and the temp file is then renamed over that placeholder.
pub fn persist_new(tmp: tempfile::NamedTempFile, dest: &Path) -> std::io::Result<()> {
    let tmp = match tmp.persist_noclobber(dest) {
        Ok(_) => return Ok(()),
        Err(e) if e.error.kind() == std::io::ErrorKind::Unsupported || e.error.raw_os_error() == Some(45) => e.file,
        Err(e) => return Err(e.error),
    };
    fs::OpenOptions::new().write(true).create_new(true).open(dest)?;
    if let Err(e) = tmp.persist(dest) {
        let _ = fs::remove_file(dest);   // the placeholder is ours and empty
        return Err(e.error);
    }
    Ok(())
}

/// Put one backup back at `path`. `replace`: the file there is the job's output and is overwritten.
pub(crate) fn place(backup: &Path, path: &Path, replace: bool) -> Result<(), String> {
    let dir = path.parent().unwrap_or(Path::new("."));
    if replace {
        if fs::rename(backup, path).is_err() {
            let tmp = tempfile::Builder::new().prefix(".imgcompress-undo-").tempfile_in(dir).map_err(|e| e.to_string())?;
            copy_file(backup, tmp.path())?;
            tmp.persist(path).map_err(|e| e.to_string())?;
            let _ = fs::remove_file(backup);
        }
    } else {
        match fs::hard_link(backup, path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => return Err(format!("{} already exists; not overwriting", path.display())),
            Err(_) => {
                let tmp = tempfile::Builder::new().prefix(".imgcompress-undo-").tempfile_in(dir).map_err(|e| e.to_string())?;
                copy_file(backup, tmp.path())?;
                persist_new(tmp, path).map_err(|e| e.to_string())?;
            }
        }
        let _ = fs::remove_file(backup);
    }
    if let Some(slot) = backup.parent() {
        let _ = fs::remove_dir(slot);
    }
    Ok(())
}

/// Revert one job. Refuses (changing nothing) if the output was edited since, a backup is gone,
/// or a restore target has been taken by another file.
pub fn undo(u: &Undo) -> Result<(), String> {
    let meta = fs::metadata(&u.output).map_err(|_| format!("{} is gone", u.output.display()))?;
    if meta.len() != u.out_size || hash_file(&u.output)? != u.out_hash {
        return Err("the file changed after it was compressed; not undoing".into());
    }
    for r in &u.restore {
        if !r.backup.exists() {
            return Err("the backup of the original is gone".into());
        }
        if r.path != u.output && r.path.exists() {
            return Err(format!("{} already exists; not overwriting", r.path.display()));
        }
    }
    // A backup still linked elsewhere is the twin of an original that went to the Trash (the backup is a
    // hard link of it). Once the original is back, that Trash copy is a duplicate: remove it, like Put Back.
    let twins: Vec<(u64, u64, PathBuf)> = u.restore.iter().filter_map(|r| twin_id(&r.backup).map(|(d, i)| (d, i, r.path.clone()))).collect();
    for r in &u.restore {
        place(&r.backup, &r.path, r.path == u.output)?;
    }
    for (dev, ino, path) in twins {
        remove_twin_in(&trash_dirs(&path), dev, ino);
    }
    if !u.restore.iter().any(|r| r.path == u.output) {
        // The output is exactly what the run wrote (hash checked above) and the original is back, so where
        // the volume has no Trash (network shares) it is deleted instead.
        if crate::pipeline::move_to_trash(&u.output).is_err() {
            fs::remove_file(&u.output).map_err(|e| format!("the original is back, but {} could not be removed: {e}", u.output.display()))?;
        }
    }
    Ok(())
}

/// (device, inode) of a backup that has another hard link (a file that went to the Trash).
#[cfg(unix)]
fn twin_id(backup: &Path) -> Option<(u64, u64)> {
    use std::os::unix::fs::MetadataExt;
    let m = fs::metadata(backup).ok()?;
    (m.nlink() > 1).then(|| (m.dev(), m.ino()))
}
#[cfg(not(unix))]
fn twin_id(_: &Path) -> Option<(u64, u64)> { None }

/// The Trash folders a file at `path` would have gone to: ~/.Trash for the home volume,
/// `<volume>/.Trashes/<uid>` for others.
#[cfg(target_os = "macos")]
fn trash_dirs(path: &Path) -> Vec<PathBuf> {
    use std::os::unix::fs::MetadataExt;
    let mut dirs = vec![];
    if let Some(home) = std::env::var_os("HOME") {
        dirs.push(PathBuf::from(home).join(".Trash"));
    }
    let start = path.parent().unwrap_or(Path::new("/"));
    if let Ok(dev) = fs::metadata(start).map(|m| m.dev()) {
        let mut mount = start.to_path_buf();
        while let Some(up) = mount.parent() {
            if fs::metadata(up).map(|m| m.dev()).ok() != Some(dev) { break; }
            mount = up.to_path_buf();
        }
        dirs.push(mount.join(".Trashes").join(unsafe { libc::getuid() }.to_string()));
    }
    dirs
}
#[cfg(not(target_os = "macos"))]
fn trash_dirs(_: &Path) -> Vec<PathBuf> { vec![] }

/// Removes the top-level entry in `dirs` that is the same file (device and inode) as the restored original.
pub fn remove_twin_in(dirs: &[PathBuf], dev: u64, ino: u64) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        for d in dirs {
            let Ok(rd) = fs::read_dir(d) else { continue };
            for e in rd.flatten() {
                if let Ok(m) = fs::symlink_metadata(e.path()) {
                    if m.is_file() && m.dev() == dev && m.ino() == ino {
                        return fs::remove_file(e.path()).is_ok();
                    }
                }
            }
        }
    }
    false
}
