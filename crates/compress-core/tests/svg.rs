use compress_core::{options::*, run};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Mutex;

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/other").join(name)
}

fn go(dir: &Path, o: &Options) -> (compress_core::pipeline::Outcome, Vec<compress_core::JobEvent>) {
    let ev = Mutex::new(vec![]);
    let out = run(&[dir.to_path_buf()], o, &AtomicBool::new(false), &|e| ev.lock().unwrap().push(e));
    (out, ev.into_inner().unwrap())
}

fn staged(names: &[&str]) -> tempfile::TempDir {
    let d = tempfile::tempdir().unwrap();
    for n in names {
        std::fs::copy(fixture(n), d.path().join(n)).unwrap();
    }
    d
}

#[test]
fn svg_in_place_smaller_and_atomic() {
    let d = staged(&["icon.svg"]);
    let before = std::fs::metadata(d.path().join("icon.svg")).unwrap().len();
    let (out, _) = go(d.path(), &Options::default());
    assert_eq!((out.processed, out.failed), (1, 0));
    let s = std::fs::read_to_string(d.path().join("icon.svg")).unwrap();
    assert!((s.len() as u64) < before && s.contains("<svg"));
    // ids survive in the default job set
    assert!(s.contains("sheen") && s.contains("icon-root"), "{s}");
    // no temp files left behind
    let names: Vec<_> = std::fs::read_dir(d.path()).unwrap().map(|e| e.unwrap().file_name()).collect();
    assert_eq!(names.len(), 1, "{names:?}");
}

#[test]
fn svgz_stays_svgz() {
    let d = staged(&["icon.svgz"]);
    let before = std::fs::read(d.path().join("icon.svgz")).unwrap();
    let (out, _) = go(d.path(), &Options::default());
    let after = std::fs::read(d.path().join("icon.svgz")).unwrap();
    assert_eq!(&after[..2], &[0x1f, 0x8b]);
    assert!(after.len() <= before.len());
    assert!(out.processed + out.skipped == 1 && out.failed == 0);
}

#[test]
fn svg_ignores_format_conversion_and_writes_alongside() {
    let d = staged(&["logo.svg"]);
    let o = Options { format: Format::Webp, destination: "alongside".into(), ..Default::default() };
    let (out, _) = go(d.path(), &o);
    assert_eq!(out.failed, 0);
    assert!(!d.path().join("logo-min.webp").exists());
    assert_eq!(out.processed, 1);
    assert!(d.path().join("logo-min.svg").exists());
}

#[test]
fn distorting_optimisation_leaves_file_byte_identical() {
    let d = tempfile::tempdir().unwrap();
    let svg = "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 1 1\" width=\"100\" height=\"100\">\n  <!-- c -->\n  <circle cx=\"0.37000\" cy=\"0.42000\" r=\"0.30000\" fill=\"#ff0000\"/>\n  <rect x=\"0.55000\" y=\"0.60000\" width=\"0.30000\" height=\"0.30000\" fill=\"#0000ff\"/>\n</svg>";
    let p = d.path().join("tiny.svg");
    std::fs::write(&p, svg).unwrap();
    let o = Options { svg_precision: 0, ..Default::default() };
    let (out, _) = go(d.path(), &o);
    assert_eq!((out.processed, out.skipped, out.failed), (0, 1, 0));
    assert_eq!(std::fs::read_to_string(&p).unwrap(), svg);
}

#[test]
fn aggressive_mode_stays_render_checked() {
    let d = staged(&["transform_trap.svg"]);
    let src = std::fs::read(d.path().join("transform_trap.svg")).unwrap();
    let o = Options { svg_aggressive: true, ..Default::default() };
    let (out, _) = go(d.path(), &o);
    assert_eq!(out.failed, 0);
    let after = std::fs::read(d.path().join("transform_trap.svg")).unwrap();
    assert!(after.len() <= src.len());
}

#[test]
fn invalid_and_oversized_svgs() {
    let d = tempfile::tempdir().unwrap();
    std::fs::write(d.path().join("bad.svg"), b"<svg xmlns=\"http://www.w3.org/2000/svg\"><g></svg>").unwrap();
    std::fs::write(d.path().join("big.svg"), std::fs::read(fixture("icon.svg")).unwrap()).unwrap();
    let o = Options { svg_max_size: 1000, ..Default::default() };
    let before = std::fs::read(d.path().join("bad.svg")).unwrap();
    let (out, _) = go(d.path(), &o);
    assert_eq!((out.failed, out.skipped, out.processed), (1, 1, 0));
    assert_eq!(std::fs::read(d.path().join("bad.svg")).unwrap(), before);
}
