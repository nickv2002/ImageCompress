use crate::options::Options;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub const EXTS: &[&str] = &["jpg", "jpeg", "png", "gif", "heic", "heif", "webp", "tif", "tiff", "avif", "jxl", "svg", "svgz"];

pub fn is_hidden(p: &Path) -> bool {
    p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with('.'))
}

pub fn ext_of(p: &Path) -> Option<String> {
    p.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase())
}

/// Expand paths into candidate files. Explicitly given files bypass the hidden filter.
pub fn discover(paths: &[PathBuf], o: &Options) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for p in paths {
        if p.is_file() {
            out.push(p.clone());
            continue;
        }
        let mut w = WalkDir::new(p);
        if !o.recursive {
            w = w.max_depth(1);
        }
        for e in w.into_iter().filter_entry(|e| e.depth() == 0 || o.include_hidden || !is_hidden(e.path())).flatten() {
            if e.file_type().is_file() && ext_of(e.path()).is_some_and(|x| EXTS.contains(&x.as_str())) {
                out.push(e.into_path());
            }
        }
    }
    out.sort();
    out.dedup();
    out
}
