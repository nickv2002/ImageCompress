//! Animation awareness: how many frames an input has, what the pipeline may do with it, and the
//! frame-level decode/encode paths (GIF, APNG, animated WebP, animated AVIF -> animated WebP / AVIF).
//! Rule: an animated input is never sent through a still decoder/encoder (that silently keeps frame 1).
use crate::convert::Fmt;
use crate::options::{AnimTo, Options};
use crate::pixels::{err, CResult, Decoded, Frames, Rgba};
use std::io::Cursor;

/// Skip reason for animated inputs the requested operation cannot handle.
pub const UNSUPPORTED: &str = "animated; not supported for this operation";

/// `(num_frames, num_plays)` from the `acTL` chunk (APNG); `None` for a plain PNG.
fn actl(data: &[u8]) -> Option<(u32, u32)> {
    // chunks: len(4) type(4) data crc(4); acTL must precede the first IDAT
    let mut i = 8;
    while let (Some(len), Some(ty)) = (data.get(i..i + 4), data.get(i + 4..i + 8)) {
        let len = u32::from_be_bytes(len.try_into().unwrap()) as usize;
        match ty {
            b"acTL" => {
                let n = u32::from_be_bytes(data.get(i + 8..i + 12)?.try_into().ok()?);
                let p = u32::from_be_bytes(data.get(i + 12..i + 16)?.try_into().ok()?);
                return Some((n, p));
            }
            b"IDAT" | b"IEND" => return None,
            _ => {}
        }
        i = i.saturating_add(12).saturating_add(len);
    }
    None
}

/// Frames announced by the `acTL` chunk (APNG); 1 for a plain PNG.
pub fn png_frames(data: &[u8]) -> u32 {
    actl(data).map_or(1, |(n, _)| n.max(1))
}

fn gif_frames(data: &[u8]) -> u32 {
    use image::AnimationDecoder;
    match image::codecs::gif::GifDecoder::new(Cursor::new(data)) {
        Ok(d) => d.into_frames().take_while(|f| f.is_ok()).count().max(1) as u32,
        Err(_) => 1,
    }
}

/// Number of frames in `data` (1 for stills). Cheap where the container allows (PNG, WebP, AVIF headers);
/// JXL reports 2 for "animated" without counting.
pub fn frame_count(data: &[u8], fmt: Fmt) -> u32 {
    match fmt {
        Fmt::Png => png_frames(data),
        Fmt::Gif => gif_frames(data),
        Fmt::Webp if crate::webp_codec::is_animated(data) => crate::anim_webp::frame_count(data).unwrap_or(2).max(1),
        Fmt::Avif => crate::avif_codec::frame_count(data),
        Fmt::Jxl => {
            if crate::jxl_codec::is_animated(data) {
                2
            } else {
                1
            }
        }
        #[cfg(target_os = "macos")]
        Fmt::Heic => crate::heic_codec::frame_count(data) as u32,
        _ => 1,
    }
}

/// APNG -> composited full-canvas RGBA frames (the `image` crate applies dispose/blend ops).
pub fn decode_apng(data: &[u8]) -> CResult<Frames> {
    use image::AnimationDecoder;
    let dec = image::codecs::png::PngDecoder::new(Cursor::new(data)).map_err(err)?;
    let (n, plays) = actl(data).ok_or("not an APNG")?;
    let mut out = Frames { w: 0, h: 0, frames: vec![], loop_count: plays as i32 };
    for f in dec.apng().map_err(err)?.into_frames() {
        let f = f.map_err(err)?;
        let (num, den) = f.delay().numer_denom_ms();
        let ms = if den == 0 { 0 } else { (num / den) as i32 };
        let buf = f.into_buffer();
        (out.w, out.h) = (buf.width(), buf.height());
        out.frames.push((buf.into_raw(), if ms <= 0 { 100 } else { ms }));
    }
    if out.frames.len() != n as usize {
        return Err(format!("APNG: decoded {} of {n} frames", out.frames.len()));
    }
    Ok(out)
}

/// Decode all frames of an animated GIF / APNG / WebP / AVIF. Also returns the ICC profile when the source has one.
pub fn decode(data: &[u8], fmt: Fmt) -> CResult<(Frames, Option<Vec<u8>>)> {
    match fmt {
        Fmt::Gif => Ok((crate::anim_webp::decode_gif(data)?, None)),
        Fmt::Png => Ok((decode_apng(data)?, None)),
        Fmt::Webp => Ok((crate::anim_webp::decode_webp(data)?, crate::webp_codec::icc_of(data))),
        Fmt::Avif => crate::avif_codec::decode_sequence(data),
        _ => Err(UNSUPPORTED.into()),
    }
}

