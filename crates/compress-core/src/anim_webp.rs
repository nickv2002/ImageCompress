//! Animated WebP: frames -> WebPAnimEncoder (the gif2webp pipeline), WebPAnimDecoder -> frames, GIF frame decode.
use crate::options::Options;
use crate::pixels::{CResult, Frames};
use image::AnimationDecoder;
use libwebp_sys as w;
use std::io::Cursor;

/// Number of frames in a (possibly animated) WebP, by demuxing. None if it is not a valid WebP.
pub fn frame_count(data: &[u8]) -> Option<u32> {
    unsafe {
        let wd = w::WebPData { bytes: data.as_ptr(), size: data.len() };
        let dmux = w::WebPDemuxInternal(&wd, 0, std::ptr::null_mut(), w::WEBP_DEMUX_ABI_VERSION as i32);
        if dmux.is_null() {
            return None;
        }
        let n = w::WebPDemuxGetI(dmux, w::WebPFormatFeature::WEBP_FF_FRAME_COUNT);
        w::WebPDemuxDelete(dmux);
        Some(n)
    }
}

/// NETSCAPE2.0 loop count (0 = forever). Absent extension means "play once" (WebP loop_count 1).
pub fn gif_loop_count(data: &[u8]) -> i32 {
    const SIG: &[u8] = b"NETSCAPE2.0";
    match data.windows(SIG.len()).position(|x| x == SIG) {
        Some(p) => match data.get(p + SIG.len()..p + SIG.len() + 4) {
            Some([3, 1, lo, hi]) => u16::from_le_bytes([*lo, *hi]) as i32,
            _ => 0,
        },
        None => 1,
    }
}

pub fn decode_gif(data: &[u8]) -> CResult<Frames> {
    let dec = image::codecs::gif::GifDecoder::new(Cursor::new(data)).map_err(|e| e.to_string())?;
    let mut out = Frames { w: 0, h: 0, frames: vec![], loop_count: gif_loop_count(data) };
    for f in dec.into_frames() {
        let f = f.map_err(|e| e.to_string())?;
        let (n, d) = f.delay().numer_denom_ms();
        let ms = if d == 0 { 0 } else { (n / d) as i32 };
        let buf = f.into_buffer();
        out.w = buf.width();
        out.h = buf.height();
        out.frames.push((buf.into_raw(), if ms == 0 { 100 } else { ms }));
    }
    if out.frames.is_empty() {
        return Err("gif has no frames".into());
    }
    Ok(out)
}

/// Encode GIF bytes to animated WebP. Lossless when `o.webp_quality` is None, else lossy at that
/// quality (alpha stays lossless-ish via `webp_alpha_quality`).
pub fn gif_to_webp(data: &[u8], o: &Options) -> CResult<Vec<u8>> {
    encode_frames(&decode_gif(data)?, o)
}

/// Encode already decoded frames (any source) as an animated WebP.
pub fn encode_frames(g: &Frames, o: &Options) -> CResult<Vec<u8>> {
    let loop_count = g.loop_count;
    if g.w > 16383 || g.h > 16383 {
        return Err("image too large for WebP".into());
    }
    let lossless = o.webp_quality.is_none();
    let quality = o.webp_quality.unwrap_or(100) as f32;
    unsafe {
        let mut cfg: w::WebPConfig = std::mem::zeroed();
        if w::WebPConfigInitInternal(&mut cfg, w::WebPPreset::WEBP_PRESET_DEFAULT, quality, w::WEBP_ENCODER_ABI_VERSION as i32) == 0 {
            return Err("webp config init".into());
        }
        cfg.lossless = lossless as i32;
        cfg.method = (o.webp_method as i32).clamp(0, 6);
        cfg.alpha_quality = (o.webp_alpha_quality as i32).clamp(0, 100);
        if lossless {
            cfg.quality = 75.0; // effort
            cfg.exact = 1;
        }
        if w::WebPValidateConfig(&cfg) == 0 {
            return Err("bad webp config".into());
        }
        let mut eo: w::WebPAnimEncoderOptions = std::mem::zeroed();
        if w::WebPAnimEncoderOptionsInitInternal(&mut eo, w::WEBP_MUX_ABI_VERSION as i32) == 0 {
            return Err("anim options init".into());
        }
        eo.anim_params.loop_count = loop_count;
        eo.minimize_size = 1;
        eo.allow_mixed = (!lossless) as i32;
        if lossless {
            eo.kmin = 9;
            eo.kmax = 17;
        } else {
            eo.kmin = 3;
            eo.kmax = 5;
        }
        let enc = w::WebPAnimEncoderNewInternal(g.w as i32, g.h as i32, &eo, w::WEBP_MUX_ABI_VERSION as i32);
        if enc.is_null() {
            return Err("anim encoder alloc".into());
        }
        let res = (|| -> CResult<Vec<u8>> {
            let mut ts = 0i32;
            for (px, delay) in &g.frames {
                let mut pic: w::WebPPicture = std::mem::zeroed();
                if w::WebPPictureInitInternal(&mut pic, w::WEBP_ENCODER_ABI_VERSION as i32) == 0 {
                    return Err("picture init".into());
                }
                pic.width = g.w as i32;
                pic.height = g.h as i32;
                pic.use_argb = 1;
                if w::WebPPictureImportRGBA(&mut pic, px.as_ptr(), (g.w * 4) as i32) == 0 {
                    w::WebPPictureFree(&mut pic);
                    return Err("picture import".into());
                }
                let ok = w::WebPAnimEncoderAdd(enc, &mut pic, ts, &cfg);
                w::WebPPictureFree(&mut pic);
                if ok == 0 {
                    let m = std::ffi::CStr::from_ptr(w::WebPAnimEncoderGetError(enc)).to_string_lossy().into_owned();
                    return Err(format!("anim add: {m}"));
                }
                ts += delay;
            }
            if w::WebPAnimEncoderAdd(enc, std::ptr::null_mut(), ts, std::ptr::null()) == 0 {
                return Err("anim flush".into());
            }
            let mut wd: w::WebPData = std::mem::zeroed();
            w::WebPDataInit(&mut wd);
            if w::WebPAnimEncoderAssemble(enc, &mut wd) == 0 {
                let m = std::ffi::CStr::from_ptr(w::WebPAnimEncoderGetError(enc)).to_string_lossy().into_owned();
                return Err(format!("anim assemble: {m}"));
            }
            let v = std::slice::from_raw_parts(wd.bytes, wd.size).to_vec();
            w::WebPDataClear(&mut wd);
            Ok(v)
        })();
        w::WebPAnimEncoderDelete(enc);
        res
    }
}

