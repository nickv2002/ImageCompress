//! Format conversion: generic decode (any supported input -> RGBA8 + ICC, orientation applied) and
//! encode to jpeg / png / webp / avif / jxl / heic.
use crate::options::{Format, JpegEncoder, Metadata, Options};
use crate::pixels::{err, CResult, Decoded, Rgba};
use crate::{avif_codec, jpeg_codec, jxl_codec, png_codec, webp_codec};
use image::ImageDecoder;
use std::io::Cursor;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fmt {
    Jpeg,
    Png,
    Gif,
    Tiff,
    Webp,
    Avif,
    Jxl,
    Heic,
}

impl Fmt {
    pub fn from_ext(ext: &str) -> Option<Fmt> {
        Some(match ext {
            "jpg" | "jpeg" => Fmt::Jpeg,
            "png" => Fmt::Png,
            "gif" => Fmt::Gif,
            "tif" | "tiff" => Fmt::Tiff,
            "webp" => Fmt::Webp,
            "avif" => Fmt::Avif,
            "jxl" => Fmt::Jxl,
            "heic" | "heif" => Fmt::Heic,
            _ => return None,
        })
    }

    pub fn from_format(f: Format) -> Option<Fmt> {
        Some(match f {
            Format::Keep => return None,
            Format::Jpeg => Fmt::Jpeg,
            Format::Png => Fmt::Png,
            Format::Webp => Fmt::Webp,
            Format::Heic => Fmt::Heic,
            Format::Avif => Fmt::Avif,
            Format::Jxl => Fmt::Jxl,
        })
    }

    /// Extension used when a conversion creates a new file.
    pub fn ext(self) -> &'static str {
        match self {
            Fmt::Jpeg => "jpg",
            Fmt::Png => "png",
            Fmt::Gif => "gif",
            Fmt::Tiff => "tiff",
            Fmt::Webp => "webp",
            Fmt::Avif => "avif",
            Fmt::Jxl => "jxl",
            Fmt::Heic => "heic",
        }
    }
}

pub enum Conv {
    Done(Vec<u8>, String),
    Skip(String),
}

/// Decode any supported format; EXIF orientation is applied to the pixels.
pub fn decode_any(data: &[u8], fmt: Fmt) -> CResult<Decoded> {
    match fmt {
        Fmt::Jpeg | Fmt::Png | Fmt::Gif | Fmt::Tiff => {
            let f = match fmt {
                Fmt::Jpeg => image::ImageFormat::Jpeg,
                Fmt::Png => image::ImageFormat::Png,
                Fmt::Gif => image::ImageFormat::Gif,
                _ => image::ImageFormat::Tiff,
            };
            let mut dec = image::ImageReader::with_format(Cursor::new(data), f).into_decoder().map_err(err)?;
            let icc = dec.icc_profile().map_err(err)?;
            let orient = dec.orientation().map_err(err)?;
            let tiff_exif = if fmt == Fmt::Tiff { dec.exif_metadata().ok().flatten() } else { None };
            let mut img = image::DynamicImage::from_decoder(dec).map_err(err)?;
            img.apply_orientation(orient);
            let rgba = img.to_rgba8();
            let mut d = Decoded::new(Rgba { w: rgba.width(), h: rgba.height(), px: rgba.into_raw() }, icc);
            (d.exif, d.xmp) = if fmt == Fmt::Tiff { (tiff_exif, None) } else { crate::meta::extract(data, fmt) };
            Ok(d)
        }
        Fmt::Webp => {
            let mut d = Decoded::new(webp_codec::decode(data)?, webp_codec::icc_of(data));
            (d.exif, d.xmp) = crate::meta::extract(data, fmt);
            // libwebp ignores the EXIF orientation; make pixels upright like every other decoder here
            // (the carried EXIF always has its orientation reset).
            if let Some(o) = d.exif.as_deref().and_then(crate::meta::exif_orientation).filter(|o| *o != image::metadata::Orientation::NoTransforms) {
                let rgba = image::RgbaImage::from_raw(d.img.w, d.img.h, std::mem::take(&mut d.img.px)).ok_or("webp size mismatch")?;
                let mut img = image::DynamicImage::ImageRgba8(rgba);
                img.apply_orientation(o);
                let rgba = img.into_rgba8();
                d.img = Rgba { w: rgba.width(), h: rgba.height(), px: rgba.into_raw() };
            }
            Ok(d)
        }
        Fmt::Avif => avif_codec::decode(data).map(|(d, _)| d),
        Fmt::Jxl => jxl_codec::decode(data),
        #[cfg(target_os = "macos")]
        Fmt::Heic => crate::heic_codec::decode(data),
        #[cfg(not(target_os = "macos"))]
        Fmt::Heic => Err("HEIC is not supported in this build".into()),
    }
}

