//! A lossy encode of a lossless raster (PNG/TIFF) that comes out larger than the target format's own
//! lossless encode must fall back to the lossless result (flat art compresses better losslessly).
use compress_core::{options::*, run, JobEvent};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Mutex;

fn size_after(src: &str, o: &Options) -> u64 {
    let d = tempfile::tempdir().unwrap().keep();
    let p: PathBuf = d.join(Path::new(src).file_name().unwrap());
    std::fs::copy(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures").join(src), &p).unwrap();
    let ev = Mutex::new(vec![]);
    run(&[p], o, &AtomicBool::new(false), &|e| ev.lock().unwrap().push(e));
    for e in ev.into_inner().unwrap() {
        match e {
            JobEvent::Finished { out_bytes, .. } => return out_bytes,
            JobEvent::Failed { error, .. } => panic!("{src}: {error}"),
            _ => {}
        }
    }
    panic!("{src}: no output (skipped?)");
}

fn base(format: Format) -> Options {
    Options { format, replace_if: ReplaceIf::Always, originals: Originals::Keep, history: false, jobs: 1, destination: "alongside".into(), ..Default::default() }
}

/// Fixed-quality lossy encodes (no SSIMULACRA2 search, which is very slow in a debug build) take the same
/// `encode_best` path as the target-quality search, so they exercise the fallback.
#[test]
fn lossy_never_larger_than_lossless_for_flat_png_art() {
    for fmt in [Format::Jxl, Format::Webp] {
        let lossless = size_after("png/flat_illustration.png", &Options { jxl_quality: None, webp_quality: None, ..base(fmt) });
        for q in [60u8, 90] {
            let lossy = size_after("png/flat_illustration.png", &Options { jxl_quality: Some(q), webp_quality: Some(q), ..base(fmt) });
            assert!(lossy <= lossless, "{fmt:?} q{q}: lossy path produced {lossy} B, lossless is {lossless} B");
        }
    }
}
