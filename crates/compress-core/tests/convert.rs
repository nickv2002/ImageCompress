use compress_core::convert::{decode_any, Fmt};
use compress_core::{jxl_codec, options::*, run};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

fn fixture(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures").join(rel)
}

fn opts(format: Format) -> Options {
    Options { format, replace_if: ReplaceIf::Always, originals: Originals::Delete, history: false, jobs: 1, ..Default::default() }
}

fn convert_one(src: &Path, o: &Options, ext: &str) -> (Vec<u8>, PathBuf) {
    let d = tempfile::tempdir().unwrap().keep();
    let p = d.join(src.file_name().unwrap());
    std::fs::copy(src, &p).unwrap();
    let out = run(&[p.clone()], o, &AtomicBool::new(false), &|_| {});
    assert_eq!((out.processed, out.failed), (1, 0), "conversion of {} to {ext}", src.display());
    let dst = p.with_extension(ext);
    assert!(dst.exists(), "{} missing", dst.display());
    if o.originals == Originals::Delete && dst != p {
        assert!(!p.exists(), "original should be deleted after format change");
    }
    (std::fs::read(&dst).unwrap(), dst)
}

fn check_dims(bytes: &[u8], f: Fmt, src: &Path, sf: Fmt) {
    let a = decode_any(bytes, f).unwrap().img;
    let b = decode_any(&std::fs::read(src).unwrap(), sf).unwrap().img;
    assert_eq!((a.w, a.h), (b.w, b.h));
}

#[test]
fn png_to_webp_avif_jxl() {
    let src = fixture("png/rgba_photo.png");
    for (f, ext, fmt, magic) in [
        (Format::Webp, "webp", Fmt::Webp, &b"RIFF"[..]),
        (Format::Avif, "avif", Fmt::Avif, &b"ftyp"[..]),
        (Format::Jxl, "jxl", Fmt::Jxl, &[][..]),
    ] {
        let mut o = opts(f);
        o.avif_speed = 9;
        o.jxl_quality = Some(80);
        o.jxl_effort = 3;
        let (bytes, _) = convert_one(&src, &o, ext);
        if !magic.is_empty() {
            assert!(bytes.windows(4).take(16).any(|w| w == magic), "{ext} magic");
        }
        check_dims(&bytes, fmt, &src, Fmt::Png);
    }
}

#[test]
fn webp_lossless_preserves_pixels() {
    let src = fixture("png/rgba_photo.png");
    let mut o = opts(Format::Webp);
    o.webp_quality = None;
    let (bytes, _) = convert_one(&src, &o, "webp");
    let a = decode_any(&bytes, Fmt::Webp).unwrap().img;
    let b = decode_any(&std::fs::read(&src).unwrap(), Fmt::Png).unwrap().img;
    // lossless: identical wherever alpha != 0
    for (pa, pb) in a.px.chunks_exact(4).zip(b.px.chunks_exact(4)) {
        assert_eq!(pa[3], pb[3]);
        if pb[3] != 0 {
            assert_eq!(pa, pb);
        }
    }
}

#[test]
fn jpeg_to_jxl_transcode_is_byte_exact() {
    for name in ["jpeg/baseline.jpg", "jpeg/progressive.jpg", "jpeg/exif_gps_orient6.jpg"] {
        let src = fixture(name);
        let orig = std::fs::read(&src).unwrap();
        let mut o = opts(Format::Jxl);
        o.jxl_effort = 3;
        let (bytes, _) = convert_one(&src, &o, "jxl");
        assert!(bytes.len() < orig.len(), "{name}: transcode should shrink");
        assert_eq!(jxl_codec::reconstruct_jpeg(&bytes).unwrap(), orig, "{name}: reconstruction must be byte-exact");
    }
}

#[test]
fn jpeg_to_jxl_lossy_when_transcode_disabled() {
    let src = fixture("jpeg/baseline.jpg");
    let mut o = opts(Format::Jxl);
    o.jxl_jpeg_transcode = false;
    o.jxl_quality = Some(80);
    o.jxl_effort = 3;
    let (bytes, _) = convert_one(&src, &o, "jxl");
    assert!(jxl_codec::reconstruct_jpeg(&bytes).is_err());
    check_dims(&bytes, Fmt::Jxl, &src, Fmt::Jpeg);
}