fn flatten_rgb(im: &Rgba) -> Vec<u8> {
    // composite onto white for formats without alpha
    im.px
        .chunks_exact(4)
        .flat_map(|p| {
            let a = p[3] as u32;
            [0, 1, 2].map(|i| ((p[i] as u32 * a + 255 * (255 - a) + 127) / 255) as u8)
        })
        .collect()
}

fn encode_png(d: &Decoded, o: &Options, keep_icc: bool) -> CResult<(Vec<u8>, String)> {
    let im = &d.img;
    let alpha = im.has_alpha();
    let mut out = Vec::new();
    {
        let mut info = png::Info::with_size(im.w, im.h);
        info.color_type = if alpha { png::ColorType::Rgba } else { png::ColorType::Rgb };
        info.bit_depth = png::BitDepth::Eight;
        if keep_icc {
            if let Some(icc) = &d.icc {
                info.icc_profile = Some(icc.clone().into());
            }
        }
        let mut enc = png::Encoder::with_info(Cursor::new(&mut out), info).map_err(err)?;
        enc.set_compression(png::Compression::Fast);
        let mut w = enc.write_header().map_err(err)?;
        if alpha {
            w.write_image_data(&im.px).map_err(err)?;
        } else {
            w.write_image_data(&im.rgb3()).map_err(err)?;
        }
    }
    if let Some(q) = o.png_quantize {
        if let Some(b) = png_codec::quantize_then_optimize(&out, o, q).map_err(err)? {
            return Ok((b, "imagequant+oxipng".into()));
        }
    }
    Ok((png_codec::optimize_lossless(&out, o).map_err(err)?, "oxipng".into()))
}

/// Encode decoded pixels to `target`. With `target_quality` set (and a lossy-capable target) this runs
/// the perceptual quality search; otherwise a fixed-quality encode.
pub fn encode(d: &Decoded, target: Fmt, o: &Options, stage: &dyn Fn(&str)) -> CResult<(Vec<u8>, String)> {
    if let Some(r) = crate::quality_search::try_search(d, target, o, stage)? {
        return Ok(r);
    }
    encode_fixed(d, target, o, stage)
}

/// Encode for a conversion. When the source is a lossless raster (PNG/TIFF) and the target has a true
/// lossless mode (JXL, WebP), a lossy encode that comes out larger than the target's own lossless
/// encode is replaced by it: flat art compresses better losslessly, and it keeps every pixel.
fn encode_best(d: &Decoded, src: Fmt, target: Fmt, o: &Options, stage: &dyn Fn(&str)) -> CResult<(Vec<u8>, String)> {
    let (b, n) = encode(d, target, o, stage)?;
    let lossy_used = o.target_quality.is_some()
        || (target == Fmt::Jxl && o.jxl_quality.is_some())
        || (target == Fmt::Webp && o.webp_quality.is_some());
    if matches!(src, Fmt::Png | Fmt::Tiff) && matches!(target, Fmt::Jxl | Fmt::Webp) && lossy_used {
        let mut lo = o.clone();
        lo.target_quality = None;
        lo.jxl_quality = None;
        lo.webp_quality = None;
        if let Ok((lb, ln)) = encode_fixed(d, target, &lo, stage) {
            if lb.len() <= b.len() {
                return Ok((lb, ln));
            }
        }
    }
    Ok((b, n))
}

/// Decode-side preparation shared by the conversion paths: optional sRGB conversion, then resize.
fn prepare(mut d: Decoded, o: &Options) -> CResult<Decoded> {
    if o.convert_to_srgb {
        crate::color::convert_to_srgb(&mut d)?;
    }
    crate::resize::apply(&mut d, o)?;
    Ok(d)
}

