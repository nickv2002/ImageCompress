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
    match t {
        Fmt::Jpeg => match o.jpeg_quality {
            Some(q) => format!("{:?} q{q}", o.jpeg_encoder).to_lowercase(),
            None => "jpegtran lossless".into(),
        },
        Fmt::Png => {
            let z = o.png_zopfli.map(|n| format!(" + zopfli {n}")).unwrap_or_default();
            match o.png_quantize {
                Some((lo, hi)) => format!("imagequant {lo}-{hi} + oxipng -o{}{z}", o.png_optimize_level),
                None => format!("oxipng -o{}{z}", o.png_optimize_level),
            }
        }
        Fmt::Gif => format!("gifsicle -O{}{}", o.gif_optimize_level, o.gif_lossy.map(|l| format!(" --lossy={l}")).unwrap_or_default()),
        Fmt::Webp => format!("webp {} method {}", quality(o.webp_quality), o.webp_method),
        Fmt::Avif => format!("avif q{} speed {} {}-bit", o.avif_quality, o.avif_speed, o.avif_depth),
        Fmt::Heic => format!("heic q{} {}-bit", o.heic_quality, o.heic_depth),
        Fmt::Jxl => format!("jxl {} effort {}", quality(o.jxl_quality), o.jxl_effort),
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
