//! EXIF / XMP / ICC carried across conversions, verified with exiftool (tests are skipped when
//! exiftool is not installed: `brew install exiftool`).
use compress_core::{options::*, run};
use serde_json::{Map, Value};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::AtomicBool;

fn fixture(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures").join(rel)
}

fn convert_to(src: &Path, format: Format, metadata: Metadata, ext: &str) -> PathBuf {
    let o = Options {
        format,
        metadata,
        replace_if: ReplaceIf::Always,
        originals: Originals::Keep,
        history: false,
        jobs: 1,
        avif_speed: 9,
        webp_method: 0,
        webp_quality: Some(80),
        jxl_quality: Some(80),
        jxl_effort: 3,
        jxl_jpeg_transcode: false,
        ..Default::default()
    };
    let d = tempfile::tempdir().unwrap().keep();
    let p = d.join(src.file_name().unwrap());
    std::fs::copy(src, &p).unwrap();
    let out = run(&[p.clone()], &o, &AtomicBool::new(false), &|_| {});
    assert_eq!((out.processed, out.failed), (1, 0), "conversion of {} to {ext}", src.display());
    p.with_extension(ext)
}

fn have_exiftool() -> bool {
    Command::new("exiftool").arg("-ver").output().is_ok_and(|o| o.status.success())
}

/// `exiftool -j -G1 -a -s -n`: "Group:Tag" -> value (numeric where possible).
fn tags(p: &Path) -> Map<String, Value> {
    let out = Command::new("exiftool").args(["-j", "-G1", "-a", "-s", "-n"]).arg(p).output().expect("exiftool");
    assert!(out.status.success(), "exiftool failed on {}: {}", p.display(), String::from_utf8_lossy(&out.stderr));
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    v[0].as_object().unwrap().clone()
}

fn has(t: &Map<String, Value>, needle: &str) -> bool {
    t.keys().any(|k| k.contains(needle))
}

/// The EXIF fixture (Make, GPS, thumbnail, Orientation 6, 1000x667) plus an XMP title, made with exiftool.
fn exif_xmp_source() -> PathBuf {
    let out = tempfile::tempdir().unwrap().keep().join("meta_src.jpg");
    let st = Command::new("exiftool")
        .args(["-q", "-XMP-dc:Title=Fixture Title", "-XMP-xmp:Rating=3", "-o"])
        .arg(&out)
        .arg(fixture("jpeg/exif_gps_orient6.jpg"))
        .status()
        .unwrap();
    assert!(st.success() && out.exists());
    out
}

/// Targets exercised for metadata; HEIC only where ImageIO exists.
fn targets() -> Vec<(Format, &'static str)> {
    let mut v = vec![(Format::Webp, "webp"), (Format::Avif, "avif"), (Format::Jxl, "jxl")];
    if cfg!(target_os = "macos") {
        v.push((Format::Heic, "heic"));
    }
    v
}

fn run_target(src: &Path, f: Format, ext: &str, m: Metadata) -> Map<String, Value> {
    tags(&convert_to(src, f, m, ext))
}

/// Pixels were rotated upright (the source is 1000x667 tagged orientation 6) and no tag says otherwise.
fn assert_upright(t: &Map<String, Value>, ext: &str) {
    let ori = t.iter().find(|(k, _)| k.ends_with(":Orientation")).map(|(_, v)| v.as_u64().unwrap());
    assert!(ori.is_none() || ori == Some(1), "{ext}: orientation must be neutral, got {ori:?}");
    let dim = |k: &str| t.get(k).and_then(Value::as_u64);
    // JXL/HEIC: exiftool reports the composite ImageSize; others have File:/Composite: dimensions
    let size = t.get("Composite:ImageSize").and_then(Value::as_str).map(str::to_owned).unwrap_or_else(|| format!("{}x{}", dim("File:ImageWidth").unwrap_or(0), dim("File:ImageHeight").unwrap_or(0)));
    // HEIC's ispe is padded to an even width (668); the clean aperture keeps 667
    let ok = |s: &str| ["667x1000", "667 1000", "668x1000", "668 1000"].contains(&s);
    assert!(ok(&size), "{ext}: expected a 667x1000 portrait, got {size}");
}

