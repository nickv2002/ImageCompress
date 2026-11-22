//! Colour management helpers (lcms2, statically linked): sRGB-equivalence detection and
//! conversion of pixels to sRGB.
use crate::pixels::{CResult, Decoded};
use lcms2::{ColorSpaceSignature, Flags, Intent, PixelFormat, Profile, Transform};

/// Largest per-channel 8-bit difference to the built-in sRGB profile at which an ICC profile still
/// counts as "sRGB". Real-world sRGB variants (v2, v4, "sRGB IEC61966-2.1") land within 1-2 levels
/// of each other; Display P3 / Adobe RGB miss by >10.
const SRGB_TOLERANCE: u8 = 2;

fn srgb_grid() -> Vec<[u8; 3]> {
    const N: usize = 17;
    let f = |i: usize| (i * 255 / (N - 1)) as u8;
    let mut v = Vec::with_capacity(N * N * N);
    for r in 0..N {
        for g in 0..N {
            for b in 0..N {
                v.push([f(r), f(g), f(b)]);
            }
        }
    }
    v
}

/// True when `icc` is an RGB profile that renders (within tolerance) like standard sRGB, so it can be
/// dropped (an untagged image is interpreted as sRGB).
pub fn is_srgb_equivalent(icc: &[u8]) -> bool {
    let Ok(p) = Profile::new_icc(icc) else { return false };
    if p.color_space() != ColorSpaceSignature::RgbData {
        return false;
    }
    let srgb = Profile::new_srgb();
    let Ok(t) = Transform::<[u8; 3], [u8; 3]>::new(&p, PixelFormat::RGB_8, &srgb, PixelFormat::RGB_8, Intent::RelativeColorimetric) else {
        return false;
    };
    let src = srgb_grid();
    let mut dst = vec![[0u8; 3]; src.len()];
    t.transform_pixels(&src, &mut dst);
    src.iter().zip(&dst).all(|(a, b)| a.iter().zip(b).all(|(x, y)| x.abs_diff(*y) <= SRGB_TOLERANCE))
}

/// True when `icc` parses and is an RGB profile.
pub fn is_rgb_profile(icc: &[u8]) -> bool {
    Profile::new_icc(icc).is_ok_and(|p| p.color_space() == ColorSpaceSignature::RgbData)
}

/// Convert `d.img` from `d.icc` to sRGB in place (alpha untouched) and drop the profile.
/// Returns Ok(true) if pixels were converted. An sRGB-equivalent profile is just dropped; non-RGB
/// profiles are left alone.
pub fn convert_to_srgb(d: &mut Decoded) -> CResult<bool> {
    let Some(icc) = d.icc.as_deref() else { return Ok(false) };
    if is_srgb_equivalent(icc) {
        d.icc = None;
        return Ok(false);
    }
    let p = Profile::new_icc(icc).map_err(|e| e.to_string())?;
    if p.color_space() != ColorSpaceSignature::RgbData {
        return Ok(false);
    }
    let srgb = Profile::new_srgb();
    let t = Transform::<[u8; 4], [u8; 4]>::new_flags(&p, PixelFormat::RGBA_8, &srgb, PixelFormat::RGBA_8, Intent::Perceptual, Flags::COPY_ALPHA)
        .map_err(|e| e.to_string())?;
    // SAFETY: [u8; 4] has alignment 1 and size 4; the RGBA buffer length is a multiple of 4.
    let px = unsafe { std::slice::from_raw_parts_mut(d.img.px.as_mut_ptr().cast::<[u8; 4]>(), d.img.px.len() / 4) };
    t.transform_in_place(px);
    d.icc = None;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p3_icc() -> Vec<u8> {
        let jpg = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/jpeg/display_p3.jpg")).unwrap();
        crate::meta::jpeg_icc(&jpg).expect("p3 fixture carries ICC")
    }

    #[test]
    fn builtin_srgb_is_equivalent() {
        let icc = Profile::new_srgb().icc().unwrap();
        assert!(is_srgb_equivalent(&icc));
    }

    #[test]
    fn display_p3_is_not() {
        assert!(!is_srgb_equivalent(&p3_icc()));
    }

    #[test]
    fn convert_p3_changes_pixels_keeps_alpha() {
        let mut d = Decoded::new(crate::pixels::Rgba { w: 1, h: 1, px: vec![0, 255, 0, 77] }, Some(p3_icc()));
        assert!(convert_to_srgb(&mut d).unwrap());
        assert!(d.icc.is_none());
        assert_eq!(d.img.px[3], 77);
        assert!(d.img.px[1] >= 250 && d.img.px[0] < 30, "{:?}", d.img.px);
    }
}
