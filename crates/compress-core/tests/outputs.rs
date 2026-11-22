//! Output naming end to end: collisions inside a run, files that already exist, concurrency, in-place.
use compress_core::{options::*, run, JobEvent};
use image::ImageFormat;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Mutex;

fn pic(seed: u32, fmt: ImageFormat) -> Vec<u8> {
    let img = image::RgbImage::from_fn(48, 48, |x, y| image::Rgb([(x * 5 + seed) as u8, (y * 3 + seed * 7) as u8, ((x ^ y) * 4) as u8]));
    let mut out = Vec::new();
    image::DynamicImage::ImageRgb8(img).write_to(&mut std::io::Cursor::new(&mut out), fmt).unwrap();
    out
}

fn put(dir: &Path, name: &str, seed: u32) -> PathBuf {
    let p = dir.join(name);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    let fmt = match name.rsplit('.').next().unwrap() {
        "png" => ImageFormat::Png,
        "jpg" | "jpeg" => ImageFormat::Jpeg,
        "gif" => ImageFormat::Gif,
        "tif" | "tiff" => ImageFormat::Tiff,
        "webp" => ImageFormat::WebP,
        e => panic!("{e}"),
    };
    std::fs::write(&p, pic(seed, fmt)).unwrap();
    p
}

fn base() -> Options {
    Options { format: Format::Webp, replace_if: ReplaceIf::Always, history: false, jobs: 1, min_size: 0, ..Default::default() }
}

fn alongside() -> Options {
    Options { destination: "alongside".into(), suffix: "-w".into(), ..base() }
}

fn go(dir: &Path, o: &Options) -> (compress_core::pipeline::Outcome, Vec<JobEvent>) {
    let ev = Mutex::new(vec![]);
    let out = run(&[dir.to_path_buf()], o, &AtomicBool::new(false), &|e| ev.lock().unwrap().push(e));
    (out, ev.into_inner().unwrap())
}

fn names(dir: &Path) -> BTreeSet<String> {
    let mut v = BTreeSet::new();
    for e in walkdir::WalkDir::new(dir).into_iter().flatten().filter(|e| e.file_type().is_file()) {
        v.insert(e.path().strip_prefix(dir).unwrap().to_string_lossy().into_owned());
    }
    v
}

fn decodes(p: &Path) -> bool {
    image::open(p).is_ok()
}

#[test]
fn colliding_names_in_one_run_are_disambiguated_not_overwritten() {
    let d = tempfile::tempdir().unwrap();
    for (i, n) in ["a.png", "a.jpg", "a.gif", "a.tif", "b.png"].iter().enumerate() {
        put(d.path(), n, i as u32 * 40);
    }
    let (out, _) = go(d.path(), &alongside());
    assert_eq!((out.processed, out.failed, out.skipped), (5, 0, 0));
    let want: BTreeSet<String> = ["a.png", "a.jpg", "a.gif", "a.tif", "b.png", "a-w.png.webp", "a-w.jpg.webp", "a-w.gif.webp", "a-w.tif.webp", "b-w.webp"].map(String::from).into();
    assert_eq!(names(d.path()), want);
    // five different pictures -> five different outputs (none overwrote another)
    let sizes: BTreeSet<Vec<u8>> = want.iter().filter(|n| n.ends_with(".webp")).map(|n| std::fs::read(d.path().join(n)).unwrap()).collect();
    assert_eq!(sizes.len(), 5);
    assert!(want.iter().filter(|n| n.ends_with(".webp")).all(|n| decodes(&d.path().join(n))));
}

#[test]
fn names_do_not_depend_on_worker_count() {
    let mk = |jobs| {
        let d = tempfile::tempdir().unwrap();
        for (i, n) in ["a.png", "a.jpg", "a.gif", "x/a.png", "y/a.png"].iter().enumerate() {
            put(d.path(), n, i as u32);
        }
        go(d.path(), &Options { jobs, ..alongside() });
        names(d.path())
    };
    assert_eq!(mk(1), mk(8));
}

#[test]
fn concurrent_workers_never_clobber_each_other() {
    let d = tempfile::tempdir().unwrap();
    let mut n = 0;
    for stem in 0..12 {
        for ext in ["png", "jpg", "gif", "tif"] {
            put(d.path(), &format!("img{stem}.{ext}"), n);
            n += 1;
        }
    }
    let (out, _) = go(d.path(), &Options { jobs: 8, ..alongside() });
    assert_eq!((out.processed, out.failed, out.skipped), (48, 0, 0));
    let outputs: Vec<String> = names(d.path()).into_iter().filter(|n| n.ends_with(".webp")).collect();
    assert_eq!(outputs.len(), 48);
    assert!(outputs.iter().all(|n| decodes(&d.path().join(n))));
    assert!(names(d.path()).iter().all(|n| !n.starts_with(".imgcompress-")), "no temp files left");
}

#[test]
fn existing_output_is_skipped_by_default_and_the_original_survives() {
    let d = tempfile::tempdir().unwrap();
    let src = put(d.path(), "a.png", 1);
    let other = d.path().join("a-w.webp");
    std::fs::write(&other, b"precious").unwrap();
    // even with originals=delete: a skipped input must not lose its original
    let ev = Mutex::new(vec![]);
    let o = Options { originals: Originals::Delete, ..alongside() };
    let out = run(&[src.clone()], &o, &AtomicBool::new(false), &|e| ev.lock().unwrap().push(e));
    let ev = ev.into_inner().unwrap();
    assert_eq!((out.processed, out.skipped, out.failed), (0, 1, 0));
    assert!(ev.iter().any(|e| matches!(e, JobEvent::Skipped { reason, .. } if reason.starts_with("output exists"))));
    assert_eq!(std::fs::read(&other).unwrap(), b"precious");
    assert!(src.exists());
}

