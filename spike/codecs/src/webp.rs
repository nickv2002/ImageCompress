use crate::{has_alpha, rgb3, Rgba};
use anyhow::{bail, Result};
use libwebp_sys as w;

/// quality 0..100 (lossy) ; `lossless` ignores quality for pixels but uses it as effort-ish (z). method 0..6.
pub fn encode(im: &Rgba, quality: f32, lossless: bool, method: i32) -> Result<Vec<u8>> {
    unsafe {
        let mut cfg: w::WebPConfig = std::mem::zeroed();
        if w::WebPConfigInitInternal(&mut cfg, w::WebPPreset::WEBP_PRESET_DEFAULT, quality, w::WEBP_ENCODER_ABI_VERSION as i32) == 0 {
            bail!("config init");
        }
        cfg.lossless = lossless as i32;
        cfg.method = method;
        if lossless {
            cfg.quality = 75.0; // effort for lossless (0 fast .. 100 slow)
            cfg.exact = 1; // keep RGB under alpha==0 (bit-exact)
        }
        if w::WebPValidateConfig(&cfg) == 0 {
            bail!("bad config");
        }
        let mut pic: w::WebPPicture = std::mem::zeroed();
        if w::WebPPictureInitInternal(&mut pic, w::WEBP_ENCODER_ABI_VERSION as i32) == 0 {
            bail!("pic init");
        }
        pic.width = im.w as i32;
        pic.height = im.h as i32;
        pic.use_argb = lossless as i32;
        let ok = if has_alpha(im) {
            w::WebPPictureImportRGBA(&mut pic, im.px.as_ptr(), (im.w * 4) as i32)
        } else {
            let rgb = rgb3(im);
            w::WebPPictureImportRGB(&mut pic, rgb.as_ptr(), (im.w * 3) as i32)
        };
        if ok == 0 {
            w::WebPPictureFree(&mut pic);
            bail!("import");
        }
        let mut wr: w::WebPMemoryWriter = std::mem::zeroed();
        w::WebPMemoryWriterInit(&mut wr);
        pic.writer = Some(w::WebPMemoryWrite);
        pic.custom_ptr = &mut wr as *mut _ as *mut _;
        let ok = w::WebPEncode(&cfg, &mut pic);
        let err = pic.error_code;
        w::WebPPictureFree(&mut pic);
        if ok == 0 {
            bail!("encode err {err:?}");
        }
        let v = std::slice::from_raw_parts(wr.mem, wr.size).to_vec();
        w::WebPMemoryWriterClear(&mut wr);
        Ok(v)
    }
}

pub fn decode(data: &[u8]) -> Result<Rgba> {
    unsafe {
        let (mut wd, mut ht) = (0, 0);
        if w::WebPGetInfo(data.as_ptr(), data.len(), &mut wd, &mut ht) == 0 {
            bail!("not webp");
        }
        let p = w::WebPDecodeRGBA(data.as_ptr(), data.len(), &mut wd, &mut ht);
        if p.is_null() {
            bail!("decode");
        }
        let v = std::slice::from_raw_parts(p, (wd * ht * 4) as usize).to_vec();
        w::WebPFree(p as *mut _);
        Ok(Rgba { w: wd as u32, h: ht as u32, px: v })
    }
}
