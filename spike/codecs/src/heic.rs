//! macOS ImageIO HEIC via objc2-image-io.
use crate::Rgba;
use anyhow::{anyhow, Result};
use objc2_core_foundation::{CFData, CFDictionary, CFMutableData, CFNumber, CFRetained, CFString, CFType};
use objc2_core_graphics::{CGBitmapInfo, CGColorRenderingIntent, CGColorSpace, CGDataProvider, CGImage, CGImageAlphaInfo, kCGColorSpaceSRGB};
use objc2_image_io::{kCGImageDestinationLossyCompressionQuality, CGImageDestination, CGImageSource};
use std::ffi::c_void;

extern "C-unwind" {
    fn CGBitmapContextCreate(data: *mut c_void, w: usize, h: usize, bpc: usize, bpr: usize, cs: *const CGColorSpace, info: u32) -> *mut objc2_core_graphics::CGContext;
}

fn srgb() -> Result<CFRetained<CGColorSpace>> {
    unsafe { CGColorSpace::with_name(Some(kCGColorSpaceSRGB)) }.ok_or_else(|| anyhow!("srgb cs"))
}

/// quality 0.0..1.0 (kCGImageDestinationLossyCompressionQuality). Non-premultiplied RGBA is fed as PremultipliedLast
/// after premultiplying, matching what CG expects.
pub fn encode(im: &Rgba, quality: f32) -> Result<Vec<u8>> {
    let alpha = crate::has_alpha(im);
    let mut buf = im.px.clone();
    if alpha {
        for p in buf.chunks_exact_mut(4) {
            let a = p[3] as u32;
            for c in &mut p[..3] {
                *c = ((*c as u32 * a + 127) / 255) as u8;
            }
        }
    }
    let cfd = unsafe { CFData::new(None, buf.as_ptr(), buf.len() as _) }.ok_or_else(|| anyhow!("cfdata"))?;
    let prov = CGDataProvider::with_cf_data(Some(&cfd)).ok_or_else(|| anyhow!("provider"))?;
    let cs = srgb()?;
    let info = CGBitmapInfo(if alpha { CGImageAlphaInfo::PremultipliedLast.0 } else { CGImageAlphaInfo::NoneSkipLast.0 });
    let img = unsafe {
        CGImage::new(im.w as _, im.h as _, 8, 32, (im.w * 4) as _, Some(&cs), info, Some(&prov), std::ptr::null(), false, CGColorRenderingIntent::RenderingIntentDefault)
    }
    .ok_or_else(|| anyhow!("cgimage"))?;
    let out = CFMutableData::new(None, 0).ok_or_else(|| anyhow!("mdata"))?;
    let ty = CFString::from_static_str("public.heic");
    let q = CFNumber::new_f64(quality as f64);
    let opts = CFDictionary::<CFString, CFType>::from_slices(&[unsafe { kCGImageDestinationLossyCompressionQuality }], &[q.as_ref()]);
    let dst = unsafe { CGImageDestination::with_data(&out, &ty, 1, None) }.ok_or_else(|| anyhow!("no HEIC destination"))?;
    unsafe {
        dst.add_image(&img, Some(opts.as_opaque()));
        if !dst.finalize() {
            return Err(anyhow!("finalize failed"));
        }
    }
    Ok(unsafe { std::slice::from_raw_parts(out.byte_ptr(), out.length() as usize) }.to_vec())
}

pub fn decode(data: &[u8]) -> Result<Rgba> {
    let cfd = unsafe { CFData::new(None, data.as_ptr(), data.len() as _) }.ok_or_else(|| anyhow!("cfdata"))?;
    let src = unsafe { CGImageSource::with_data(&cfd, None) }.ok_or_else(|| anyhow!("source"))?;
    let img = unsafe { src.image_at_index(0, None) }.ok_or_else(|| anyhow!("image_at_index"))?;
    let (w, h) = (CGImage::width(Some(&img)), CGImage::height(Some(&img)));
    let cs = srgb()?;
    let mut px = vec![0u8; w * h * 4];
    unsafe {
        let ctx = CGBitmapContextCreate(px.as_mut_ptr().cast(), w, h, 8, w * 4, &*cs, CGImageAlphaInfo::PremultipliedLast.0);
        if ctx.is_null() {
            return Err(anyhow!("ctx"));
        }
        let ctx = CFRetained::from_raw(std::ptr::NonNull::new_unchecked(ctx));
        objc2_core_graphics::CGContext::draw_image(Some(&ctx), objc2_core_foundation::CGRect::new(objc2_core_foundation::CGPoint::new(0., 0.), objc2_core_foundation::CGSize::new(w as f64, h as f64)), Some(&img));
    }
    // un-premultiply
    for p in px.chunks_exact_mut(4) {
        let a = p[3] as u32;
        if a != 0 && a != 255 {
            for c in &mut p[..3] {
                *c = ((*c as u32 * 255 + a / 2) / a).min(255) as u8;
            }
        }
    }
    Ok(Rgba { w: w as u32, h: h as u32, px })
}
