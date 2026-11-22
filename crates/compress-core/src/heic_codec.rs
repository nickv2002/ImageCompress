//! macOS ImageIO HEIC via objc2-image-io (hardware HEVC where available).
use crate::pixels::{CResult, Decoded, Rgba};
use objc2_core_foundation::{CFData, CFDictionary, CFMutableData, CFNumber, CFRetained, CFString, CFType};
use objc2_core_graphics::{CGBitmapInfo, CGColorRenderingIntent, CGColorSpace, CGDataProvider, CGImage, CGImageAlphaInfo, CGImageByteOrderInfo, kCGColorSpaceSRGB};
use objc2_image_io::{kCGImageDestinationLossyCompressionQuality, CGImageDestination, CGImageSource};
use std::ffi::c_void;

extern "C-unwind" {
    fn CGBitmapContextCreate(data: *mut c_void, w: usize, h: usize, bpc: usize, bpr: usize, cs: *const CGColorSpace, info: u32) -> *mut objc2_core_graphics::CGContext;
}

fn srgb() -> CResult<CFRetained<CGColorSpace>> {
    unsafe { CGColorSpace::with_name(Some(kCGColorSpaceSRGB)) }.ok_or_else(|| "srgb colorspace".to_string())
}

/// Map the CLI's 1-100 scale (heif-enc `-q`) to ImageIO's 0..1 lossy quality. ImageIO's hardware HEVC is ~25-35%
/// bigger than x265 at equal SSIMULACRA2, so q50 -> 0.64 is a compromise between matching x265 q50 by size
/// (~0.55-0.65) and by quality (~0.72); measured on the bench fixtures. On the bench fixture 0.64
/// lands within 2% of heif-enc -q 50 in bytes and within 1.5 SSIMULACRA2 points.
pub fn quality_to_imageio(q: u8) -> f32 {
    let q = q.clamp(1, 100) as f32;
    if q <= 50.0 {
        0.1 + (q - 1.0) / 49.0 * 0.54
    } else {
        0.64 + (q - 50.0) / 50.0 * 0.36
    }
}

fn color_space(icc: Option<&[u8]>) -> CResult<CFRetained<CGColorSpace>> {
    match icc.and_then(|b| {
        let d = unsafe { CFData::new(None, b.as_ptr(), b.len() as _) }?;
        unsafe { CGColorSpace::with_icc_data(Some(d.as_ref())) }
    }) {
        Some(c) => Ok(c),
        None => srgb(),
    }
}

/// Hand a finished CGImage to the HEIC destination. ImageIO picks HEVC Main10 whenever the CGImage has more
/// than 8 bits per component; there is no option flag for it.
fn write_heic(img: &CGImage, quality: f32, carrier: Option<&[u8]>) -> CResult<Vec<u8>> {
    let out = CFMutableData::new(None, 0).ok_or("mdata")?;
    let ty = CFString::from_static_str("public.heic");
    let q = CFNumber::new_f64(quality as f64);
    let opts = CFDictionary::<CFString, CFType>::from_slices(&[unsafe { kCGImageDestinationLossyCompressionQuality }], &[q.as_ref()]);
    let dst = unsafe { CGImageDestination::with_data(&out, &ty, 1, None) }.ok_or("no HEIC destination")?;
    unsafe {
        let md = carrier.and_then(|c| {
            let cj = CFData::new(None, c.as_ptr(), c.len() as _)?;
            CGImageSource::with_data(&cj, None)?.metadata_at_index(0, None)
        });
        match &md {
            Some(md) => dst.add_image_and_metadata(img, Some(md), Some(opts.as_opaque())),
            None => dst.add_image(img, Some(opts.as_opaque())),
        }
        if !dst.finalize() {
            return Err("heic finalize failed".into());
        }
    }
    Ok(unsafe { std::slice::from_raw_parts(out.byte_ptr(), out.length() as usize) }.to_vec())
}

/// quality 0.0..1.0. Non-premultiplied RGBA is premultiplied before handing to CoreGraphics.
pub fn encode(im: &Rgba, quality: f32, icc: Option<&[u8]>) -> CResult<Vec<u8>> {
    encode_meta(im, quality, icc, None)
}