/// Encode decoded pixels to `target` with the per-format options (fixed quality).
pub fn encode_fixed(d: &Decoded, target: Fmt, o: &Options, stage: &dyn Fn(&str)) -> CResult<(Vec<u8>, String)> {
    let mut icc = if o.metadata != Metadata::None { d.icc.clone() } else { None };
    if o.compact_icc && icc.as_deref().is_some_and(crate::color::is_srgb_equivalent) {
        icc = None;
    }
    let im = &d.img;
    let carried = crate::meta::carry(d, o);
    stage("encode");
    match target {
        Fmt::Jpeg => {
            let rgb = flatten_rgb(im);
            let q = o.jpeg_quality.unwrap_or(DEFAULT_JPEG_QUALITY);
            let pick = |auto: jpeg_codec::Subsampling| match o.jpeg_subsampling {
                crate::options::Subsampling::Auto => auto,
                crate::options::Subsampling::S444 => jpeg_codec::Subsampling::S444,
                crate::options::Subsampling::S422 => jpeg_codec::Subsampling::S422,
                crate::options::Subsampling::S420 => jpeg_codec::Subsampling::S420,
            };
            match o.jpeg_encoder {
                JpegEncoder::Jpegli => {
                    let b = jpeg_codec::encode_rgb8_jpegli(&rgb, im.w, im.h, &jpeg_codec::JpegliOpts {
                        distance: crate::jpeg_pipeline::quality_to_distance(q),
                        quality: Some(q),
                        progressive_level: if o.jpeg_progressive { 2 } else { 0 },
                        subsampling: pick(jpeg_codec::Subsampling::S444),
                        icc,
                    }).map_err(err)?;
                    Ok((b, "jpegli".into()))
                }
                JpegEncoder::Mozjpeg => {
                    let b = jpeg_codec::encode_rgb8(&rgb, im.w, im.h, &jpeg_codec::LossyOpts {
                        quality: q,
                        progressive: o.jpeg_progressive,
                        subsampling: pick(if q >= 90 { jpeg_codec::Subsampling::S444 } else { jpeg_codec::Subsampling::S420 }),
                        icc,
                        ..Default::default()
                    }).map_err(err)?;
                    Ok((b, "mozjpeg".into()))
                }
            }
        }
        Fmt::Png => encode_png(d, o, icc.is_some()),
        Fmt::Webp => {
            let (lossless, q) = match o.webp_quality {
                Some(q) => (false, q as f32),
                None => (true, 100.0),
            };
            let mut b = webp_codec::encode(im, q, lossless, o.webp_method as i32, o.webp_alpha_quality as i32)?;
            b = webp_codec::embed(b, icc.as_deref(), carried.exif.as_deref(), carried.xmp.as_deref())?;
            Ok((b, if lossless { "libwebp-lossless" } else { "libwebp" }.into()))
        }
        Fmt::Avif => {
            let q = o.avif_quality as i32;
            let b = avif_codec::encode(im, &avif_codec::EncOpts {
                quality: q,
                alpha_quality: o.avif_alpha_quality.map(|v| v as i32).unwrap_or(q),
                speed: o.avif_speed as i32,
                depth: if o.avif_depth >= 10 { 10 } else { 8 },
                threads: 0,
                icc,
                exif: carried.exif,
                xmp: carried.xmp,
            })?;
            Ok((b, "libaom".into()))
        }
        Fmt::Jxl => {
            // Only RGB profiles can describe the RGBA buffer; others are dropped.
            let emb = jxl_codec::Embed { icc: icc.as_deref().filter(|i| crate::color::is_rgb_profile(i)), exif: carried.exif.as_deref(), xmp: carried.xmp.as_deref() };
            match o.jxl_quality {
                Some(q) => Ok((jxl_codec::encode_embedded(im, false, jxl_codec::quality_to_distance(q), o.jxl_effort as u32, o.jxl_progressive, &emb)?, "libjxl".into())),
                None => Ok((jxl_codec::encode_embedded(im, true, 0.0, o.jxl_effort as u32, false, &emb)?, "libjxl-lossless".into())),
            }
        }
        #[cfg(target_os = "macos")]
        Fmt::Heic => {
            let q = crate::heic_codec::quality_to_imageio(o.heic_quality);
            let carrier = (carried.exif.is_some() || carried.xmp.is_some()).then(|| crate::meta::carrier_jpeg(&carried)).flatten();
            if o.heic_depth >= 10 {
                let hi = crate::heic_codec::Rgba16::from_rgba8(im);
                Ok((crate::heic_codec::encode16(&hi, q, icc.as_deref(), carrier.as_deref())?, "imageio-heic-10bit".into()))
            } else {
                Ok((crate::heic_codec::encode_meta(im, q, icc.as_deref(), carrier.as_deref())?, "imageio-heic".into()))
            }
        }
        #[cfg(not(target_os = "macos"))]
        Fmt::Heic => Err("HEIC encoding is not supported in this build".into()),
        Fmt::Gif | Fmt::Tiff => Err(format!("cannot encode to .{}", target.ext())),
    }
}

