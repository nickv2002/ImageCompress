//! ICC / EXIF / XMP preservation across format conversions.
use compress_core::convert::{decode_any, Fmt};
use compress_core::{color, options::*, run};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

fn fixture(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures").join(rel)
}

fn opts(format: Format, metadata: Metadata) -> Options {
    Options { format, metadata, replace_if: ReplaceIf::Always, originals: Originals::Keep, history: false, jobs: 1, avif_speed: 9, webp_method: 0, jxl_effort: 3, jxl_jpeg_transcode: false, ..Default::default() }
}

/// Convert `src` into a scratch dir; returns the output path.
fn convert_to(src: &Path, o: &Options, ext: &str) -> PathBuf {
    let d = tempfile::tempdir().unwrap().keep();
    let p = d.join(src.file_name().unwrap());
    std::fs::copy(src, &p).unwrap();
    let out = run(&[p.clone()], o, &AtomicBool::new(false), &|_| {});
    assert_eq!((out.processed, out.failed), (1, 0), "conversion of {} to {ext}", src.display());
    let dst = p.with_extension(ext);
    assert!(dst.exists(), "{} missing", dst.display());
    dst
}

fn rmse(a: &[u8], b: &[u8]) -> f64 {
    assert_eq!(a.len(), b.len());
    let s: f64 = a.chunks_exact(4).zip(b.chunks_exact(4)).flat_map(|(x, y)| (0..3).map(move |i| (x[i] as f64 - y[i] as f64).powi(2))).sum();
    (s / (a.len() / 4 * 3) as f64).sqrt()
}

/// P3 pixels converted to sRGB: the reference every wide-gamut output must match.
fn p3_reference() -> compress_core::pixels::Decoded {
    let mut d = decode_any(&std::fs::read(fixture("jpeg/display_p3.jpg")).unwrap(), Fmt::Jpeg).unwrap();
    assert!(d.icc.as_deref().is_some_and(|i| !color::is_srgb_equivalent(i)), "fixture must be wide gamut");
    color::convert_to_srgb(&mut d).unwrap();
    d
}

#[test]
fn jxl_embeds_p3_icc_lossless_roundtrip() {
    let raw = decode_any(&std::fs::read(fixture("jpeg/display_p3.jpg")).unwrap(), Fmt::Jpeg).unwrap();
    let mut o = opts(Format::Jxl, Metadata::Icc);
    o.jxl_quality = None;
    let out = convert_to(&fixture("jpeg/display_p3.jpg"), &o, "jxl");
    let mut d = decode_any(&std::fs::read(out).unwrap(), Fmt::Jxl).unwrap();
    let icc = d.icc.clone().expect("P3 profile survives in the JXL");
    assert!(!color::is_srgb_equivalent(&icc));
    // lossless: stored pixel values are untouched (still P3 values), not silently clipped to sRGB
    assert_eq!(rmse(&raw.img.px, &d.img.px), 0.0);
    color::convert_to_srgb(&mut d).unwrap();
    assert!(rmse(&p3_reference().img.px, &d.img.px) < 1.0);
}

#[test]
fn jxl_embeds_p3_icc_lossy_matches_reference() {
    let mut o = opts(Format::Jxl, Metadata::Icc);
    o.jxl_quality = Some(90);
    let out = convert_to(&fixture("jpeg/display_p3.jpg"), &o, "jxl");
    let mut d = decode_any(&std::fs::read(out).unwrap(), Fmt::Jxl).unwrap();
    assert!(d.icc.as_deref().is_some_and(|i| !color::is_srgb_equivalent(i)), "wide-gamut tag kept");
    color::convert_to_srgb(&mut d).unwrap();
    let reference = p3_reference();
    let e = rmse(&reference.img.px, &d.img.px);
    // an unconverted (mis-decoded) result sits ~11 away on this fixture; lossy noise alone is ~5
    let raw = decode_any(&std::fs::read(fixture("jpeg/display_p3.jpg")).unwrap(), Fmt::Jpeg).unwrap();
    let wrong = rmse(&reference.img.px, &raw.img.px);
    assert!(e < 7.0 && e < wrong * 0.6, "lossy P3 jxl vs sRGB reference: rmse {e} (unconverted would be {wrong})");
}

#[test]
fn jxl_metadata_none_drops_icc_and_srgb_flag_converts() {
    let mut o = opts(Format::Jxl, Metadata::None);
    o.jxl_quality = None;
    let out = convert_to(&fixture("jpeg/display_p3.jpg"), &o, "jxl");
    assert!(decode_any(&std::fs::read(out).unwrap(), Fmt::Jxl).unwrap().icc.is_none());

    let mut o = opts(Format::Jxl, Metadata::Icc);
    o.jxl_quality = None;
    o.convert_to_srgb = true;
    let out = convert_to(&fixture("jpeg/display_p3.jpg"), &o, "jxl");
    let d = decode_any(&std::fs::read(out).unwrap(), Fmt::Jxl).unwrap();
    assert!(d.icc.is_none());
    assert!(rmse(&p3_reference().img.px, &d.img.px) < 1.0);
}
