use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Originals {
    Keep,
    Trash,
    Delete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ReplaceIf {
    Smaller,
    Always,
}

/// What happens when the output file name is already taken by a file that is not the job's own source.
/// (Two inputs of the same run that would write the same name never overwrite each other, whatever this says:
/// they are given distinct names, see `outputs`.)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Overwrite {
    /// Leave the existing file alone and skip the input (nothing is written, the original is untouched).
    #[default]
    Skip,
    /// Write next to it under the first free name: `name-2.ext`, `name-3.ext`, ...
    Rename,
    /// Replace the existing file (never a file that is itself an input of the same run).
    Replace,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Format {
    Keep,
    Jpeg,
    Png,
    Webp,
    Heic,
    Avif,
    Jxl,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Metadata {
    /// Strip everything.
    None,
    /// Keep only the ICC profile.
    Icc,
    /// Keep all metadata: ICC, EXIF and XMP, carried across format conversions too (WebP, AVIF, JXL,
    /// HEIC; orientation is reset to 1 because pixels are written upright).
    All,
    /// Keep ICC and EXIF, but remove the GPS IFD (and thumbnails/MPF/APP13). Orientation is kept
    /// when pixels are untouched and reset to 1 when they were rotated by a re-encode.
    ExifNoGps,
}

/// What animated GIF / APNG / WebP / AVIF inputs become (an explicit `format` of webp or avif has the same effect).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum AnimTo {
    /// Optimise in the source format (gifsicle, oxipng; animated WebP/AVIF are re-encoded at their quality setting).
    #[default]
    Keep,
    /// Animated WebP (libwebp anim encoder). Still GIFs become still WebP.
    Webp,
    /// Animated AVIF image sequence (libaom).
    Avif,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum JpegEncoder {
    Jpegli,
    Mozjpeg,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum Subsampling {
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "444")]
    S444,
    #[serde(rename = "422")]
    S422,
    #[serde(rename = "420")]
    S420,
}

/// Single source of truth for CLI flags, presets and the GUI.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(default, rename_all = "kebab-case")]
pub struct Options {
    // input
    pub recursive: bool,
    pub include_hidden: bool,
    /// Minimum input size in bytes.
    pub min_size: u64,
    // output
    pub format: Format,
    /// "in-place", "alongside" (adds suffix), or a directory path.
    pub destination: String,
    pub suffix: String,
    pub originals: Originals,
    pub replace_if: ReplaceIf,
    /// When the output name is taken by a file that is not this job's own source: skip, rename or replace.
    pub overwrite: Overwrite,
    pub preserve_dates: bool,
    pub metadata: Metadata,
    // jpeg
    pub jpeg_encoder: JpegEncoder,
    /// 1-100; None = lossless (coefficient copy). Lossless works on JPEG input only: other images converted to
    /// JPEG are encoded at `DEFAULT_JPEG_QUALITY`, or skipped when lossless was asked for (`jpeg_lossless_requested`).
    pub jpeg_quality: Option<u8>,
    /// Set by `presets::resolve` when a layer (preset, options file, flag) sets `jpeg-quality` to null on purpose,
    /// as opposed to leaving the default. Not an option of its own.
    #[serde(skip)]
    #[schemars(skip)]
    pub jpeg_lossless_requested: bool,
    pub jpeg_progressive: bool,
    pub jpeg_subsampling: Subsampling,
    // png
    /// Quantize quality range (min, max); None = lossless only.
    pub png_quantize: Option<(u8, u8)>,
    pub png_quantize_speed: u8,
    pub png_dither: bool,
    pub png_optimize_level: u8,
    pub png_zopfli: Option<u8>,
    // jpeg xl
    /// 1-100; None = lossless (reversible JPEG transcode for JPEG input).
    pub jxl_quality: Option<u8>,
    /// 1-10.
    pub jxl_effort: u8,
    /// Reversible JPEG -> JXL recompression when the quality is lossless.
    pub jxl_jpeg_transcode: bool,
    pub jxl_progressive: bool,
    // avif
    /// 1-100 (avifenc -q).
    pub avif_quality: u8,
    /// 1-100; None = same as avif_quality.
    pub avif_alpha_quality: Option<u8>,
    /// 0-10 (avifenc -s).
    pub avif_speed: u8,
    /// 8 or 10.
    pub avif_depth: u8,
    // heic
    /// 1-100 (heif-enc -q scale).
    pub heic_quality: u8,
    /// 8 or 10 (macOS only). 10 writes HEVC Main10; 16-bit PNG/TIFF sources keep their extra precision.
    pub heic_depth: u8,
    // webp
    /// 1-100; None = lossless.
    pub webp_quality: Option<u8>,
    /// 0-6.
    pub webp_method: u8,
    /// 1-100.
    pub webp_alpha_quality: u8,
    // gif
    pub gif_optimize_level: u8,
    pub gif_lossy: Option<u32>,
    // svg
    /// Decimal places kept in path data and numbers (0-5).
    pub svg_precision: u8,
    /// Also run cleanupIds/convertTransform/applyTransforms. Still render-checked, but renames ids that
    /// HTML/CSS/JS may refer to.
    pub svg_aggressive: bool,
    /// Skip SVGs larger than this many bytes (0 = no limit). The optimiser is slow on huge files.
    pub svg_max_size: u64,
    // --- BEGIN extras: quality search / metadata policy / resize / animation ---
    /// SSIMULACRA2 target; per-image binary search of the lossy encoder quality (JPEG, WebP, AVIF, JXL).
    pub target_quality: Option<f32>,
    /// Accept a result scoring in [target, target + tolerance]. The result never scores below target
    /// unless no tried quality reached it.
    pub target_tolerance: f32,
    /// Maximum encodes per image for the quality search.
    pub max_encodes: u8,
    /// Drop sRGB-equivalent ICC profiles (untagged is sRGB). Wide-gamut profiles are always kept.
    pub compact_icc: bool,
    /// Remove EXIF thumbnails, MPF previews and Photoshop APP13 (implied by `metadata = exif-no-gps`).
    pub strip_previews: bool,
    /// Convert pixels with a non-sRGB ICC profile to sRGB (lcms2) and drop the profile.
    pub convert_to_srgb: bool,
    /// Downscale so the longer side is at most this many pixels (never upscales).
    pub max_dimension: Option<u32>,
    /// Scale by this percent (1-99; never upscales).
    pub scale: Option<f32>,
    pub anim_to: AnimTo,
    // --- END extras ---
    // run
    pub history: bool,
    pub jobs: usize,
    pub dry_run: bool,
    /// Keep a backup of every file this run replaces or removes here, so `undo` can restore it. Not a user option.
    #[serde(skip)]
    #[schemars(skip)]
    pub backup_dir: Option<std::path::PathBuf>,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            recursive: true,
            include_hidden: false,
            min_size: 0,
            format: Format::Keep,
            destination: "in-place".into(),
            suffix: "-min".into(),
            originals: Originals::Keep,
            replace_if: ReplaceIf::Smaller,
            overwrite: Overwrite::Skip,
            preserve_dates: true,
            metadata: Metadata::Icc,
            jpeg_encoder: JpegEncoder::Jpegli,
            jpeg_quality: None,
            jpeg_lossless_requested: false,
            jpeg_progressive: true,
            jpeg_subsampling: Subsampling::Auto,
            png_quantize: None,
            png_quantize_speed: 3,
            png_dither: true,
            png_optimize_level: 4,
            png_zopfli: None,
            jxl_quality: None,
            jxl_effort: 7,
            jxl_jpeg_transcode: true,
            jxl_progressive: false,
            avif_quality: 60,
            avif_alpha_quality: None,
            avif_speed: 6,
            avif_depth: 8,
            heic_quality: 50,
            heic_depth: 8,
            webp_quality: Some(80),
            webp_method: 4,
            webp_alpha_quality: 100,
            gif_optimize_level: 3,
            gif_lossy: None,
            svg_precision: 3,
            svg_aggressive: false,
            svg_max_size: 1024 * 1024,
            // --- BEGIN extras ---
            target_quality: None,
            target_tolerance: 1.5,
            max_encodes: 6,
            compact_icc: true,
            strip_previews: false,
            convert_to_srgb: false,
            max_dimension: None,
            scale: None,
            anim_to: AnimTo::Keep,
            // --- END extras ---
            history: true,
            jobs: 0,
            dry_run: false,
            backup_dir: None,
        }
    }
}
