//! One CLI flag per `Options` field (a test enforces it). Flags only produce *overrides*:
//! a partial JSON object that is layered on top of defaults, preset and options file.
//! To add a field: add the flag below and one line in `overrides()`.
use clap::Args;
use serde_json::{json, Map, Value};

pub type Overrides = Map<String, Value>;

#[derive(Args, Default, Debug, Clone)]
pub struct OptFlags {
    // ---- input
    #[arg(long, overrides_with = "no_recursive")]
    recursive: bool,
    #[arg(long, overrides_with = "recursive")]
    no_recursive: bool,
    #[arg(long)]
    include_hidden: bool,
    /// Skip inputs smaller than this, e.g. 10KB, 2MB, 500
    #[arg(long, value_name = "SIZE")]
    min_size: Option<String>,
    // ---- output
    /// keep|jpeg|png|webp|heic|avif|jxl
    #[arg(long, value_parser = ["keep", "jpeg", "png", "webp", "heic", "avif", "jxl"])]
    format: Option<String>,
    /// in-place, alongside (adds the suffix), or an output directory
    #[arg(long, value_name = "in-place|alongside|DIR")]
    destination: Option<String>,
    #[arg(long, allow_hyphen_values = true)]
    suffix: Option<String>,
    #[arg(long, value_parser = ["keep", "trash", "delete"])]
    originals: Option<String>,
    #[arg(long, value_parser = ["smaller", "always"])]
    replace_if: Option<String>,
    /// What to do when the output name is taken by a file that is not the input itself
    #[arg(long, value_parser = ["skip", "rename", "replace"])]
    overwrite: Option<String>,
    #[arg(long, overrides_with = "no_preserve_dates")]
    preserve_dates: bool,
    #[arg(long, overrides_with = "preserve_dates")]
    no_preserve_dates: bool,
    #[arg(long, value_parser = ["none", "icc", "exif-no-gps", "all"])]
    metadata: Option<String>,
    /// Shortcut: 1-100 or "lossless", applied to the encoder --format selects (JPEG and PNG when format is keep)
    #[arg(long, value_name = "N|lossless")]
    quality: Option<String>,
    // ---- jpeg
    #[arg(long, value_parser = ["jpegli", "mozjpeg"])]
    jpeg_encoder: Option<String>,
    /// 1-100 or "lossless" (a reversible repack, so JPEG input only: other images are skipped)
    #[arg(long, value_name = "1-100|lossless")]
    jpeg_quality: Option<String>,
    #[arg(long, overrides_with = "no_jpeg_progressive")]
    jpeg_progressive: bool,
    #[arg(long, overrides_with = "jpeg_progressive")]
    no_jpeg_progressive: bool,
    #[arg(long, value_parser = ["auto", "444", "422", "420"])]
    jpeg_subsampling: Option<String>,
    // ---- png
    #[arg(long, value_name = "MIN-MAX|off")]
    png_quantize: Option<String>,
    /// 1-11
    #[arg(long, value_name = "N")]
    png_quantize_speed: Option<u8>,
    #[arg(long, overrides_with = "no_png_dither")]
    png_dither: bool,
    #[arg(long, overrides_with = "png_dither")]
    no_png_dither: bool,
    #[arg(long, value_name = "0-6|max")]
    png_optimize_level: Option<String>,
    #[arg(long, value_name = "ITERATIONS|off")]
    png_zopfli: Option<String>,
    // ---- jpeg xl
    #[arg(long, value_name = "1-100|lossless")]
    jxl_quality: Option<String>,
    /// 1-10
    #[arg(long, value_name = "N")]
    jxl_effort: Option<u8>,
    #[arg(long, overrides_with = "no_jxl_jpeg_transcode")]
    jxl_jpeg_transcode: bool,
    #[arg(long, overrides_with = "jxl_jpeg_transcode")]
    no_jxl_jpeg_transcode: bool,
    #[arg(long, overrides_with = "no_jxl_progressive")]
    jxl_progressive: bool,
    #[arg(long, overrides_with = "jxl_progressive")]
    no_jxl_progressive: bool,
    // ---- avif
    /// 1-100
    #[arg(long, value_name = "N")]
    avif_quality: Option<u8>,
    #[arg(long, value_name = "1-100|auto")]
    avif_alpha_quality: Option<String>,
    /// 0-10
    #[arg(long, value_name = "N")]
    avif_speed: Option<u8>,
    #[arg(long, value_parser = ["8", "10"])]
    avif_depth: Option<String>,
    // ---- heic
    /// 1-100
    #[arg(long, value_name = "N")]
    heic_quality: Option<u8>,
    #[arg(long, value_parser = ["8", "10"])]
    heic_depth: Option<String>,
    // ---- webp
    #[arg(long, value_name = "1-100|lossless")]
    webp_quality: Option<String>,
    /// 0-6
    #[arg(long, value_name = "N")]
    webp_method: Option<u8>,
    /// 1-100
    #[arg(long, value_name = "N")]
    webp_alpha_quality: Option<u8>,
    // ---- gif
    /// 1-3
    #[arg(long, value_name = "N")]
    gif_optimize_level: Option<u8>,
    #[arg(long, value_name = "0-200|off")]
    gif_lossy: Option<String>,
    // ---- svg
    /// 0-5 decimal places kept in SVG numbers and paths
    #[arg(long, value_name = "N")]
    svg_precision: Option<u8>,
    /// Also run cleanupIds/convertTransform/applyTransforms (renames ids; still render-checked)
    #[arg(long, overrides_with = "no_svg_aggressive")]
    svg_aggressive: bool,
    #[arg(long, overrides_with = "svg_aggressive")]
    no_svg_aggressive: bool,
    /// Skip SVGs larger than this, e.g. 500KB, 2MB (0 = no limit)
    #[arg(long, value_name = "SIZE")]
    svg_max_size: Option<String>,
    // ---- quality / metadata extras / resize (extra_args.rs)
    #[command(flatten)]
    extra: crate::extra_args::ExtraArgs,
    // ---- run
    #[arg(long, overrides_with = "no_history")]
    history: bool,
    #[arg(long, overrides_with = "history")]
    no_history: bool,
    /// Worker count (0 = all cores)
    #[arg(long, value_name = "N")]
    jobs: Option<usize>,
    /// Print the plan, change nothing
    #[arg(long)]
    dry_run: bool,
}

