//! CLI flags for: perceptual quality search, metadata extras, resize, animation.
//! Kept in their own file; `RunArgs` flattens `ExtraArgs` and `build()` calls `apply_extras`.
use clap::Args;
use compress_core::options::Options;

#[derive(Args, Default, Debug, Clone)]
pub struct ExtraArgs {
    /// Target SSIMULACRA2 score (e.g. 70) or "off": per-image search of the lossy quality (JPEG, WebP, AVIF, JXL)
    #[arg(long)]
    pub target_quality: Option<String>,
    /// Accept results scoring in [target, target+N]
    #[arg(long)]
    pub target_tolerance: Option<f32>,
    /// Maximum encodes per image for --target-quality
    #[arg(long)]
    pub max_encodes: Option<u8>,
    /// Drop sRGB-equivalent ICC profiles (default)
    #[arg(long, overrides_with = "no_compact_icc")]
    pub compact_icc: bool,
    #[arg(long)]
    pub no_compact_icc: bool,
    /// Remove EXIF thumbnails, MPF previews and APP13
    #[arg(long)]
    pub strip_previews: bool,
    /// Convert non-sRGB (e.g. Display P3) pixels to sRGB and drop the profile
    #[arg(long)]
    pub convert_to_srgb: bool,
    /// Downscale so the longer side is at most PX (never upscales)
    #[arg(long)]
    pub max_dimension: Option<u32>,
    /// Downscale to PERCENT of the original size (1-99)
    #[arg(long)]
    pub scale: Option<f32>,
    /// Animated GIF/APNG/WebP/AVIF output: keep (optimise in place), webp or avif (animated)
    #[arg(long, value_parser = ["keep", "webp", "avif"])]
    pub anim_to: Option<String>,
}

pub fn extra_overrides(a: &ExtraArgs, m: &mut serde_json::Map<String, serde_json::Value>) -> Result<(), String> {
    use serde_json::{json, Value};
    if let Some(v) = &a.target_quality {
        let val = if v == "off" {
            Value::Null
        } else {
            let q: f32 = v.parse().map_err(|_| "--target-quality wants a number (SSIMULACRA2) or off")?;
            if !(1.0..=100.0).contains(&q) {
                return Err("--target-quality wants 1-100".into());
            }
            json!(q)
        };
        m.insert("target-quality".into(), val);
    }
    if let Some(v) = a.target_tolerance {
        if v < 0.0 {
            return Err("--target-tolerance must be >= 0".into());
        }
        m.insert("target-tolerance".into(), json!(v));
    }
    if let Some(v) = a.max_encodes {
        if v == 0 {
            return Err("--max-encodes wants >= 1".into());
        }
        m.insert("max-encodes".into(), json!(v));
    }
    if a.compact_icc {
        m.insert("compact-icc".into(), json!(true));
    }
    if a.no_compact_icc {
        m.insert("compact-icc".into(), json!(false));
    }
    if a.strip_previews {
        m.insert("strip-previews".into(), json!(true));
    }
    if a.convert_to_srgb {
        m.insert("convert-to-srgb".into(), json!(true));
    }
    if let Some(v) = a.max_dimension {
        if v == 0 {
            return Err("--max-dimension wants >= 1".into());
        }
        m.insert("max-dimension".into(), json!(v));
    }
    if let Some(v) = a.scale {
        if !(1.0..100.0).contains(&v) {
            return Err("--scale wants 1-99 (percent; never upscales)".into());
        }
        m.insert("scale".into(), json!(v));
    }
    if let Some(v) = &a.anim_to {
        m.insert("anim-to".into(), json!(v));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::{Args, Command};

    /// Every Options field added with these features must have a flag of the same (kebab) name.
    #[test]
    fn every_extra_option_has_a_flag() {
        let cmd = ExtraArgs::augment_args(Command::new("t"));
        let flags: Vec<String> = cmd.get_arguments().map(|a| a.get_id().to_string().replace('_', "-")).collect();
        let schema = serde_json::to_value(schemars::schema_for!(Options)).unwrap();
        let props = schema["properties"].as_object().unwrap();
        for f in ["target-quality", "target-tolerance", "max-encodes", "compact-icc", "strip-previews", "convert-to-srgb", "max-dimension", "scale", "anim-to"] {
            assert!(props.contains_key(f), "Options lacks {f}");
            assert!(flags.iter().any(|x| x == f), "no flag for {f}");
        }
    }
}