#[test]
fn all_carries_exif_xmp_and_gps() {
    if !have_exiftool() {
        return;
    }
    let src = exif_xmp_source();
    for (f, ext) in targets() {
        let t = run_target(&src, f, ext, Metadata::All);
        assert_eq!(t["IFD0:Make"], "FixtureCam", "{ext}");
        assert!(has(&t, "GPSLatitude"), "{ext}: GPS kept by metadata=all");
        assert_eq!(t["XMP-dc:Title"], "Fixture Title", "{ext}");
        assert_upright(&t, ext);
    }
}

#[test]
fn exif_no_gps_drops_gps_keeps_the_rest() {
    if !have_exiftool() {
        return;
    }
    let src = exif_xmp_source();
    for (f, ext) in targets() {
        let t = run_target(&src, f, ext, Metadata::ExifNoGps);
        assert_eq!(t["IFD0:Make"], "FixtureCam", "{ext}");
        assert!(!has(&t, "GPS"), "{ext}: GPS must be gone: {:?}", t.keys().filter(|k| k.contains("GPS")).collect::<Vec<_>>());
        assert_eq!(t["XMP-dc:Title"], "Fixture Title", "{ext}");
        assert!(!has(&t, "ThumbnailImage") && !has(&t, "ThumbnailOffset"), "{ext}: thumbnail stripped");
        assert_upright(&t, ext);
    }
}

/// Metadata is re-read from WebP / JXL / AVIF sources (not just JPEG) and carried on.
#[test]
fn chained_conversions_keep_metadata_and_gps_policy_applies_per_hop() {
    if !have_exiftool() {
        return;
    }
    let mut p = exif_xmp_source();
    for (f, ext) in [(Format::Webp, "webp"), (Format::Jxl, "jxl"), (Format::Avif, "avif"), (Format::Webp, "webp")] {
        p = convert_to(&p, f, Metadata::All, ext);
        let t = tags(&p);
        assert_eq!(t["IFD0:Make"], "FixtureCam", "hop to {ext}");
        assert!(has(&t, "GPSLatitude"), "hop to {ext}: gps kept");
        assert_eq!(t["XMP-dc:Title"], "Fixture Title", "hop to {ext}");
        assert_upright(&t, ext);
    }
    // final hop from the AVIF-derived WebP chain with the privacy policy
    for (f, ext) in [(Format::Jxl, "jxl"), (Format::Avif, "avif")] {
        let t = tags(&convert_to(&p, f, Metadata::ExifNoGps, ext));
        assert_eq!(t["IFD0:Make"], "FixtureCam", "{ext}");
        assert!(!has(&t, "GPS"), "{ext}: gps dropped");
    }
}

#[test]
fn icc_and_none_policies() {
    if !have_exiftool() {
        return;
    }
    let src = exif_xmp_source();
    let p3 = fixture("jpeg/display_p3.jpg");
    for (f, ext) in targets() {
        let t = run_target(&src, f, ext, Metadata::Icc);
        assert!(!has(&t, "Make") && !has(&t, "XMP-dc") && !has(&t, "GPS"), "{ext}: metadata=icc carries no EXIF/XMP");

        let t = run_target(&p3, f, ext, Metadata::Icc);
        // lossy JXL stores a recognised P3 as an enumerated colour encoding (no ICC blob for exiftool to
        // show); that path is covered by tests/metadata.rs
        if ext != "jxl" {
            let desc = t.get("ICC_Profile:ProfileDescription").and_then(Value::as_str).unwrap_or("");
            assert!(desc.contains("P3"), "{ext}: Display P3 profile kept, got {desc:?}");
        }

        let t = run_target(&p3, f, ext, Metadata::None);
        assert!(!has(&t, "ICC_Profile"), "{ext}: metadata=none drops the profile");
        let t = run_target(&src, f, ext, Metadata::None);
        assert!(!has(&t, "Make") && !has(&t, "XMP") && !has(&t, "GPS") && !has(&t, "ICC_Profile"), "{ext}: metadata=none leaves no metadata");
    }
}