pub fn parse_size(s: &str) -> Result<u64, String> {
    let t = s.trim().to_ascii_lowercase();
    let t = t.strip_suffix('b').unwrap_or(&t);
    let (n, mul) = match t.chars().last() {
        Some('k') => (&t[..t.len() - 1], 1024u64),
        Some('m') => (&t[..t.len() - 1], 1024 * 1024),
        Some('g') => (&t[..t.len() - 1], 1024 * 1024 * 1024),
        _ => (t, 1),
    };
    n.trim().parse::<f64>().ok().filter(|v| *v >= 0.0).map(|v| (v * mul as f64) as u64).ok_or_else(|| format!("bad size: {s}"))
}

fn range(v: u64, lo: u64, hi: u64, flag: &str) -> Result<Value, String> {
    if (lo..=hi).contains(&v) { Ok(json!(v)) } else { Err(format!("--{flag} wants {lo}-{hi}")) }
}

/// "<word>" -> null, otherwise an integer in lo..=hi.
fn num_or(v: &str, word: &str, lo: u64, hi: u64, flag: &str) -> Result<Value, String> {
    if v == word {
        return Ok(Value::Null);
    }
    v.parse::<u64>().map_err(|_| format!("--{flag} wants {lo}-{hi} or {word}")).and_then(|n| range(n, lo, hi, flag))
}

fn tri(yes: bool, no: bool) -> Option<Value> {
    if yes { Some(json!(true)) } else if no { Some(json!(false)) } else { None }
}

