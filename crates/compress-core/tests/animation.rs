//! Animated inputs must never silently lose frames: every outcome is either a skip that says why or
//! an output with the same number of frames.
use compress_core::anim::frame_count;
use compress_core::convert::Fmt;
use compress_core::{options::*, run, JobEvent};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Mutex;

const FRAMES: u32 = 8;

fn fixture(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures").join(rel)
}

fn base() -> Options {
    Options { replace_if: ReplaceIf::Always, originals: Originals::Delete, history: false, jobs: 1, ..Default::default() }
}

enum Outcome {
    Out(PathBuf, Vec<u8>),
    Skipped(String),
}

fn go(src: &str, o: &Options) -> Outcome {
    let d = tempfile::tempdir().unwrap().keep();
    let p = d.join(Path::new(src).file_name().unwrap());
    std::fs::copy(fixture(src), &p).unwrap();
    let ev = Mutex::new(vec![]);
    run(&[p.clone()], o, &AtomicBool::new(false), &|e| ev.lock().unwrap().push(e));
    for e in ev.into_inner().unwrap() {
        match e {
            JobEvent::Finished { output, .. } => {
                let b = std::fs::read(&output).unwrap();
                return Outcome::Out(output, b);
            }
            JobEvent::Skipped { reason, .. } => return Outcome::Skipped(reason),
            JobEvent::Failed { error, .. } => panic!("{src}: failed: {error}"),
            _ => {}
        }
    }
    panic!("{src}: no outcome");
}

fn fmt_of(p: &Path) -> Fmt {
    Fmt::from_ext(&p.extension().unwrap().to_string_lossy().to_ascii_lowercase()).unwrap()
}

/// The invariant: skip with an "animated" reason, or keep every frame.
fn assert_no_frame_loss(src: &str, o: &Options, what: &str) {
    match go(src, o) {
        Outcome::Skipped(r) => assert!(r.contains("animated") || r == "not smaller", "{src} [{what}]: skipped without saying why: {r}"),
        Outcome::Out(p, b) => {
            let n = frame_count(&b, fmt_of(&p));
            assert_eq!(n, FRAMES, "{src} [{what}] -> {}: frames {n}, expected {FRAMES}", p.display());
        }
    }
}