/// What the pipeline does with an animated input.
pub enum Route {
    /// Optimise in the source format with its own tool (gifsicle, oxipng): the frames survive untouched.
    Native,
    Skip(String),
    /// Re-encoded as an animated WebP / AVIF.
    Encoded { bytes: Vec<u8>, ext: &'static str, encoder: String },
}

/// Decide and (for re-encodes) perform the work for an animated `src` input.
pub fn route(data: &[u8], src: Fmt, o: &Options, stage: &dyn Fn(&str)) -> Result<Route, String> {
    let target = match Fmt::from_format(o.format) {
        Some(t) => t,
        None => match o.anim_to {
            AnimTo::Keep => src,
            AnimTo::Webp => Fmt::Webp,
            AnimTo::Avif => Fmt::Avif,
        },
    };
    let skip = |r: &str| Ok(Route::Skip(r.to_string()));
    match (src, target) {
        (Fmt::Gif, Fmt::Gif) => Ok(Route::Native),
        (Fmt::Png, Fmt::Png) => {
            // oxipng keeps frames; resize / sRGB conversion would need an APNG re-encoder
            if crate::extras::pixel_changes_requested(data, src, o) {
                return skip(UNSUPPORTED);
            }
            Ok(Route::Native)
        }
        (Fmt::Webp, Fmt::Webp) if o.webp_quality.is_none() => skip("animated WebP: a lossless re-encode would grow the file; set a lossy webp quality"),
        (Fmt::Gif | Fmt::Png | Fmt::Webp | Fmt::Avif, Fmt::Webp | Fmt::Avif) => {
            stage("decode");
            let (mut fr, mut icc) = decode(data, src)?;
            if o.convert_to_srgb && icc.as_deref().is_some_and(|i| !crate::color::is_srgb_equivalent(i)) {
                for (px, _) in &mut fr.frames {
                    let mut d = Decoded::new(Rgba { w: fr.w, h: fr.h, px: std::mem::take(px) }, icc.clone());
                    crate::color::convert_to_srgb(&mut d)?;
                    *px = d.img.px;
                }
                icc = None;
            }
            if o.metadata == crate::options::Metadata::None || (o.compact_icc && icc.as_deref().is_some_and(crate::color::is_srgb_equivalent)) {
                icc = None;
            }
            if let Some((nw, nh)) = crate::resize::target_size(fr.w, fr.h, o) {
                stage("resize");
                for (px, _) in &mut fr.frames {
                    *px = crate::resize::resize_rgba(&Rgba { w: fr.w, h: fr.h, px: std::mem::take(px) }, nw, nh)?.px;
                }
                (fr.w, fr.h) = (nw, nh);
            }
            stage("encode");
            if target == Fmt::Webp {
                let mut b = crate::anim_webp::encode_frames(&fr, o)?;
                if let Some(icc) = &icc {
                    b = crate::webp_codec::set_icc(b, icc)?;
                }
                Ok(Route::Encoded { bytes: b, ext: "webp", encoder: "libwebp-anim".into() })
            } else {
                let q = o.avif_quality as i32;
                let b = crate::avif_codec::encode_sequence(&fr, &crate::avif_codec::EncOpts {
                    quality: q,
                    alpha_quality: o.avif_alpha_quality.map(|v| v as i32).unwrap_or(q),
                    speed: o.avif_speed as i32,
                    depth: if o.avif_depth >= 10 { 10 } else { 8 },
                    threads: 0,
                    icc,
                    exif: None,
                    xmp: None,
                })?;
                Ok(Route::Encoded { bytes: b, ext: "avif", encoder: "libaom-anim".into() })
            }
        }
        // JXL / HEIC sources (no frame decoder), and jpeg / png / jxl / heic targets (no animated encoder)
        _ => skip(UNSUPPORTED),
    }
}

/// Check that a finished animated output still has the frames of the input. `Err` aborts before the original is touched.
pub fn verify(out: &[u8], fmt: Fmt, frames_in: u32) -> Result<(), String> {
    let n = match fmt {
        Fmt::Webp => {
            // libwebp merges consecutive identical frames (extending the delay), so fewer is legal
            let n = crate::anim_webp::decode_webp(out).map_err(|e| format!("output failed verification: {e}"))?.frames.len() as u32;
            if n >= 1 && n <= frames_in {
                return Ok(());
            }
            n
        }
        Fmt::Avif => {
            let n = crate::avif_codec::decode_sequence(out).map_err(|e| format!("output failed verification: {e}"))?.0.frames.len() as u32;
            n
        }
        Fmt::Png => decode_apng(out).map_err(|e| format!("output failed verification: {e}"))?.frames.len() as u32,
        _ => frame_count(out, fmt),
    };
    if n == frames_in {
        Ok(())
    } else {
        Err(format!("output failed verification: {n} frames, input had {frames_in}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fx(rel: &str) -> Vec<u8> {
        std::fs::read(format!("{}/../../fixtures/{rel}", env!("CARGO_MANIFEST_DIR"))).unwrap()
    }

    #[test]
    fn counts_frames_of_every_animated_fixture() {
        assert_eq!(frame_count(&fx("anim/animated.png"), Fmt::Png), 8);
        assert_eq!(frame_count(&fx("anim/animated.webp"), Fmt::Webp), 8);
        assert_eq!(frame_count(&fx("anim/animated.avif"), Fmt::Avif), 8);
        assert!(frame_count(&fx("anim/animated.jxl"), Fmt::Jxl) > 1);
        assert_eq!(frame_count(&fx("gif/animated.gif"), Fmt::Gif), 8);
        #[cfg(target_os = "macos")]
        assert!(frame_count(&fx("anim/sequence.heic"), Fmt::Heic) > 1);
    }

    #[test]
    fn stills_count_one() {
        assert_eq!(frame_count(&fx("png/palette.png"), Fmt::Png), 1);
        assert_eq!(frame_count(&fx("gif/static.gif"), Fmt::Gif), 1);
        assert_eq!(frame_count(&fx("modern/photo.webp"), Fmt::Webp), 1);
        assert_eq!(frame_count(&fx("modern/photo.avif"), Fmt::Avif), 1);
        assert_eq!(frame_count(&fx("modern/photo.jxl"), Fmt::Jxl), 1);
        #[cfg(target_os = "macos")]
        assert_eq!(frame_count(&fx("modern/photo.heic"), Fmt::Heic), 1);
    }

    #[test]
    fn decoders_return_every_frame_with_timing() {
        for (f, fmt) in [("anim/animated.png", Fmt::Png), ("anim/animated.webp", Fmt::Webp), ("anim/animated.avif", Fmt::Avif), ("gif/animated.gif", Fmt::Gif)] {
            let (fr, _) = decode(&fx(f), fmt).unwrap();
            assert_eq!(fr.frames.len(), 8, "{f}");
            assert!(fr.w > 0 && fr.frames.iter().all(|(px, d)| px.len() == (fr.w * fr.h * 4) as usize && *d > 0), "{f}");
        }
        let (fr, _) = decode(&fx("anim/animated.png"), Fmt::Png).unwrap();
        assert!(fr.frames.iter().all(|(_, d)| *d == 100), "apng delays");
        assert_eq!(fr.loop_count, 0, "apng loops forever");
        let (fr, _) = decode(&fx("anim/animated.avif"), Fmt::Avif).unwrap();
        assert!(fr.frames.iter().all(|(_, d)| *d == 100), "avif delays");
    }

    #[test]
    fn avif_sequence_roundtrip_keeps_frames_timing_and_alpha() {
        let (mut fr, _) = decode(&fx("anim/animated.png"), Fmt::Png).unwrap();
        fr.loop_count = 3;
        fr.frames[2].1 = 250;
        // make one frame translucent so the sequence needs an alpha plane
        fr.frames[1].0.chunks_exact_mut(4).for_each(|p| p[3] = 128);
        let o = crate::avif_codec::EncOpts { quality: 60, alpha_quality: 60, speed: 9, depth: 8, threads: 0, icc: None, exif: None, xmp: None };
        let b = crate::avif_codec::encode_sequence(&fr, &o).unwrap();
        let (back, _) = crate::avif_codec::decode_sequence(&b).unwrap();
        assert_eq!(back.frames.len(), 8);
        assert_eq!(back.frames[2].1, 250);
        assert_eq!(back.loop_count, 3);
        assert!((back.frames[1].0[3] as i32 - 128).abs() < 20, "alpha kept, got {}", back.frames[1].0[3]);
        assert_eq!(back.frames[0].0[3], 255);
        assert_eq!(frame_count(&b, Fmt::Avif), 8);
    }

    #[test]
    fn webp_roundtrip_keeps_loop_and_delays() {
        let (mut fr, _) = decode(&fx("anim/animated.png"), Fmt::Png).unwrap();
        fr.loop_count = 2;
        fr.frames[3].1 = 300;
        let b = crate::anim_webp::encode_frames(&fr, &Options::default()).unwrap();
        let back = crate::anim_webp::decode_webp(&b).unwrap();
        assert_eq!(back.frames.len(), 8);
        assert_eq!(back.loop_count, 2);
        assert_eq!(back.frames[3].1, 300);
    }
}
