//! Downscaling (never upscales). Resizes in linear light: sRGB u8 -> linear u16 (no banding in the
//! darks) -> Lanczos3 -> sRGB u8. Alpha is premultiplied around the convolution by the resizer.
use crate::options::Options;
use crate::pixels::{CResult, Rgba};
use fast_image_resize::{
    create_srgb_mapper,
    images::Image,
    FilterType, PixelType, ResizeAlg, ResizeOptions, Resizer,
};

/// Dimensions after applying `max_dimension` / `scale`, or `None` if nothing changes.
pub fn target_size(w: u32, h: u32, o: &Options) -> Option<(u32, u32)> {
    let mut f = 1.0f64;
    if let Some(p) = o.scale {
        if p > 0.0 && p < 100.0 {
            f = f.min(p as f64 / 100.0);
        }
    }
    if let Some(m) = o.max_dimension {
        let long = w.max(h);
        if m > 0 && long > m {
            f = f.min(m as f64 / long as f64);
        }
    }
    if f >= 1.0 {
        return None;
    }
    let nw = ((w as f64 * f).round() as u32).max(1);
    let nh = ((h as f64 * f).round() as u32).max(1);
    (nw < w || nh < h).then_some((nw, nh))
}

fn run(im: &Rgba, nw: u32, nh: u32, linear: bool, filter: FilterType) -> CResult<Rgba> {
    let src = Image::from_vec_u8(im.w, im.h, im.px.clone(), PixelType::U8x4).map_err(|e| e.to_string())?;
    let opts = ResizeOptions::new().resize_alg(ResizeAlg::Convolution(filter));
    let mut rs = Resizer::new();
    let px = if linear {
        let mapper = create_srgb_mapper();
        let mut lin = Image::new(im.w, im.h, PixelType::U16x4);
        mapper.forward_map(&src, &mut lin).map_err(|e| e.to_string())?;
        let mut small = Image::new(nw, nh, PixelType::U16x4);
        rs.resize(&lin, &mut small, &opts).map_err(|e| e.to_string())?;
        let mut out = Image::new(nw, nh, PixelType::U8x4);
        mapper.backward_map(&small, &mut out).map_err(|e| e.to_string())?;
        out.into_vec()
    } else {
        let mut out = Image::new(nw, nh, PixelType::U8x4);
        rs.resize(&src, &mut out, &opts).map_err(|e| e.to_string())?;
        out.into_vec()
    };
    Ok(Rgba { w: nw, h: nh, px })
}

/// High-quality linear-light Lanczos3 downscale (the user-facing resize).
pub fn resize_rgba(im: &Rgba, nw: u32, nh: u32) -> CResult<Rgba> {
    run(im, nw, nh, true, FilterType::Lanczos3)
}

/// Apply `max_dimension` / `scale` to a decoded image in place. Returns whether it was resized.
pub fn apply(d: &mut crate::pixels::Decoded, o: &Options) -> CResult<bool> {
    let Some((nw, nh)) = target_size(d.img.w, d.img.h, o) else { return Ok(false) };
    d.img = resize_rgba(&d.img, nw, nh)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts(max: Option<u32>, scale: Option<f32>) -> Options {
        Options { max_dimension: max, scale, ..Default::default() }
    }

    #[test]
    fn sizes() {
        assert_eq!(target_size(4000, 3000, &opts(Some(1000), None)), Some((1000, 750)));
        assert_eq!(target_size(3000, 4000, &opts(Some(1000), None)), Some((750, 1000)));
        assert_eq!(target_size(800, 600, &opts(Some(1000), None)), None, "never upscale");
        assert_eq!(target_size(800, 600, &opts(None, Some(50.0))), Some((400, 300)));
        assert_eq!(target_size(800, 600, &opts(None, Some(100.0))), None);
        assert_eq!(target_size(800, 600, &opts(Some(500), Some(50.0))), Some((400, 300)), "stricter wins");
        assert_eq!(target_size(10, 1, &opts(None, Some(1.0))), Some((1, 1)));
    }

    #[test]
    fn linear_light_averages_in_linear() {
        // black/white checkerboard averages to ~188 (sRGB of linear 0.5), not 128 (gamma-space mean)
        let (w, h) = (16u32, 16u32);
        let mut px = vec![];
        for y in 0..h {
            for x in 0..w {
                let v = if (x + y) % 2 == 0 { 0 } else { 255 };
                px.extend_from_slice(&[v, v, v, 255]);
            }
        }
        let big = Rgba { w, h, px };
        let small = resize_rgba(&big, 1, 1).unwrap();
        let g = small.px[0] as i32;
        assert!((g - 188).abs() <= 3, "got {g}");
        assert_eq!(small.px[3], 255);
    }
}