// HEIC sequences are only detected (and decoded at all) where ImageIO exists
const ANIMATED: &[&str] = &["anim/animated.png", "anim/animated.webp", "anim/animated.avif", "anim/animated.jxl", #[cfg(target_os = "macos")] "anim/sequence.heic"];

fn variants() -> Vec<(String, Options)> {
    let mut v = vec![("default".to_string(), base())];
    for m in [Metadata::None, Metadata::Icc, Metadata::All, Metadata::ExifNoGps] {
        v.push((format!("metadata {m:?}"), Options { metadata: m, ..base() }));
    }
    v.push(("png-quantize".into(), Options { png_quantize: Some((60, 90)), ..base() }));
    v.push(("zopfli".into(), Options { png_zopfli: Some(3), ..base() }));
    v.push(("lossy-preset".into(), Options { png_quantize: Some((40, 80)), jpeg_quality: Some(70), webp_quality: Some(60), ..base() }));
    v.push(("lossless webp".into(), Options { webp_quality: None, ..base() }));
    v.push(("max-dimension".into(), Options { max_dimension: Some(80), ..base() }));
    v.push(("target-quality".into(), Options { target_quality: Some(60.0), ..base() }));
    for f in [Format::Jpeg, Format::Png, Format::Webp, Format::Avif, Format::Jxl, Format::Heic] {
        v.push((format!("format {f:?}"), Options { format: f, ..base() }));
    }
    v
}

#[test]
fn animated_inputs_never_lose_frames() {
    for src in ANIMATED {
        for (name, o) in variants() {
            assert_no_frame_loss(src, &o, &name);
        }
    }
}

#[test]
fn apng_lossless_keeps_frames_and_animation_chunks_for_every_metadata_mode() {
    for m in [Metadata::None, Metadata::Icc, Metadata::All, Metadata::ExifNoGps] {
        let Outcome::Out(p, b) = go("anim/animated.png", &Options { metadata: m, ..base() }) else { panic!("{m:?}: skipped") };
        assert_eq!(p.extension().unwrap(), "png");
        assert_eq!(frame_count(&b, Fmt::Png), FRAMES, "{m:?}");
        for chunk in [&b"acTL"[..], b"fcTL", b"fdAT"] {
            assert!(b.windows(4).any(|w| w == chunk), "{m:?}: lost {}", String::from_utf8_lossy(chunk));
        }
    }
}

#[test]
fn apng_lossy_settings_do_not_quantize_the_animation_away() {
    let o = Options { png_quantize: Some((60, 90)), ..base() };
    let Outcome::Out(_, b) = go("anim/animated.png", &o) else { panic!("skipped") };
    assert_eq!(frame_count(&b, Fmt::Png), FRAMES);
}

#[test]
fn unsupported_animations_skip_with_a_clear_reason() {
    // animated JXL and multi-frame HEIC have no frame decoder yet
    for src in ["anim/animated.jxl", #[cfg(target_os = "macos")] "anim/sequence.heic"] {
        for o in [base(), Options { format: Format::Webp, ..base() }, Options { jxl_quality: Some(80), ..base() }] {
            match go(src, &o) {
                Outcome::Skipped(r) => assert_eq!(r, "animated; not supported for this operation", "{src}"),
                Outcome::Out(p, _) => panic!("{src}: should have been skipped, wrote {}", p.display()),
            }
        }
    }
}

fn must_encode(src: &str, o: &Options) -> (PathBuf, Vec<u8>) {
    match go(src, o) {
        Outcome::Out(p, b) => (p, b),
        Outcome::Skipped(r) => panic!("{src}: skipped: {r}"),
    }
}

fn dims(b: &[u8], f: Fmt) -> (u32, u32) {
    let (fr, _) = compress_core::anim::decode(b, f).unwrap();
    (fr.w, fr.h)
}

#[test]
fn converts_every_animated_source_to_animated_webp_and_avif() {
    for src in ["anim/animated.png", "anim/animated.webp", "anim/animated.avif", "gif/animated.gif"] {
        for (f, ext) in [(Format::Webp, "webp"), (Format::Avif, "avif")] {
            let (p, b) = must_encode(src, &Options { format: f, ..base() });
            assert_eq!(p.extension().unwrap(), ext, "{src}");
            assert_eq!(frame_count(&b, fmt_of(&p)), FRAMES, "{src} -> {ext}");
        }
    }
}

#[test]
fn anim_to_option_redirects_animations_and_stills_follow_for_gif() {
    for (a, ext) in [(AnimTo::Webp, "webp"), (AnimTo::Avif, "avif")] {
        for src in ["anim/animated.png", "gif/animated.gif"] {
            let (p, b) = must_encode(src, &Options { anim_to: a, ..base() });
            assert_eq!(p.extension().unwrap(), ext);
            assert_eq!(frame_count(&b, fmt_of(&p)), FRAMES);
        }
        // a still GIF follows too (a one-frame image in the new format)
        let (p, b) = must_encode("gif/static.gif", &Options { anim_to: a, ..base() });
        assert_eq!(p.extension().unwrap(), ext);
        assert_eq!(frame_count(&b, fmt_of(&p)), 1);
        // an ordinary PNG is not an animation: untouched by anim_to
        let (p, _) = must_encode("png/palette.png", &Options { anim_to: a, ..base() });
        assert_eq!(p.extension().unwrap(), "png");
    }
}

#[test]
fn animated_webp_and_avif_recompress_in_format() {
    for (src, ext) in [("anim/animated.webp", "webp"), ("anim/animated.avif", "avif")] {
        let o = Options { webp_quality: Some(30), avif_quality: 25, avif_speed: 9, ..base() };
        let before = std::fs::metadata(fixture(src)).unwrap().len();
        let (p, b) = must_encode(src, &o);
        assert_eq!(p.extension().unwrap(), ext);
        assert_eq!(frame_count(&b, fmt_of(&p)), FRAMES);
        assert!((b.len() as u64) < before, "{src}: {} !< {before}", b.len());
    }
}

#[test]
fn animated_lossless_webp_recompression_is_refused() {
    match go("anim/animated.webp", &Options { webp_quality: None, ..base() }) {
        Outcome::Skipped(r) => assert!(r.starts_with("animated WebP"), "{r}"),
        Outcome::Out(..) => panic!("lossless re-encode of a lossy animated WebP must be skipped"),
    }
}

#[test]
fn resize_applies_to_every_frame() {
    for (src, f) in [("anim/animated.png", Format::Webp), ("anim/animated.webp", Format::Avif), ("gif/animated.gif", Format::Webp)] {
        let (p, b) = must_encode(src, &Options { format: f, max_dimension: Some(80), ..base() });
        assert_eq!(frame_count(&b, fmt_of(&p)), FRAMES, "{src}");
        let (w, h) = dims(&b, fmt_of(&p));
        assert!(w.max(h) <= 80 && w.max(h) >= 79, "{src}: {w}x{h}");
    }
    // APNG staying APNG cannot be resized yet: skipped, not silently ignored
    match go("anim/animated.png", &Options { max_dimension: Some(80), ..base() }) {
        Outcome::Skipped(r) => assert!(r.contains("animated")),
        Outcome::Out(..) => panic!("APNG resize must be skipped"),
    }
}

#[test]
fn delays_and_loop_survive_conversion() {
    let (_, b) = must_encode("anim/animated.png", &Options { format: Format::Webp, ..base() });
    let (fr, _) = compress_core::anim::decode(&b, Fmt::Webp).unwrap();
    assert_eq!(fr.loop_count, 0);
    assert!(fr.frames.iter().all(|(_, d)| *d == 100));
    let (_, b) = must_encode("anim/animated.webp", &Options { format: Format::Avif, ..base() });
    let (fr, _) = compress_core::anim::decode(&b, Fmt::Avif).unwrap();
    assert_eq!(fr.loop_count, 0);
    assert!(fr.frames.iter().all(|(_, d)| *d == 100));
}
