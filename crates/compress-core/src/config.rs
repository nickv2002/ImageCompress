//! Config file locations and atomic writes. See PRESETS.md.
use std::io::Write;
use std::path::{Path, PathBuf};

pub const APP_GROUP: &str = "3D9SMH4RWM.com.nickvance.imagecompress";
pub const ENV_CONFIG: &str = "IMGCOMPRESS_CONFIG";
pub const ENV_HISTORY: &str = "IMGCOMPRESS_HISTORY";

fn home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(std::env::temp_dir)
}

/// Directory holding `Presets/`, `preset-order.json` and `jobs/`. `IMGCOMPRESS_CONFIG` (a directory) overrides
/// the platform default.
pub fn config_dir() -> PathBuf {
    if let Some(p) = std::env::var_os(ENV_CONFIG).filter(|p| !p.is_empty()) {
        return PathBuf::from(p);
    }
    if cfg!(target_os = "macos") {
        home().join("Library/Group Containers").join(APP_GROUP)
    } else {
        let base = std::env::var_os("XDG_CONFIG_HOME").filter(|p| !p.is_empty()).map(PathBuf::from).unwrap_or_else(|| home().join(".config"));
        base.join("imgcompress")
    }
}

/// One `<id>.json` file per preset.
pub fn presets_dir() -> PathBuf {
    config_dir().join("Presets")
}

/// The list order the Mac app shows (a JSON array of ids); optional.
pub fn order_path() -> PathBuf {
    config_dir().join("preset-order.json")
}

/// Where `imgcompress open` drops job files for the app.
pub fn jobs_dir() -> PathBuf {
    config_dir().join("jobs")
}

/// History database. Deliberately outside the App Group container so plain CLI runs never touch it.
/// `IMGCOMPRESS_HISTORY` overrides.
pub fn history_path() -> PathBuf {
    if let Some(p) = std::env::var_os(ENV_HISTORY).filter(|p| !p.is_empty()) {
        return PathBuf::from(p);
    }
    if cfg!(target_os = "macos") {
        home().join("Library/Application Support/imgcompress/history.sqlite")
    } else {
        let base = std::env::var_os("XDG_DATA_HOME").filter(|p| !p.is_empty()).map(PathBuf::from).unwrap_or_else(|| home().join(".local/share"));
        base.join("imgcompress/history.sqlite")
    }
}

/// Write `bytes` to `path` via a temp file in the same directory, then rename.
pub fn atomic_write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let dir = path.parent().filter(|d| !d.as_os_str().is_empty()).unwrap_or(Path::new("."));
    std::fs::create_dir_all(dir)?;
    let mut tmp = tempfile::Builder::new().prefix(".tmp-").tempfile_in(dir)?;
    tmp.write_all(bytes)?;
    tmp.as_file().sync_all()?;
    tmp.persist(path).map_err(|e| e.error)?;
    Ok(())
}