/// The quality used when another format becomes JPEG and no JPEG quality was given.
pub const DEFAULT_JPEG_QUALITY: u8 = 85;

/// Why `src` cannot become a lossless JPEG, when lossless was asked for. JPEG's only lossless mode is a
/// reversible repack of an existing JPEG; any other input would have to be re-encoded lossily, so it is
/// skipped instead of being written as a lossy file under a lossless setting. Leaving the JPEG quality at
/// its default is not a request: those files get `DEFAULT_JPEG_QUALITY`. (The quality search picks its
/// own quality, so `target_quality` is not a lossless request either.)
pub fn lossless_jpeg_unavailable(src: Fmt, target: Fmt, o: &Options) -> Option<String> {
    (target == Fmt::Jpeg && src != Fmt::Jpeg && o.jpeg_quality.is_none() && o.jpeg_lossless_requested && o.target_quality.is_none()).then(|| {
        format!("JPEG has no lossless mode for .{} input; choose a JPEG quality, or a lossless format (PNG, WebP or JPEG XL)", src.ext())
    })
}

/// Convert / recompress `data` (of format `src`) into `target` (which may equal `src` for webp/avif/jxl/heic).
pub fn convert(data: &[u8], src: Fmt, target: Fmt, o: &Options, stage: &dyn Fn(&str)) -> Result<Conv, String> {
    if let Some(r) = lossless_jpeg_unavailable(src, target, o) {
        return Ok(Conv::Skip(r));
    }
    // Reversible JPEG -> JXL transcode (no decode, byte-exact roundtrip verified).
    if src == Fmt::Jpeg && target == Fmt::Jxl && o.jxl_jpeg_transcode && o.jxl_quality.is_none() && !crate::extras::pixel_changes_requested(data, src, o) {
        stage("jxl-transcode");
        if let Ok(b) = jxl_codec::transcode_jpeg(data, o.jxl_effort as u32) {
            if jxl_codec::reconstruct_jpeg(&b).is_ok_and(|r| r == data) {
                return Ok(Conv::Done(b, "libjxl-jpeg-transcode".into()));
            }
        }
        // fall through to a pixel (lossless) encode if the JPEG can't be transcoded
    }
    // 16-bit PNG/TIFF -> 10-bit HEIC keeps the extra precision (the pipeline pixel type is 8-bit)
    #[cfg(target_os = "macos")]
    if target == Fmt::Heic && o.heic_depth >= 10 && o.target_quality.is_none() && matches!(src, Fmt::Png | Fmt::Tiff) && crate::anim::frame_count(data, src) <= 1 {
        let f = if src == Fmt::Png { image::ImageFormat::Png } else { image::ImageFormat::Tiff };
        if let Some((b, n)) = crate::heic_codec::encode_16bit_source(data, f, o)? {
            stage("encode");
            return Ok(Conv::Done(b, n));
        }
    }
    // Stills only: animated inputs are routed by the pipeline (`anim`), never decoded to one frame here.
    if crate::anim::frame_count(data, src) > 1 {
        return Ok(Conv::Skip(crate::anim::UNSUPPORTED.into()));
    }
    if src == Fmt::Avif {
        let (d, _) = avif_codec::decode(data)?;
        let d = prepare(d, o)?;
        let (b, n) = encode_best(&d, src, target, o, stage)?;
        return Ok(Conv::Done(b, n));
    }
    stage("decode");
    let d = prepare(decode_any(data, src)?, o)?;
    let (b, n) = encode_best(&d, src, target, o, stage)?;
    Ok(Conv::Done(b, n))
}