#[test]
fn existing_output_rename_picks_the_next_free_name() {
    let d = tempfile::tempdir().unwrap();
    put(d.path(), "a.png", 1);
    std::fs::write(d.path().join("a-w.webp"), b"one").unwrap();
    std::fs::write(d.path().join("a-w-2.webp"), b"two").unwrap();
    let (out, _) = go(d.path(), &Options { overwrite: Overwrite::Rename, ..alongside() });
    // the two pre-existing webp files are inputs too (-> a-w-w.webp, ...); they are not valid images
    assert_eq!(out.processed, 1);
    assert_eq!(std::fs::read(d.path().join("a-w.webp")).unwrap(), b"one");
    assert_eq!(std::fs::read(d.path().join("a-w-2.webp")).unwrap(), b"two");
    assert!(decodes(&d.path().join("a-w-3.webp")));
}

#[test]
fn existing_output_replace_overwrites_only_with_the_option() {
    let d = tempfile::tempdir().unwrap();
    put(d.path(), "a.png", 1);
    let other = d.path().join("a-w.webp");
    // not an input of the run: keep it out of discovery by pointing the run at the png only
    std::fs::write(&other, b"old").unwrap();
    let src = d.path().join("a.png");
    let o = Options { overwrite: Overwrite::Replace, ..alongside() };
    let out = run(&[src], &o, &AtomicBool::new(false), &|_| {});
    assert_eq!((out.processed, out.failed), (1, 0));
    assert!(decodes(&other));
}

#[test]
fn replace_does_not_overwrite_another_input_of_the_run() {
    let d = tempfile::tempdir().unwrap();
    put(d.path(), "a.png", 1);
    let w = put(d.path(), "a-w.webp", 9);
    let before = std::fs::read(&w).unwrap();
    let (out, _) = go(d.path(), &Options { overwrite: Overwrite::Replace, ..alongside() });
    // a.png wants a-w.webp, which is itself an input: it goes to a-w-2.webp, the input is untouched
    assert_eq!((out.processed, out.failed), (2, 0));
    assert_eq!(std::fs::read(&w).unwrap(), before);
    assert_eq!(names(d.path()), ["a.png", "a-w.webp", "a-w-2.webp", "a-w-w.webp"].map(String::from).into());
}

#[test]
fn destination_folder_flattening_collisions_are_numbered() {
    let d = tempfile::tempdir().unwrap();
    let src = d.path().join("in");
    for (sub, seed) in [("x", 1), ("y", 2), ("z", 3)] {
        put(&src, &format!("{sub}/IMG.png"), seed);
    }
    let out_dir = d.path().join("out");
    let o = Options { destination: out_dir.to_string_lossy().into(), format: Format::Keep, ..base() };
    let (out, _) = go(&src, &o);
    assert_eq!((out.processed, out.failed, out.skipped), (3, 0, 0));
    assert_eq!(names(&out_dir), ["IMG-2.png", "IMG-3.png", "IMG.png"].map(String::from).into());
}

#[test]
fn rerun_is_idempotent_for_converted_outputs() {
    let d = tempfile::tempdir().unwrap();
    let src = put(d.path(), "a.png", 1);
    let o = Options { destination: d.path().to_string_lossy().into(), ..base() };
    // first run converts in place of a folder-destination; a second run on the png alone skips cleanly
    let out1 = run(&[src.clone()], &o, &AtomicBool::new(false), &|_| {});
    let out2 = run(&[src], &o, &AtomicBool::new(false), &|_| {});
    assert_eq!((out1.processed, out2.processed, out2.skipped, out2.failed), (1, 0, 1, 0));
}

#[test]
fn in_place_same_format_still_replaces_atomically() {
    let d = tempfile::tempdir().unwrap();
    let p = put(d.path(), "a.png", 1);
    let before = std::fs::read(&p).unwrap();
    let o = Options { format: Format::Keep, replace_if: ReplaceIf::Always, ..base() };
    let (out, ev) = go(d.path(), &o);
    assert_eq!((out.processed, out.failed), (1, 0));
    assert!(ev.iter().any(|e| matches!(e, JobEvent::Finished { output, .. } if output == &p)));
    assert_ne!(std::fs::read(&p).unwrap(), before);
    assert_eq!(names(d.path()), ["a.png".to_string()].into());
}

#[test]
fn in_place_conversion_keeps_the_same_format_neighbour() {
    let d = tempfile::tempdir().unwrap();
    put(d.path(), "a.png", 1);
    let w = put(d.path(), "a.webp", 2);
    let (out, _) = go(d.path(), &base());
    assert_eq!((out.processed, out.failed), (2, 0));
    // a.webp was re-encoded in place; the png got a distinct name instead of fighting for a.webp
    assert_eq!(names(d.path()), ["a.png", "a.png.webp", "a.webp"].map(String::from).into());
    assert!(decodes(&w));
}