/// As [`encode`]; `carrier` is a JPEG whose EXIF/XMP are written into the HEIC. ImageIO cannot take a raw
/// EXIF blob, so the (already policy-edited) blobs travel in a tiny carrier JPEG (`meta::carrier_jpeg`)
/// whose `CGImageMetadata` goes through `CGImageDestinationAddImageAndMetadata`.
pub fn encode_meta(im: &Rgba, quality: f32, icc: Option<&[u8]>, carrier: Option<&[u8]>) -> CResult<Vec<u8>> {
    let alpha = im.has_alpha();
    let mut buf = im.px.clone();
    if alpha {
        for p in buf.chunks_exact_mut(4) {
            let a = p[3] as u32;
            for c in &mut p[..3] {
                *c = ((*c as u32 * a + 127) / 255) as u8;
            }
        }
    }
    let cfd = unsafe { CFData::new(None, buf.as_ptr(), buf.len() as _) }.ok_or("cfdata")?;
    let prov = CGDataProvider::with_cf_data(Some(&cfd)).ok_or("provider")?;
    let cs = color_space(icc)?;
    let info = CGBitmapInfo(if alpha { CGImageAlphaInfo::PremultipliedLast.0 } else { CGImageAlphaInfo::NoneSkipLast.0 });
    let img = unsafe {
        CGImage::new(im.w as _, im.h as _, 8, 32, (im.w * 4) as _, Some(&cs), info, Some(&prov), std::ptr::null(), false, CGColorRenderingIntent::RenderingIntentDefault)
    }
    .ok_or("cgimage")?;
    write_heic(&img, quality, carrier)
}

/// 16-bit RGBA, tightly packed, straight (non-premultiplied) alpha. Only used to feed the 10-bit HEIC encoder.
#[derive(Debug, Clone)]
pub struct Rgba16 {
    pub w: u32,
    pub h: u32,
    pub px: Vec<u16>,
}

impl Rgba16 {
    /// Promote 8-bit pixels (`v * 257` maps 0..255 onto 0..65535 exactly).
    pub fn from_rgba8(im: &Rgba) -> Rgba16 {
        Rgba16 { w: im.w, h: im.h, px: im.px.iter().map(|&v| v as u16 * 257).collect() }
    }

    pub fn has_alpha(&self) -> bool {
        self.px.chunks_exact(4).any(|p| p[3] != u16::MAX)
    }
}

/// 10-bit HEIC (HEVC Main10). The CGImage must be RGBA64 (`bitsPerPixel 64`) with 16-bit little-endian
/// samples: a 48 bpp RGB CGImage is silently reduced to 8-bit precision by ImageIO.
pub fn encode16(im: &Rgba16, quality: f32, icc: Option<&[u8]>, carrier: Option<&[u8]>) -> CResult<Vec<u8>> {
    let alpha = im.has_alpha();
    let mut buf: Vec<u8> = Vec::with_capacity(im.px.len() * 2);
    for p in im.px.chunks_exact(4) {
        let a = p[3] as u32;
        for &c in &p[..3] {
            let v = if alpha { ((c as u32 * a + 32767) / 65535) as u16 } else { c };
            buf.extend_from_slice(&v.to_le_bytes());
        }
        buf.extend_from_slice(&p[3].to_le_bytes());
    }
    let cfd = unsafe { CFData::new(None, buf.as_ptr(), buf.len() as _) }.ok_or("cfdata")?;
    let prov = CGDataProvider::with_cf_data(Some(&cfd)).ok_or("provider")?;
    let cs = color_space(icc)?;
    let alpha_info = if alpha { CGImageAlphaInfo::PremultipliedLast.0 } else { CGImageAlphaInfo::NoneSkipLast.0 };
    let info = CGBitmapInfo(alpha_info | CGImageByteOrderInfo::Order16Little.0);
    let img = unsafe {
        CGImage::new(im.w as _, im.h as _, 16, 64, (im.w * 8) as _, Some(&cs), info, Some(&prov), std::ptr::null(), false, CGColorRenderingIntent::RenderingIntentDefault)
    }
    .ok_or("cgimage")?;
    write_heic(&img, quality, carrier)
}

