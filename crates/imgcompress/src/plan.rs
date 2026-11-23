//! `--dry-run`: describe what would happen to each file.
use compress_core::convert::Fmt;
use compress_core::discover::ext_of;
use compress_core::options::Options;
use serde_json::json;
use std::path::PathBuf;

fn quality(q: Option<u8>) -> String {
    q.map_or("lossless".into(), |q| format!("q{q}"))
}

fn encoder(t: Fmt, o: &Options) -> String {
    // the per-image quality search replaces the fixed quality for the formats it supports
    let search = o.target_quality.filter(|_| compress_core::quality_search::supports(t)).map(|v| format!("target-quality {v}"));
    let quality_or_search = |q: Option<u8>| search.clone().unwrap_or_else(|| quality(q));
    match t {
        Fmt::Jpeg => match (&search, o.jpeg_quality) {
            (Some(s), _) => format!("{:?} {s}", o.jpeg_encoder).to_lowercase(),
            (None, Some(q)) => format!("{:?} q{q}", o.jpeg_encoder).to_lowercase(),
            (None, None) => "jpegtran lossless".into(),
        },
        Fmt::Png => {
            let z = o.png_zopfli.map(|n| format!(" + zopfli {n}")).unwrap_or_default();
            match o.png_quantize {
                Some((lo, hi)) => format!("imagequant {lo}-{hi} + oxipng -o{}{z}", o.png_optimize_level),
                None => format!("oxipng -o{}{z}", o.png_optimize_level),
            }
        }
        Fmt::Gif => format!("gifsicle -O{}{}", o.gif_optimize_level, o.gif_lossy.map(|l| format!(" --lossy={l}")).unwrap_or_default()),
        Fmt::Webp => format!("webp {} method {}", quality_or_search(o.webp_quality), o.webp_method),
        Fmt::Avif => format!("avif {} speed {} {}-bit", search.clone().unwrap_or_else(|| format!("q{}", o.avif_quality)), o.avif_speed, o.avif_depth),
        Fmt::Heic => format!("heic q{} {}-bit", o.heic_quality, o.heic_depth),
        Fmt::Jxl => format!("jxl {} effort {}", quality_or_search(o.jxl_quality), o.jxl_effort),
        _ => "n/a".into(),
    }
}

pub fn describe(path: &std::path::Path, o: &Options) -> (String, String, String) {
    let ext = ext_of(path).unwrap_or_default();
    let src = Fmt::from_ext(&ext);
    if matches!(ext.as_str(), "svg" | "svgz") {
        let how = format!("oxvg precision {}{}, resvg render check", o.svg_precision, if o.svg_aggressive { " aggressive" } else { "" });
        return (ext.clone(), ext, how);
    }
    let Some(s) = src else { return (ext, "-".into(), "skip: unsupported format".into()) };
    let t = Fmt::from_format(o.format).unwrap_or(s);
    if matches!(s, Fmt::Tiff) && s == t {
        return (ext, ext_of(path).unwrap_or_default(), "skip: unsupported format (no recompression)".into());
    }
    let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    if size < o.min_size {
        return (ext, t.ext().into(), "skip: below min size".into());
    }
    if let Some(r) = compress_core::convert::lossless_jpeg_unavailable(s, t, o) {
        return (ext, t.ext().into(), format!("skip: {r}"));
    }
    if t == Fmt::Jpeg && s != Fmt::Jpeg && o.jpeg_quality.is_none() {
        // another format becoming JPEG with the quality left at its default: encoded at the default quality
        let q = Options { jpeg_quality: Some(compress_core::convert::DEFAULT_JPEG_QUALITY), ..o.clone() };
        return (ext, t.ext().into(), encoder(t, &q));
    }
    (ext, t.ext().into(), encoder(t, o))
}

pub fn print(files: &[PathBuf], o: &Options, as_json: bool) {
    let plan = compress_core::outputs::Plan::build(files, o);
    for f in files {
        let (from, to, how) = describe(f, o);
        let out = plan.get(f).map(|e| e.dest.clone());
        if as_json {
            let (output, skip) = match &out {
                Some(Ok(d)) => (json!(d), json!(null)),
                Some(Err(r)) => (json!(null), json!(r)),
                None => (json!(null), json!(null)),
            };
            println!("{}", json!({"event": "plan", "path": f, "from": from, "to": to, "steps": how, "destination": o.destination, "originals": o.originals, "output": output, "skip": skip}));
        } else {
            let at = match &out {
                Some(Ok(d)) if d != f => format!(" => {}", d.display()),
                Some(Err(r)) => format!(" [skip: {r}]"),
                _ => String::new(),
            };
            println!("{}: {from} -> {to}  [{how}]{at}", f.display());
        }
    }
    if !as_json {
        eprintln!("dry run: {} file(s); destination {}, originals {:?}, nothing was changed", files.len(), o.destination, o.originals);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use compress_core::options::Format;

    fn fixture(rel: &str) -> PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures").join(rel)
    }

    #[test]
    fn lossless_jpeg_from_other_formats_is_planned_as_a_skip() {
        let png = fixture("png/rgba_photo.png");
        // asked for lossless (a preset, options file or flag set jpeg-quality to null): skipped
        let o = Options { format: Format::Jpeg, jpeg_quality: None, jpeg_lossless_requested: true, ..Default::default() };
        let (_, to, how) = describe(&png, &o);
        assert_eq!(to, "jpg");
        assert!(how.starts_with("skip: JPEG has no lossless mode for .png input"), "{how}");
        // quality left at its default: encoded at the default quality, as before
        let (_, _, how) = describe(&png, &Options { jpeg_lossless_requested: false, ..o.clone() });
        assert_eq!(how, "jpegli q85");
        let (_, _, how) = describe(&png, &Options { jpeg_quality: Some(80), ..o.clone() });
        assert_eq!(how, "jpegli q80");
        let (_, _, how) = describe(&png, &Options { target_quality: Some(88.0), ..o.clone() });
        assert_eq!(how, "jpegli target-quality 88");
        // a JPEG still repacks losslessly
        let (_, _, how) = describe(&fixture("jpeg/baseline.jpg"), &o);
        assert_eq!(how, "jpegtran lossless");
        // keep leaves a PNG a PNG
        let (_, to, how) = describe(&png, &Options { format: Format::Keep, ..o });
        assert_eq!(to, "png");
        assert!(how.starts_with("oxipng"), "{how}");
    }
}
