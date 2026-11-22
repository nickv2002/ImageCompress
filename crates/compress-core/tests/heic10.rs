#![cfg(target_os = "macos")]
use compress_core::heic_codec::{bit_depth, encode_16bit_source, encode16, quality_to_imageio, Rgba16};
use compress_core::{options::*, run};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

fn fixture(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures").join(rel)
}

fn to_heic(src: &Path, depth: u8) -> Vec<u8> {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join(src.file_name().unwrap());
    std::fs::copy(src, &p).unwrap();
    let o = Options { format: Format::Heic, heic_depth: depth, replace_if: ReplaceIf::Always, originals: Originals::Delete, history: false, jobs: 1, ..Default::default() };
    let out = run(&[p.clone()], &o, &AtomicBool::new(false), &|_| {});
    assert_eq!((out.processed, out.failed), (1, 0));
    std::fs::read(p.with_extension("heic")).unwrap()
}

/// Smooth 16-bit gradient: 8 bits cannot represent it without banding.
fn gradient16_png() -> Vec<u8> {
    let (w, h) = (640u32, 360u32);
    let img = image::ImageBuffer::<image::Rgb<u16>, _>::from_fn(w, h, |x, y| {
        let v = ((x as u64 * 65535 / (w as u64 - 1)) as u16 / 2).saturating_add((y as u64 * 8000 / h as u64) as u16);
        image::Rgb([v, v / 2 + 3000, 65535 - v])
    });
    let mut out = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png).unwrap();
    out
}

#[test]
fn depth_option_controls_hevc_bit_depth() {
    let jpg = fixture("jpeg/baseline.jpg");
    assert_eq!(bit_depth(&to_heic(&jpg, 8)), Some(8));
    assert_eq!(bit_depth(&to_heic(&jpg, 10)), Some(10));
    // 16-bit PNG source through the pipeline
    assert_eq!(bit_depth(&to_heic(&fixture("png/rgb16.png"), 10)), Some(10));
    assert_eq!(bit_depth(&to_heic(&fixture("png/rgb16.png"), 8)), Some(8));
}

#[test]
fn sixteen_bit_source_keeps_precision_and_is_smaller_than_promoted_8bit() {
    let png = gradient16_png();
    let o = Options { heic_depth: 10, ..Default::default() };
    let (b16, name) = encode_16bit_source(&png, image::ImageFormat::Png, &o).unwrap().expect("16-bit path taken");
    assert_eq!(name, "imageio-heic-16bit");
    assert_eq!(bit_depth(&b16), Some(10));
    // same image squeezed through 8-bit pixels, then promoted
    let rgba8 = image::load_from_memory(&png).unwrap().to_rgba8();
    let promoted = Rgba16::from_rgba8(&compress_core::pixels::Rgba { w: rgba8.width(), h: rgba8.height(), px: rgba8.into_raw() });
    let b8 = encode16(&promoted, quality_to_imageio(o.heic_quality), None, None).unwrap();
    assert_eq!(bit_depth(&b8), Some(10));
    assert!(b16.len() < b8.len(), "16-bit source {} B should beat promoted 8-bit {} B on a smooth gradient", b16.len(), b8.len());
}

#[test]
fn eight_bit_png_is_not_a_16bit_source_and_resize_falls_back() {
    let png8 = std::fs::read(fixture("png/rgba_photo.png")).unwrap();
    let o = Options { heic_depth: 10, ..Default::default() };
    assert!(encode_16bit_source(&png8, image::ImageFormat::Png, &o).unwrap().is_none());
    let o = Options { heic_depth: 10, max_dimension: Some(100), ..Default::default() };
    assert!(encode_16bit_source(&std::fs::read(fixture("png/rgb16.png")).unwrap(), image::ImageFormat::Png, &o).unwrap().is_none());
}

#[test]
fn alpha_survives_10bit() {
    let mut px = vec![0u16; 64 * 64 * 4];
    for (i, p) in px.chunks_exact_mut(4).enumerate() {
        p.copy_from_slice(&[40000, 20000, 60000, if i % 64 < 32 { 65535 } else { 20000 }]);
    }
    let b = encode16(&Rgba16 { w: 64, h: 64, px }, 0.8, None, None).unwrap();
    assert_eq!(bit_depth(&b), Some(10));
    let d = compress_core::convert::decode_any(&b, compress_core::convert::Fmt::Heic).unwrap();
    assert!(d.img.has_alpha());
}