impl OptFlags {
    /// `current_format` is the effective `format` before flags apply (for `--quality`).
    pub fn overrides(&self, current_format: &str) -> Result<Overrides, String> {
        let mut m = Overrides::new();
        // --quality first; the specific flags below override it.
        if let Some(v) = &self.quality {
            let q = num_or(v, "lossless", 1, 100, "quality")?;
            let fmt = self.format.as_deref().unwrap_or(current_format);
            let ql = q.as_u64().map(|q| q as u8);
            let quant = || ql.map_or(Value::Null, |q| json!([q.saturating_sub(20), q]));
            match fmt {
                "keep" => {
                    m.insert("jpeg-quality".into(), q.clone());
                    m.insert("png-quantize".into(), quant());
                }
                "jpeg" => { m.insert("jpeg-quality".into(), q); }
                "png" => { m.insert("png-quantize".into(), quant()); }
                "webp" => { m.insert("webp-quality".into(), q); }
                "jxl" => { m.insert("jxl-quality".into(), q); }
                "avif" | "heic" => {
                    if q.is_null() {
                        return Err(format!("--quality lossless is not supported for {fmt}"));
                    }
                    m.insert(format!("{fmt}-quality"), q);
                }
                _ => unreachable!(),
            }
        }
        let mut put = |k: &str, v: Option<Value>| {
            if let Some(v) = v {
                m.insert(k.into(), v);
            }
        };
        put("recursive", tri(self.recursive, self.no_recursive));
        put("include-hidden", self.include_hidden.then(|| json!(true)));
        put("min-size", self.min_size.as_deref().map(parse_size).transpose()?.map(|v| json!(v)));
        put("format", self.format.as_ref().map(|v| json!(v)));
        put("destination", self.destination.as_ref().map(|v| json!(v)));
        put("suffix", self.suffix.as_ref().map(|v| json!(v)));
        put("originals", self.originals.as_ref().map(|v| json!(v)));
        put("replace-if", self.replace_if.as_ref().map(|v| json!(v)));
        put("overwrite", self.overwrite.as_ref().map(|v| json!(v)));
        put("preserve-dates", tri(self.preserve_dates, self.no_preserve_dates));
        crate::extra_args::extra_overrides(&self.extra, &mut m)?;
        let mut put = |k: &str, v: Option<Value>| {
            if let Some(v) = v {
                m.insert(k.into(), v);
            }
        };
        put("metadata", self.metadata.as_ref().map(|v| json!(v)));
        put("jpeg-encoder", self.jpeg_encoder.as_ref().map(|v| json!(v)));
        put("jpeg-quality", self.jpeg_quality.as_deref().map(|v| num_or(v, "lossless", 1, 100, "jpeg-quality")).transpose()?);
        put("jpeg-progressive", tri(self.jpeg_progressive, self.no_jpeg_progressive));
        put("jpeg-subsampling", self.jpeg_subsampling.as_ref().map(|v| json!(v)));
        put("png-quantize", self.png_quantize.as_deref().map(parse_quantize).transpose()?);
        put("png-quantize-speed", self.png_quantize_speed.map(|v| range(v as u64, 1, 11, "png-quantize-speed")).transpose()?);
        put("png-dither", tri(self.png_dither, self.no_png_dither));
        put("png-optimize-level", self.png_optimize_level.as_deref().map(|v| if v == "max" { Ok(json!(6)) } else { num_or(v, "\0", 0, 6, "png-optimize-level") }).transpose()?);
        put("png-zopfli", self.png_zopfli.as_deref().map(|v| num_or(v, "off", 1, 255, "png-zopfli")).transpose()?);
        put("jxl-quality", self.jxl_quality.as_deref().map(|v| num_or(v, "lossless", 1, 100, "jxl-quality")).transpose()?);
        put("jxl-effort", self.jxl_effort.map(|v| range(v as u64, 1, 10, "jxl-effort")).transpose()?);
        put("jxl-jpeg-transcode", tri(self.jxl_jpeg_transcode, self.no_jxl_jpeg_transcode));
        put("jxl-progressive", tri(self.jxl_progressive, self.no_jxl_progressive));
        put("avif-quality", self.avif_quality.map(|v| range(v as u64, 1, 100, "avif-quality")).transpose()?);
        put("avif-alpha-quality", self.avif_alpha_quality.as_deref().map(|v| num_or(v, "auto", 1, 100, "avif-alpha-quality")).transpose()?);
        put("avif-speed", self.avif_speed.map(|v| range(v as u64, 0, 10, "avif-speed")).transpose()?);
        put("avif-depth", self.avif_depth.as_ref().map(|v| json!(v.parse::<u8>().unwrap())));
        put("heic-quality", self.heic_quality.map(|v| range(v as u64, 1, 100, "heic-quality")).transpose()?);
        put("heic-depth", self.heic_depth.as_ref().map(|v| json!(v.parse::<u8>().unwrap())));
        put("webp-quality", self.webp_quality.as_deref().map(|v| num_or(v, "lossless", 1, 100, "webp-quality")).transpose()?);
        put("webp-method", self.webp_method.map(|v| range(v as u64, 0, 6, "webp-method")).transpose()?);
        put("webp-alpha-quality", self.webp_alpha_quality.map(|v| range(v as u64, 1, 100, "webp-alpha-quality")).transpose()?);
        put("gif-optimize-level", self.gif_optimize_level.map(|v| range(v as u64, 1, 3, "gif-optimize-level")).transpose()?);
        put("gif-lossy", self.gif_lossy.as_deref().map(|v| num_or(v, "off", 0, 200, "gif-lossy")).transpose()?);
        put("svg-precision", self.svg_precision.map(|v| range(v as u64, 0, 5, "svg-precision")).transpose()?);
        put("svg-aggressive", tri(self.svg_aggressive, self.no_svg_aggressive));
        put("svg-max-size", self.svg_max_size.as_deref().map(parse_size).transpose()?.map(|v| json!(v)));
        put("history", tri(self.history, self.no_history));
        put("jobs", self.jobs.map(|v| json!(v)));
        put("dry-run", self.dry_run.then(|| json!(true)));
        Ok(m)
    }
}