/// Bit depth of the first HEVC stream in a HEIC file (from the `hvcC` box: bitDepthLumaMinus8).
pub fn bit_depth(heic: &[u8]) -> Option<u8> {
    let at = heic.windows(4).position(|w| w == b"hvcC")? + 4;
    heic.get(at + 17).map(|b| (b & 7) + 8)
}

/// 16-bit PNG/TIFF straight to 10-bit HEIC without squeezing through the 8-bit pipeline type. `Ok(None)` when
/// the source is not 16-bit or the options need pixel work that only exists for 8-bit pixels (resize,
/// sRGB conversion); the caller then takes the normal 8-bit-pixel route (which still writes 10-bit HEIC,
/// just from promoted 8-bit samples).
pub fn encode_16bit_source(data: &[u8], fmt: image::ImageFormat, o: &crate::options::Options) -> CResult<Option<(Vec<u8>, String)>> {
    use image::ImageDecoder;
    if o.max_dimension.is_some() || o.scale.is_some() || o.convert_to_srgb {
        return Ok(None);
    }
    let mut dec = image::ImageReader::with_format(std::io::Cursor::new(data), fmt).into_decoder().map_err(|e| e.to_string())?;
    if dec.original_color_type().bits_per_pixel() / dec.original_color_type().channel_count() as u16 != 16 {
        return Ok(None);
    }
    let icc = dec.icc_profile().map_err(|e| e.to_string())?;
    let orient = dec.orientation().map_err(|e| e.to_string())?;
    let mut img = image::DynamicImage::from_decoder(dec).map_err(|e| e.to_string())?;
    img.apply_orientation(orient);
    let rgba = img.to_rgba16();
    let im = Rgba16 { w: rgba.width(), h: rgba.height(), px: rgba.into_raw() };
    let mut icc = if o.metadata != crate::options::Metadata::None { icc } else { None };
    if o.compact_icc && icc.as_deref().is_some_and(crate::color::is_srgb_equivalent) {
        icc = None;
    }
    let b = encode16(&im, quality_to_imageio(o.heic_quality), icc.as_deref(), None)?;
    Ok(Some((b, "imageio-heic-16bit".into())))
}

/// Decode to sRGB RGBA8 (ImageIO colour-manages), un-premultiplied. Orientation is applied by ImageIO's transform.
pub fn decode(data: &[u8]) -> CResult<Decoded> {
    let cfd = unsafe { CFData::new(None, data.as_ptr(), data.len() as _) }.ok_or("cfdata")?;
    let src = unsafe { CGImageSource::with_data(&cfd, None) }.ok_or("heic source")?;
    let img = unsafe { src.image_at_index(0, None) }.ok_or("heic image_at_index")?;
    let (w, h) = (CGImage::width(Some(&img)), CGImage::height(Some(&img)));
    let cs = srgb()?;
    let mut px = vec![0u8; w * h * 4];
    unsafe {
        let ctx = CGBitmapContextCreate(px.as_mut_ptr().cast(), w, h, 8, w * 4, &*cs, CGImageAlphaInfo::PremultipliedLast.0);
        if ctx.is_null() {
            return Err("cg context".into());
        }
        let ctx = CFRetained::from_raw(std::ptr::NonNull::new_unchecked(ctx));
        objc2_core_graphics::CGContext::draw_image(Some(&ctx), objc2_core_foundation::CGRect::new(objc2_core_foundation::CGPoint::new(0., 0.), objc2_core_foundation::CGSize::new(w as f64, h as f64)), Some(&img));
    }
    for p in px.chunks_exact_mut(4) {
        let a = p[3] as u32;
        if a != 0 && a != 255 {
            for c in &mut p[..3] {
                *c = ((*c as u32 * 255 + a / 2) / a).min(255) as u8;
            }
        }
    }
    Ok(Decoded::new(Rgba { w: w as u32, h: h as u32, px }, None))
}

/// Number of images ImageIO sees (an HEIC image sequence / multi-image file has more than one).
pub fn frame_count(data: &[u8]) -> usize {
    let Some(cfd) = (unsafe { CFData::new(None, data.as_ptr(), data.len() as _) }) else { return 1 };
    let Some(src) = (unsafe { CGImageSource::with_data(&cfd, None) }) else { return 1 };
    unsafe { src.count() }.max(1)
}
