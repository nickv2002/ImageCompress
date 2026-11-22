use crate::options::Options;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

/// Locate gifsicle: next to the executable, ../Helpers (app bundle), then PATH / Homebrew.
pub fn find_gifsicle() -> Option<PathBuf> {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            for c in [dir.join("gifsicle"), dir.join("../Helpers/gifsicle")] {
                if c.is_file() {
                    return Some(c);
                }
            }
        }
    }
    for c in ["/opt/homebrew/bin/gifsicle", "/usr/local/bin/gifsicle", "/usr/bin/gifsicle"] {
        let p = PathBuf::from(c);
        if p.is_file() {
            return Some(p);
        }
    }
    std::env::var_os("PATH").and_then(|paths| std::env::split_paths(&paths).map(|d| d.join("gifsicle")).find(|p| p.is_file()))
}

/// gifsicle over stdin/stdout. `None` if the helper is missing.
pub fn optimize(data: &[u8], o: &Options) -> Result<Option<Vec<u8>>, String> {
    let Some(bin) = find_gifsicle() else { return Ok(None) };
    let mut cmd = Command::new(bin);
    cmd.arg(format!("-O{}", o.gif_optimize_level.clamp(1, 3)));
    if let Some(l) = o.gif_lossy {
        cmd.arg(format!("--lossy={l}"));
    }
    cmd.args(["--no-warnings"]).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = cmd.spawn().map_err(|e| e.to_string())?;
    let mut stdin = child.stdin.take().unwrap();
    let input = data.to_vec();
    let t = std::thread::spawn(move || stdin.write_all(&input));
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    let _ = t.join();
    if !out.status.success() {
        return Err(format!("gifsicle: {}", String::from_utf8_lossy(&out.stderr).trim()));
    }
    Ok(Some(out.stdout))
}