fn parse_quantize(v: &str) -> Result<Value, String> {
    if v == "off" {
        return Ok(Value::Null);
    }
    let bad = || "--png-quantize wants MIN-MAX (0-100) or off".to_string();
    let (lo, hi) = v.split_once('-').ok_or_else(bad)?;
    let (lo, hi): (u8, u8) = (lo.trim().parse().map_err(|_| bad())?, hi.trim().parse().map_err(|_| bad())?);
    if lo > hi || hi > 100 {
        return Err(bad());
    }
    Ok(json!([lo, hi]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes() {
        assert_eq!(parse_size("10KB").unwrap(), 10240);
        assert_eq!(parse_size("2mb").unwrap(), 2 * 1024 * 1024);
        assert_eq!(parse_size("500").unwrap(), 500);
        assert!(parse_size("x").is_err());
    }

    #[test]
    fn quality_shortcut() {
        let f = OptFlags { quality: Some("80".into()), ..Default::default() };
        let m = f.overrides("keep").unwrap();
        assert_eq!(m["jpeg-quality"], json!(80));
        assert_eq!(m["png-quantize"], json!([60, 80]));
        let f = OptFlags { quality: Some("70".into()), format: Some("webp".into()), jpeg_quality: Some("lossless".into()), ..Default::default() };
        let m = f.overrides("keep").unwrap();
        assert_eq!(m["webp-quality"], json!(70));
        assert_eq!(m["jpeg-quality"], Value::Null);
        assert!(OptFlags { quality: Some("lossless".into()), format: Some("avif".into()), ..Default::default() }.overrides("keep").is_err());
        // inherits the preset's format
        let m = OptFlags { quality: Some("55".into()), ..Default::default() }.overrides("heic").unwrap();
        assert_eq!(m["heic-quality"], json!(55));
    }
}
