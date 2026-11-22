//! Codec spike: exercises jpegxl-rs, libavif-sys(+aom), ravif, libwebp-sys, ImageIO HEIC.
//! Subcommands print `bytes=<n> ms=<t>` so a shell harness can score with ssimulacra2.
mod avif;
mod heic;
mod jxlc;
mod webp;

use anyhow::{bail, Result};
use std::{fs, path::Path, time::Instant};

/// Decoded 8-bit RGBA buffer, tightly packed.
pub struct Rgba {
    pub w: u32,
    pub h: u32,
    pub px: Vec<u8>,
}

pub fn load(p: &str) -> Result<Rgba> {
    let im = image::open(p)?.to_rgba8();
    Ok(Rgba { w: im.width(), h: im.height(), px: im.into_raw() })
}

pub fn save_png(p: &str, im: &Rgba) -> Result<()> {
    image::save_buffer(p, &im.px, im.w, im.h, image::ColorType::Rgba8)?;
    Ok(())
}

/// Opaque if all alpha == 255 (lets encoders skip the alpha channel).
pub fn has_alpha(im: &Rgba) -> bool {
    im.px.chunks_exact(4).any(|p| p[3] != 255)
}

fn rgb_only(im: &Rgba) -> Vec<u8> {
    im.px.chunks_exact(4).flat_map(|p| [p[0], p[1], p[2]]).collect()
}
pub(crate) use rgb_only as rgb3;

fn main() -> Result<()> {
    let a: Vec<String> = std::env::args().collect();
    let cmd = a.get(1).map(String::as_str).unwrap_or("");
    let t = Instant::now();
    let out_bytes: Option<usize> = match cmd {
        // jxl-lossy in out distance effort
        "jxl-lossy" => {
            let im = load(&a[2])?;
            let b = jxlc::encode_lossy(&im, a[4].parse()?, a[5].parse()?)?;
            fs::write(&a[3], &b)?;
            Some(b.len())
        }
        "jxl-lossless" => {
            let im = load(&a[2])?;
            let b = jxlc::encode_lossless(&im, a[4].parse()?)?;
            fs::write(&a[3], &b)?;
            Some(b.len())
        }
        "jxl-jpeg" => {
            let b = jxlc::transcode_jpeg(&fs::read(&a[2])?)?;
            fs::write(&a[3], &b)?;
            Some(b.len())
        }
        "jxl-recon" => {
            let b = jxlc::reconstruct_jpeg(&fs::read(&a[2])?)?;
            fs::write(&a[3], &b)?;
            Some(b.len())
        }
        "jxl-dec" => {
            save_png(&a[3], &jxlc::decode_libjxl(&fs::read(&a[2])?)?)?;
            None
        }
        "jxl-dec-rs" => {
            save_png(&a[3], &jxlc::decode_jxl_rs(&fs::read(&a[2])?)?)?;
            None
        }
        // avif-enc in out quality speed depth
        "avif-enc" => {
            let im = load(&a[2])?;
            let b = avif::encode(&im, a[4].parse()?, a[5].parse()?, a[6].parse()?, a.get(7).and_then(|s| s.parse().ok()).unwrap_or(0))?;
            fs::write(&a[3], &b)?;
            Some(b.len())
        }
        "avif-dec" => {
            save_png(&a[3], &avif::decode(&fs::read(&a[2])?)?)?;
            None
        }
        // ravif in out quality speed
        "ravif" => {
            let im = load(&a[2])?;
            let b = avif::encode_ravif(&im, a[4].parse()?, a[5].parse()?)?;
            fs::write(&a[3], &b)?;
            Some(b.len())
        }
        // webp-enc in out quality(0-100) [lossless=1]
        "webp-enc" => {
            let im = load(&a[2])?;
            let b = webp::encode(&im, a[4].parse()?, a.get(5).map(|s| s == "1").unwrap_or(false), 4)?;
            fs::write(&a[3], &b)?;
            Some(b.len())
        }
        "webp-dec" => {
            save_png(&a[3], &webp::decode(&fs::read(&a[2])?)?)?;
            None
        }
        "heic-enc" => {
            let im = load(&a[2])?;
            let b = heic::encode(&im, a[4].parse()?)?;
            fs::write(&a[3], &b)?;
            Some(b.len())
        }
        "heic-dec" => {
            save_png(&a[3], &heic::decode(&fs::read(&a[2])?)?)?;
            None
        }
        "versions" => {
            println!("{}", avif::versions());
            None
        }
        _ => bail!("unknown cmd {cmd}"),
    };
    let _ = Path::new("");
    println!("bytes={} ms={}", out_bytes.map(|b| b.to_string()).unwrap_or("-".into()), t.elapsed().as_millis());
    Ok(())
}
