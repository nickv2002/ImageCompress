//! `replace-if` and the PNG palette note: converting to a format that comes out larger (JPEG -> PNG).
use compress_core::{options::*, run, JobEvent};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Mutex;

fn fixture(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures").join(rel)
}

fn opts(format: Format, replace_if: ReplaceIf) -> Options {
    Options { format, replace_if, originals: Originals::Keep, history: false, jobs: 1, ..Default::default() }
}

/// Copies `src` into a fresh folder and runs one job on it; returns the copy and the events.
fn run_on(src: &Path, o: &Options) -> (PathBuf, Vec<JobEvent>) {
    let d = tempfile::tempdir().unwrap().keep();
    let p = d.join(src.file_name().unwrap());
    std::fs::copy(src, &p).unwrap();
    let ev = Mutex::new(vec![]);
    run(&[p.clone()], o, &AtomicBool::new(false), &|e| ev.lock().unwrap().push(e));
    (p, ev.into_inner().unwrap())
}

fn finished(ev: &[JobEvent]) -> Option<(u64, u64, String, Option<String>)> {
    ev.iter().find_map(|e| match e {
        JobEvent::Finished { in_bytes, out_bytes, encoder, note, .. } => Some((*in_bytes, *out_bytes, encoder.clone(), note.clone())),
        _ => None,
    })
}

fn skip_reason(ev: &[JobEvent]) -> Option<String> {
    ev.iter().find_map(|e| match e { JobEvent::Skipped { reason, .. } => Some(reason.clone()), _ => None })
}

/// A spreadsheet-like screenshot saved as JPEG: flat colours, grid lines, JPEG ringing around them.
fn flat_jpeg(dir: &Path) -> PathBuf {
    let img = image::RgbImage::from_fn(640, 400, |x, y| {
        if x % 80 == 0 || y % 20 == 0 { image::Rgb([200, 200, 200]) }
        else if y < 20 { image::Rgb([30, 110, 60]) }
        else if (x / 80 + y / 20) % 7 == 0 { image::Rgb([20, 20, 20]) }
        else { image::Rgb([255, 255, 255]) }
    });
    let p = dir.join("sheet.jpg");
    let mut f = std::fs::File::create(&p).unwrap();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut f, 85).encode_image(&img).unwrap();
    p
}

#[test]
fn larger_conversion_is_skipped_by_default() {
    // a photo JPEG is several times larger as a lossless PNG
    let (p, ev) = run_on(&fixture("jpeg/baseline.jpg"), &opts(Format::Png, ReplaceIf::Smaller));
    assert_eq!(skip_reason(&ev).as_deref(), Some("not smaller"));
    assert!(!p.with_extension("png").exists());
}

#[test]
fn smaller_or_converted_keeps_a_larger_conversion() {
    let (p, ev) = run_on(&fixture("jpeg/baseline.jpg"), &opts(Format::Png, ReplaceIf::SmallerOrConverted));
    let (i, o, _, _) = finished(&ev).expect("converted file kept");
    assert!(o > i, "test needs a conversion that grows: {i} -> {o}");
    assert!(p.with_extension("png").exists());
    assert!(p.exists(), "original kept");
}

#[test]
fn smaller_or_converted_still_skips_a_same_format_result_that_does_not_shrink() {
    // an oxipng result run through oxipng again cannot get smaller
    let (first, ev) = run_on(&fixture("png/flat_illustration.png"), &opts(Format::Keep, ReplaceIf::Always));
    assert!(finished(&ev).is_some());
    let (_, ev) = run_on(&first, &opts(Format::Keep, ReplaceIf::SmallerOrConverted));
    assert_eq!(skip_reason(&ev).as_deref(), Some("not smaller"));
}

#[test]
fn photo_to_png_without_a_palette_says_why() {
    let mut o = opts(Format::Png, ReplaceIf::Always);
    o.png_quantize = Some((80, 95));
    let (_, ev) = run_on(&fixture("jpeg/baseline.jpg"), &o);
    let (_, _, enc, note) = finished(&ev).unwrap();
    assert_eq!(enc, "oxipng");
    assert_eq!(note.as_deref(), Some(compress_core::pipeline::PNG_NO_PALETTE));
}

#[test]
fn flat_jpeg_to_png_gets_a_palette_and_is_compressed() {
    let d = tempfile::tempdir().unwrap();
    let src = flat_jpeg(d.path());
    let mut o = opts(Format::Png, ReplaceIf::Smaller);
    o.png_quantize = Some((80, 95));
    let (_, ev) = run_on(&src, &o);
    let (i, out, enc, note) = finished(&ev).expect("palette PNG is smaller than the JPEG");
    assert_eq!(enc, "imagequant+oxipng");
    assert!(out < i);
    assert_eq!(note, None);
    // lossless PNG gets no note: no palette was asked for
    let (_, ev) = run_on(&src, &opts(Format::Png, ReplaceIf::Always));
    assert_eq!(finished(&ev).unwrap().3, None);
}
