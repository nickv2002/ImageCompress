//! Shared in-memory pixel type for the format codecs.

/// Decoded 8-bit RGBA buffer, tightly packed, straight (non-premultiplied) alpha.
#[derive(Debug, Clone)]
pub struct Rgba {
    pub w: u32,
    pub h: u32,
    pub px: Vec<u8>,
}

impl Rgba {
    /// Opaque if every alpha == 255 (lets encoders skip the alpha channel).
    pub fn has_alpha(&self) -> bool {
        self.px.chunks_exact(4).any(|p| p[3] != 255)
    }

    pub fn rgb3(&self) -> Vec<u8> {
        self.px.chunks_exact(4).flat_map(|p| [p[0], p[1], p[2]]).collect()
    }
}

/// A decoded image plus the metadata we carry through a conversion.
#[derive(Debug, Clone)]
pub struct Decoded {
    pub img: Rgba,
    /// ICC profile the pixels are expressed in (None = sRGB).
    pub icc: Option<Vec<u8>>,
    /// Raw EXIF (TIFF header onward, no `Exif\0\0`) as found in the source, unedited.
    pub exif: Option<Vec<u8>>,
    /// XMP packet as found in the source.
    pub xmp: Option<Vec<u8>>,
}

impl Decoded {
    pub fn new(img: Rgba, icc: Option<Vec<u8>>) -> Self {
        Decoded { img, icc, exif: None, xmp: None }
    }
}

pub type CResult<T> = Result<T, String>;

pub(crate) fn err<T: std::fmt::Display>(x: T) -> String {
    x.to_string()
}

/// Decoded animation: full-canvas RGBA frames (already composited) with per-frame delays.
#[derive(Debug, Clone)]
pub struct Frames {
    pub w: u32,
    pub h: u32,
    /// (full-canvas RGBA, delay in ms)
    pub frames: Vec<(Vec<u8>, i32)>,
    /// Plays; 0 = loop forever (WebP/APNG convention).
    pub loop_count: i32,
}
