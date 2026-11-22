//! WebP via libwebp (static, vendored by libwebp-sys).
use crate::pixels::{CResult, Rgba};
use libwebp_sys as w;

/// quality 0..100 (lossy). `lossless` ignores quality for pixels. method 0..6. `alpha_quality` 0..100 (lossy alpha).
pub fn encode(im: &Rgba, quality: f32, lossless: bool, method: i32, alpha_quality: i32) -> CResult<Vec<u8>> {
    if im.w > 16383 || im.h > 16383 {
        return Err("image too large for WebP (max 16383 px per side)".into());
    }
    unsafe {
        let mut cfg: w::WebPConfig = std::mem::zeroed();
        if w::WebPConfigInitInternal(&mut cfg, w::WebPPreset::WEBP_PRESET_DEFAULT, quality, w::WEBP_ENCODER_ABI_VERSION as i32) == 0 {
            return Err("webp config init".into());
        }
        cfg.lossless = lossless as i32;
        cfg.method = method.clamp(0, 6);
        cfg.alpha_quality = alpha_quality.clamp(0, 100);
        if lossless {
            cfg.quality = 75.0; // effort for lossless
            cfg.exact = 1; // keep RGB under alpha==0
        }
        if w::WebPValidateConfig(&cfg) == 0 {
            return Err("bad webp config".into());
        }
        let mut pic: w::WebPPicture = std::mem::zeroed();
        if w::WebPPictureInitInternal(&mut pic, w::WEBP_ENCODER_ABI_VERSION as i32) == 0 {
            return Err("webp picture init".into());
        }
        pic.width = im.w as i32;
        pic.height = im.h as i32;
        pic.use_argb = lossless as i32;
        let ok = if im.has_alpha() {
            w::WebPPictureImportRGBA(&mut pic, im.px.as_ptr(), (im.w * 4) as i32)
        } else {
            let rgb = im.rgb3();
            w::WebPPictureImportRGB(&mut pic, rgb.as_ptr(), (im.w * 3) as i32)
        };
        if ok == 0 {
            w::WebPPictureFree(&mut pic);
            return Err("webp import".into());
        }
        let mut wr: w::WebPMemoryWriter = std::mem::zeroed();
        w::WebPMemoryWriterInit(&mut wr);
        pic.writer = Some(w::WebPMemoryWrite);
        pic.custom_ptr = &mut wr as *mut _ as *mut _;
        let ok = w::WebPEncode(&cfg, &mut pic);
        let ec = pic.error_code;
        w::WebPPictureFree(&mut pic);
        if ok == 0 {
            w::WebPMemoryWriterClear(&mut wr);
            return Err(format!("webp encode error {ec:?}"));
        }
        let v = std::slice::from_raw_parts(wr.mem, wr.size).to_vec();
        w::WebPMemoryWriterClear(&mut wr);
        Ok(v)
    }
}

/// Embed an ICC profile (RIFF ICCP chunk) into a WebP file.
pub fn set_icc(webp: Vec<u8>, icc: &[u8]) -> CResult<Vec<u8>> {
    use img_parts::{webp::WebP, ImageICC};
    let mut p = WebP::from_bytes(webp.into()).map_err(|e| e.to_string())?;
    p.set_icc_profile(Some(icc.to_vec().into()));
    let mut out = Vec::new();
    p.encoder().write_to(&mut out).map_err(|e| e.to_string())?;
    Ok(out)
}

/// Embed ICC / EXIF / XMP chunks with libwebp's mux (fixes the VP8X flags; works on animations too).
/// `exif` is the raw TIFF payload (the WebP EXIF chunk carries no `Exif\0\0` prefix).
pub fn embed(webp: Vec<u8>, icc: Option<&[u8]>, exif: Option<&[u8]>, xmp: Option<&[u8]>) -> CResult<Vec<u8>> {
    if icc.is_none() && exif.is_none() && xmp.is_none() {
        return Ok(webp);
    }
    unsafe {
        let src = w::WebPData { bytes: webp.as_ptr(), size: webp.len() };
        let mux = w::WebPMuxCreateInternal(&src, 1, w::WEBP_MUX_ABI_VERSION as i32);
        if mux.is_null() {
            return Err("webp mux: cannot parse encoder output".into());
        }
        let mut res = Ok(());
        for (fourcc, blob) in [(c"ICCP", icc), (c"EXIF", exif), (c"XMP ", xmp)] {
            if let Some(b) = blob {
                let d = w::WebPData { bytes: b.as_ptr(), size: b.len() };
                if w::WebPMuxSetChunk(mux, fourcc.as_ptr(), &d, 1) != w::WebPMuxError::WEBP_MUX_OK {
                    res = Err(format!("webp mux: cannot set {} chunk", fourcc.to_string_lossy()));
                    break;
                }
            }
        }
        let mut out = w::WebPData { bytes: std::ptr::null(), size: 0 };
        let res = res.and_then(|_| {
            if w::WebPMuxAssemble(mux, &mut out) == w::WebPMuxError::WEBP_MUX_OK {
                Ok(std::slice::from_raw_parts(out.bytes, out.size).to_vec())
            } else {
                Err("webp mux: assemble failed".to_string())
            }
        });
        w::WebPDataClear(&mut out);
        w::WebPMuxDelete(mux);
        res
    }
}

/// Raw EXIF TIFF payload and XMP packet of a WebP file (an `Exif\0\0` prefix some writers add is removed).
pub fn exif_xmp_of(data: &[u8]) -> (Option<Vec<u8>>, Option<Vec<u8>>) {
    use img_parts::webp::WebP;
    let Ok(p) = WebP::from_bytes(data.to_vec().into()) else { return (None, None) };
    let chunk = |id: &[u8; 4]| p.chunk_by_id(*id).and_then(|c| c.content().data().map(|b| b.to_vec()));
    let exif = chunk(b"EXIF").map(|e| e.strip_prefix(b"Exif\0\0").map(<[u8]>::to_vec).unwrap_or(e));
    (exif, chunk(b"XMP "))
}

pub fn icc_of(data: &[u8]) -> Option<Vec<u8>> {
    use img_parts::{webp::WebP, ImageICC};
    WebP::from_bytes(data.to_vec().into()).ok()?.icc_profile().map(|b| b.to_vec())
}

pub fn is_animated(data: &[u8]) -> bool {
    // VP8X header: bytes 12..16 == "VP8X", flags at 20; animation bit = 0x02
    data.len() > 21 && &data[12..16] == b"VP8X" && data[20] & 0x02 != 0
}

pub fn decode(data: &[u8]) -> CResult<Rgba> {
    unsafe {
        let (mut wd, mut ht) = (0, 0);
        if w::WebPGetInfo(data.as_ptr(), data.len(), &mut wd, &mut ht) == 0 {
            return Err("not a valid WebP".into());
        }
        let p = w::WebPDecodeRGBA(data.as_ptr(), data.len(), &mut wd, &mut ht);
        if p.is_null() {
            return Err("webp decode failed".into());
        }
        let v = std::slice::from_raw_parts(p, (wd * ht * 4) as usize).to_vec();
        w::WebPFree(p as *mut _);
        Ok(Rgba { w: wd as u32, h: ht as u32, px: v })
    }
}
