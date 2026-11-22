//! SQLite record of files that are already compressed, so repeated runs skip them.
//! A file counts as done when path, mtime, size, content hash and the options fingerprint all match.
use compress_core::Options;
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;
use std::sync::Mutex;
use std::time::UNIX_EPOCH;

pub struct History {
    conn: Mutex<Connection>,
    opts: String,
}

pub struct Stats {
    pub files: u64,
    pub bytes: u64,
}

fn stat(p: &Path) -> Option<(i64, i64)> {
    let m = std::fs::metadata(p).ok()?;
    let t = m.modified().ok()?.duration_since(UNIX_EPOCH).ok()?;
    Some((t.as_nanos() as i64, m.len() as i64))
}

fn hash(p: &Path) -> Option<String> {
    let mut h = blake3::Hasher::new();
    h.update_reader(std::fs::File::open(p).ok()?).ok()?;
    Some(h.finalize().to_hex().to_string())
}

/// Fingerprint of everything that changes the encoded output (not where it goes or how it runs).
pub fn fingerprint(o: &Options) -> String {
    let mut v = serde_json::to_value(o).unwrap_or_default();
    if let Some(m) = v.as_object_mut() {
        for k in ["history", "jobs", "dry-run", "destination", "suffix", "overwrite", "originals", "recursive", "include-hidden", "min-size", "preserve-dates"] {
            m.remove(k);
        }
    }
    blake3::hash(v.to_string().as_bytes()).to_hex()[..16].to_string()
}

fn open_conn(path: &Path) -> rusqlite::Result<Connection> {
    if let Some(d) = path.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    let c = Connection::open(path)?;
    c.busy_timeout(std::time::Duration::from_secs(30))?;
    c.execute_batch(
        "CREATE TABLE IF NOT EXISTS files (
            path TEXT PRIMARY KEY, opts TEXT NOT NULL, mtime_ns INTEGER NOT NULL,
            size INTEGER NOT NULL, hash TEXT NOT NULL, ts INTEGER NOT NULL);",
    )?;
    Ok(c)
}

impl History {
    pub fn open(path: &Path, o: &Options) -> Result<History, String> {
        Ok(History { conn: Mutex::new(open_conn(path).map_err(|e| format!("history {}: {e}", path.display()))?), opts: fingerprint(o) })
    }

    /// True if `file` is unchanged since it was recorded under the same options.
    pub fn is_done(&self, file: &Path) -> bool {
        let Ok(abs) = std::fs::canonicalize(file) else { return false };
        let Some((mt, sz)) = stat(&abs) else { return false };
        let row: Option<String> = self
            .conn
            .lock()
            .unwrap()
            .query_row("SELECT hash FROM files WHERE path=?1 AND opts=?2 AND mtime_ns=?3 AND size=?4", params![abs.to_string_lossy(), self.opts, mt, sz], |r| r.get(0))
            .optional()
            .ok()
            .flatten();
        row.is_some_and(|h| hash(&abs).as_deref() == Some(&h))
    }

    /// Record the current state of `file` as already compressed.
    pub fn record(&self, file: &Path) {
        let Ok(abs) = std::fs::canonicalize(file) else { return };
        let (Some((mt, sz)), Some(h)) = (stat(&abs), hash(&abs)) else { return };
        let ts = std::time::SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
        let _ = self.conn.lock().unwrap().execute(
            "INSERT OR REPLACE INTO files (path, opts, mtime_ns, size, hash, ts) VALUES (?1,?2,?3,?4,?5,?6)",
            params![abs.to_string_lossy(), self.opts, mt, sz, h, ts],
        );
    }
}

pub fn stats(path: &Path) -> Result<Stats, String> {
    if !path.exists() {
        return Ok(Stats { files: 0, bytes: 0 });
    }
    let c = open_conn(path).map_err(|e| e.to_string())?;
    c.query_row("SELECT COUNT(*), COALESCE(SUM(size),0) FROM files", [], |r| Ok(Stats { files: r.get::<_, i64>(0)? as u64, bytes: r.get::<_, i64>(1)? as u64 })).map_err(|e| e.to_string())
}

pub fn clear(path: &Path) -> Result<u64, String> {
    if !path.exists() {
        return Ok(0);
    }
    let c = open_conn(path).map_err(|e| e.to_string())?;
    let n = c.execute("DELETE FROM files", []).map_err(|e| e.to_string())?;
    let _ = c.execute_batch("VACUUM");
    Ok(n as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_skip_and_invalidate() {
        let d = tempfile::tempdir().unwrap();
        let f = d.path().join("a.png");
        std::fs::write(&f, b"hello").unwrap();
        let db = d.path().join("h.sqlite");
        let o = Options::default();
        let h = History::open(&db, &o).unwrap();
        assert!(!h.is_done(&f));
        h.record(&f);
        assert!(h.is_done(&f));
        let mut o2 = o.clone();
        o2.png_quantize = Some((1, 2));
        assert!(!History::open(&db, &o2).unwrap().is_done(&f));
        std::fs::write(&f, b"hellO").unwrap();
        assert!(!h.is_done(&f));
        assert_eq!(stats(&db).unwrap().files, 1);
        assert_eq!(clear(&db).unwrap(), 1);
        assert_eq!(stats(&db).unwrap().files, 0);
    }
}
