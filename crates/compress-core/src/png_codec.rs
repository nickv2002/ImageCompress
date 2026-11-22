use crate::options::Options;
use std::io::Cursor;

#[derive(Debug, thiserror::Error)]
pub enum CodecError {
    #[error("{0}")]
    Msg(String),
}
fn e<T: std::fmt::Display>(x: T) -> CodecError { CodecError::Msg(x.to_string()) }

fn oxi_opts(o: &Options, animated: bool) -> oxipng::Options {
    let mut op = oxipng::Options::from_preset(o.png_optimize_level.min(6));
    op.strip = crate::meta::png_strip(o.metadata, animated);
    op.optimize_alpha = true;
    if let Some(it) = o.png_zopfli {
        if let Some(n) = std::num::NonZeroU8::new(it) {
            op.deflater = oxipng::Deflater::Zopfli(oxipng::ZopfliOptions { iteration_count: n.into(), ..Default::default() });
        }
    }
    op
}

/// Lossless PNG optimisation of the raw bytes (APNGs keep every frame).
pub fn optimize_lossless(data: &[u8], o: &Options) -> Result<Vec<u8>, CodecError> {
    oxipng::optimize_from_memory(data, &oxi_opts(o, crate::anim::png_frames(data) > 1)).map_err(e)
}

/// Quantize with imagequant then optimise. Returns None if the quality floor can't be met.
pub fn quantize_then_optimize(data: &[u8], o: &Options, (qmin, qmax): (u8, u8)) -> Result<Option<Vec<u8>>, CodecError> {
    let img = image::load_from_memory_with_format(data, image::ImageFormat::Png).map_err(e)?.to_rgba8();
    let (w, h) = (img.width() as usize, img.height() as usize);
    let mut attr = imagequant::new();
    attr.set_quality(qmin, qmax).map_err(e)?;
    attr.set_speed(o.png_quantize_speed.clamp(1, 10) as i32).map_err(e)?;
    let pixels: Vec<imagequant::RGBA> = img.pixels().map(|p| imagequant::RGBA::new(p[0], p[1], p[2], p[3])).collect();
    let mut qi = attr.new_image(pixels, w, h, 0.0).map_err(e)?;
    let mut res = match attr.quantize(&mut qi) {
        Ok(r) => r,
        Err(imagequant::Error::QualityTooLow) => return Ok(None),
        Err(x) => return Err(e(x)),
    };
    res.set_dithering_level(if o.png_dither { 1.0 } else { 0.0 }).map_err(e)?;
    let (pal, idx) = res.remapped(&mut qi).map_err(e)?;
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(Cursor::new(&mut out), w as u32, h as u32);
        enc.set_color(png::ColorType::Indexed);
        enc.set_depth(png::BitDepth::Eight);
        enc.set_palette(pal.iter().flat_map(|c| [c.r, c.g, c.b]).collect::<Vec<u8>>());
        enc.set_trns(pal.iter().map(|c| c.a).collect::<Vec<u8>>());
        enc.set_compression(png::Compression::Fast);
        let mut wr = enc.write_header().map_err(e)?;
        wr.write_image_data(&idx).map_err(e)?;
    }
    Ok(Some(optimize_lossless(&out, o)?))
}
