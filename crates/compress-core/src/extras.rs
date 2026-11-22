//! Glue for the quality-search / metadata / resize features: decides when a file must
//! take the decode -> encode path and applies the post-encode metadata policy.
use crate::convert::Fmt;
use crate::options::Options;
use image::ImageDecoder;
use std::io::Cursor;

fn image_format(f: Fmt) -> Option<image::ImageFormat> {
    match f {
        Fmt::Jpeg => Some(image::ImageFormat::Jpeg),
        Fmt::Png => Some(image::ImageFormat::Png),
        _ => None,
    }
}

fn dims(data: &[u8], f: Fmt) -> Option<(u32, u32)> {
    image::ImageReader::with_format(Cursor::new(data), image_format(f)?).into_dimensions().ok()
}

fn nonsrgb_icc(data: &[u8], f: Fmt) -> bool {
    let icc = match f {
        Fmt::Jpeg => crate::meta::jpeg_icc(data),
        Fmt::Png => image::ImageReader::with_format(Cursor::new(data), image::ImageFormat::Png)
            .into_decoder()
            .ok()
            .and_then(|mut d| d.icc_profile().ok().flatten()),
        _ => None,
    };
    icc.is_some_and(|i| !crate::color::is_srgb_equivalent(&i))
}

/// True when the requested options change pixels (resize, sRGB conversion, perceptual search), so a
/// JPEG/PNG cannot stay on its byte-level (lossless) path.
pub fn pixel_changes_requested(data: &[u8], src: Fmt, o: &Options) -> bool {
    let Some(_) = image_format(src) else { return false };
    if o.target_quality.is_some() && src == Fmt::Jpeg {
        return true;
    }
    if (o.max_dimension.is_some() || o.scale.is_some()) && dims(data, src).is_some_and(|(w, h)| crate::resize::target_size(w, h, o).is_some()) {
        return true;
    }
    o.convert_to_srgb && nonsrgb_icc(data, src)
}

/// Metadata policy post-pass on a finished file.
pub fn finalize(src: &[u8], out: Vec<u8>, src_fmt: Option<Fmt>, out_ext: &str, o: &Options, reencoded: bool) -> Vec<u8> {
    match out_ext {
        "jpg" | "jpeg" if src_fmt == Some(Fmt::Jpeg) => crate::meta::jpeg_post(src, out, o, reencoded),
        "png" => crate::meta::png_post(out, o),
        _ => out,
    }
}