/// Decode an animated WebP to full-canvas RGBA frames (libwebp's WebPAnimDecoder composites and disposes).
pub fn decode_webp(data: &[u8]) -> CResult<Frames> {
    unsafe {
        let wd = w::WebPData { bytes: data.as_ptr(), size: data.len() };
        let mut opts: w::WebPAnimDecoderOptions = std::mem::zeroed();
        if w::WebPAnimDecoderOptionsInit(&mut opts) == 0 {
            return Err("webp anim decoder options".into());
        }
        opts.color_mode = w::WEBP_CSP_MODE::MODE_RGBA;
        opts.use_threads = 0;
        let dec = w::WebPAnimDecoderNew(&wd, &opts);
        if dec.is_null() {
            return Err("animated webp does not decode".into());
        }
        let res = (|| -> CResult<Frames> {
            let mut info: w::WebPAnimInfo = std::mem::zeroed();
            if w::WebPAnimDecoderGetInfo(dec, &mut info) == 0 {
                return Err("webp anim info".into());
            }
            let (cw, ch) = (info.canvas_width, info.canvas_height);
            let mut out = Frames { w: cw, h: ch, frames: vec![], loop_count: info.loop_count as i32 };
            let mut prev = 0i32;
            while w::WebPAnimDecoderHasMoreFrames(dec) != 0 {
                let mut buf: *mut u8 = std::ptr::null_mut();
                let mut ts = 0i32;
                if w::WebPAnimDecoderGetNext(dec, &mut buf, &mut ts) == 0 || buf.is_null() {
                    return Err("webp animation frame failed to decode".into());
                }
                let px = std::slice::from_raw_parts(buf, cw as usize * ch as usize * 4).to_vec();
                out.frames.push((px, (ts - prev).max(1)));
                prev = ts;
            }
            if out.frames.len() as u32 != info.frame_count {
                return Err(format!("animated webp: decoded {} of {} frames", out.frames.len(), info.frame_count));
            }
            Ok(out)
        })();
        w::WebPAnimDecoderDelete(dec);
        res
    }
}

/// Animation-aware verification used by the pipeline: demux must succeed and the frame count match.
pub fn verify(webp: &[u8], expect_frames: usize) -> Result<(), String> {
    match frame_count(webp) {
        // libwebp merges consecutive identical frames (extending the duration), so fewer is legal
        Some(n) if n >= 1 && n as usize <= expect_frames => Ok(()),
        Some(n) => Err(format!("animated webp has {n} frames, expected at most {expect_frames}")),
        None => Err("animated webp does not demux".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gif() -> Vec<u8> {
        std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/gif/animated.gif")).unwrap()
    }

    #[test]
    fn frame_count_matches_gif() {
        let g = decode_gif(&gif()).unwrap();
        assert!(g.frames.len() > 1);
        for lossless in [false, true] {
            let mut o = Options::default();
            if lossless {
                o.webp_quality = None;
            }
            let b = gif_to_webp(&gif(), &o).unwrap();
            verify(&b, g.frames.len()).unwrap();
            assert_eq!(frame_count(&b).unwrap() as usize, g.frames.len(), "distinct frames are all kept");
            assert!(crate::webp_codec::is_animated(&b));
        }
    }
}