#[test]
fn same_format_recompress_webp_avif_jxl() {
    for (name, ext) in [("modern/photo.webp", "webp"), ("modern/photo.avif", "avif"), ("modern/photo.jxl", "jxl")] {
        let src = fixture(name);
        let mut o = opts(Format::Keep);
        o.jxl_quality = Some(80);
        o.jxl_effort = 3;
        o.avif_speed = 9;
        let (bytes, _) = convert_one(&src, &o, ext);
        let sf = Fmt::from_ext(ext).unwrap();
        check_dims(&bytes, sf, &src, sf);
    }
}

#[test]
fn recompress_keeps_original_when_not_smaller() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("photo.webp");
    std::fs::copy(fixture("modern/photo.webp"), &p).unwrap();
    let before = std::fs::read(&p).unwrap();
    let mut o = Options { history: false, jobs: 1, ..Default::default() };
    o.webp_quality = None; // lossless re-encode of a lossy file is bigger
    let out = run(&[p.clone()], &o, &AtomicBool::new(false), &|_| {});
    assert_eq!((out.processed, out.skipped), (0, 1));
    assert_eq!(std::fs::read(&p).unwrap(), before);
}

#[test]
fn format_change_refuses_to_overwrite_existing_file() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("a.png");
    std::fs::copy(fixture("png/rgba_photo.png"), &p).unwrap();
    std::fs::write(d.path().join("a.webp"), b"precious").unwrap();
    let out = run(&[p.clone()], &opts(Format::Webp), &AtomicBool::new(false), &|_| {});
    // default `overwrite = skip`: reported as skipped, nothing touched
    assert_eq!((out.skipped, out.failed, out.processed), (1, 0, 0));
    assert!(p.exists());
    assert_eq!(std::fs::read(d.path().join("a.webp")).unwrap(), b"precious");
}

#[test]
fn format_change_alongside_keeps_original_by_default() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("a.png");
    std::fs::copy(fixture("png/rgba_photo.png"), &p).unwrap();
    let o = Options { format: Format::Webp, destination: "alongside".into(), replace_if: ReplaceIf::Always, history: false, ..Default::default() };
    run(&[p.clone()], &o, &AtomicBool::new(false), &|_| {});
    assert!(p.exists() && d.path().join("a-min.webp").exists());
}

#[test]
fn png_to_jpeg_and_back_to_png() {
    let src = fixture("png/rgba_photo.png");
    let (jpg, jp) = convert_one(&src, &opts(Format::Jpeg), "jpg");
    check_dims(&jpg, Fmt::Jpeg, &src, Fmt::Png);
    let o = opts(Format::Png);
    let (png, _) = convert_one(&jp, &o, "png");
    check_dims(&png, Fmt::Png, &src, Fmt::Png);
}

#[cfg(target_os = "macos")]
#[test]
fn heic_roundtrip_and_conversion() {
    let src = fixture("jpeg/baseline.jpg");
    let (bytes, hp) = convert_one(&src, &opts(Format::Heic), "heic");
    check_dims(&bytes, Fmt::Heic, &src, Fmt::Jpeg);
    // heic -> webp, and same-format heic recompress
    let (w, _) = convert_one(&hp, &opts(Format::Webp), "webp");
    check_dims(&w, Fmt::Webp, &src, Fmt::Jpeg);
    let src2 = fixture("modern/photo.heic");
    if src2.exists() {
        let mut o = opts(Format::Keep);
        o.heic_quality = 30;
        let (b2, _) = convert_one(&src2, &o, "heic");
        check_dims(&b2, Fmt::Heic, &src2, Fmt::Heic);
    }
}

#[cfg(target_os = "macos")]
#[test]
fn heic_quality_mapping_is_monotonic() {
    use compress_core::heic_codec::quality_to_imageio as m;
    assert!((m(50) - 0.64).abs() < 1e-6);
    assert!((1..100).all(|q| m(q) <= m(q + 1)));
    assert!(m(100) <= 1.0 && m(1) > 0.0);
}
