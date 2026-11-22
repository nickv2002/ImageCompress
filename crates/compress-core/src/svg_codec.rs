//! SVG / SVGZ optimisation: oxvg (an SVGO port) with a conservative job set, guarded by a resvg
//! render comparison. Same-format only (an SVG never converts to a raster format).
//!
//! Safety layers, in order:
//! 1. The default SVGO job set is trimmed: `cleanupIds` (renames ids that inline `<svg>` used from
//!    HTML/CSS/JS refers to), `convertTransform` and `applyTransforms` (lost precision on huge
//!    rotation centres) are off unless `svg_aggressive` is set.
//! 2. The optimised document is rasterised next to the original (256 px) and rejected when it
//!    differs; the next more conservative job set is then tried, and finally the original is kept.
//! 3. Documents with `<text>`/`<foreignObject>` are only processed with the conservative set, because
//!    the guard renders without fonts and cannot see text.
//! 4. Size cap (`svg_max_size`) and a hard deadline: oxvg has no cancellation API, so a document that
//!    overruns is abandoned on its worker thread and the file is skipped.
use crate::options::Options;
use oxvg_ast::{parse::roxmltree::parse, serialize::Node as _, visitor::Info};
use oxvg_optimiser::Jobs;
use std::io::Read;
use std::time::Duration;

pub enum Svg {
    /// Optimised bytes (gzip-compressed for svgz input) and the encoder label.
    Done(Vec<u8>, String),
    Skip(String),
}

/// Wall-clock budget per file (optimise + render check, all attempts).
const DEADLINE: Duration = Duration::from_secs(30);
/// Render-guard raster size (longer side) and tolerance.
const GUARD_PX: f32 = 256.0;
const GUARD_RMSE: f64 = 3.0;
/// Fraction of pixels allowed to differ by more than `GUARD_PIXEL_DELTA` (catches small missing shapes
/// that barely move the RMSE).
const GUARD_PIXEL_FRACTION: f64 = 0.005;
const GUARD_PIXEL_DELTA: i32 = 64;
/// Upper bound for gunzipping when `svg_max_size` is 0 (unlimited), so a gzip bomb cannot eat memory.
const GUNZIP_HARD_CAP: u64 = 256 << 20;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Tier {
    Aggressive,
    Standard,
    Conservative,
}

impl Tier {
    fn label(self) -> &'static str {
        match self {
            Tier::Aggressive => "oxvg-aggressive",
            Tier::Standard => "oxvg",
            Tier::Conservative => "oxvg-conservative",
        }
    }
}

fn jobs(tier: Tier, precision: u8) -> Jobs {
    let mut j = Jobs::default();
    if tier != Tier::Aggressive {
        j.cleanup_ids = None;
        j.convert_transform = None;
        j.apply_transforms = None;
    }
    if tier == Tier::Conservative {
        j.merge_paths = None;
        j.convert_shape_to_path = None;
        j.collapse_groups = None;
        j.move_elems_attrs_to_group = None;
        j.move_group_attrs_to_elems = None;
        j.remove_hidden_elems = None;
        j.remove_empty_containers = None;
        j.inline_styles = None;
        j.merge_styles = None;
        j.minify_styles = None;
    }
    if let Some(c) = j.convert_path_data.as_mut() {
        c.tolerance.precision = precision as i32;
    }
    if let Some(c) = j.cleanup_numeric_values.as_mut() {
        c.float_precision = precision;
    }
    j
}

fn run_jobs(src: &str, tier: Tier, precision: u8) -> Result<String, String> {
    let j = jobs(tier, precision);
    parse(src, |dom, alloc| -> Result<String, String> {
        j.run(dom, &Info::new(alloc)).map_err(|e| format!("svg optimiser: {e:?}"))?;
        dom.serialize().map_err(|e| format!("svg serialize: {e:?}"))
    })
    .map_err(|e| format!("corrupt input: not a valid SVG ({e:?})"))?
}

struct Raster {
    size: (f32, f32),
    w: u32,
    h: u32,
    px: Vec<u8>,
}

fn render(svg: &str) -> Result<Raster, String> {
    use resvg::{tiny_skia, usvg};
    let tree = usvg::Tree::from_str(svg, &usvg::Options::default()).map_err(|e| e.to_string())?;
    let s = tree.size();
    if !(s.width() > 0.0 && s.height() > 0.0) {
        return Err("empty canvas".into());
    }
    let k = GUARD_PX / s.width().max(s.height());
    let (w, h) = (((s.width() * k).ceil() as u32).clamp(1, 4096), ((s.height() * k).ceil() as u32).clamp(1, 4096));
    let mut pm = tiny_skia::Pixmap::new(w, h).ok_or("pixmap")?;
    resvg::render(&tree, tiny_skia::Transform::from_scale(w as f32 / s.width(), h as f32 / s.height()), &mut pm.as_mut());
    Ok(Raster { size: (s.width(), s.height()), w, h, px: pm.take() })
}

/// True when the two renders are visually the same.
fn renders_match(a: &Raster, b: &Raster) -> bool {
    let close = |x: f32, y: f32| (x - y).abs() <= 1e-3 * x.abs().max(1.0);
    if !(close(a.size.0, b.size.0) && close(a.size.1, b.size.1) && a.w == b.w && a.h == b.h && a.px.len() == b.px.len()) {
        return false;
    }
    let mut sq = 0f64;
    let mut strong = 0usize;
    for (p, q) in a.px.chunks_exact(4).zip(b.px.chunks_exact(4)) {
        let mut worst = 0;
        for c in 0..4 {
            let d = p[c] as i32 - q[c] as i32;
            sq += (d * d) as f64;
            worst = worst.max(d.abs());
        }
        strong += (worst > GUARD_PIXEL_DELTA) as usize;
    }
    let n = a.px.len() as f64;
    (sq / n).sqrt() <= GUARD_RMSE && (strong as f64) <= GUARD_PIXEL_FRACTION * (a.px.len() / 4) as f64
}

fn has_text(src: &str) -> bool {
    src.contains("<text") || src.contains("<foreignObject")
}

/// Runs on the worker thread. `Ok(None)` = every attempt failed the render guard / did not shrink.
fn optimise_text(src: &str, o: &Options) -> Result<Option<(String, Tier)>, String> {
    let tiers: &[Tier] = if has_text(src) {
        &[Tier::Conservative]
    } else if o.svg_aggressive {
        &[Tier::Aggressive, Tier::Standard, Tier::Conservative]
    } else {
        &[Tier::Standard, Tier::Conservative]
    };
    let reference = render(src).map_err(|e| format!("cannot verify render of the original: {e}"))?;
    for &t in tiers {
        let out = run_jobs(src, t, o.svg_precision)?;
        if out.len() >= src.len() {
            continue;
        }
        match render(&out) {
            Ok(r) if renders_match(&reference, &r) => return Ok(Some((out, t))),
            _ => continue,
        }
    }
    Ok(None)
}

fn gzip(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let iterations = if bytes.len() > (256 << 10) { 5 } else { 15 };
    let opts = zopfli::Options { iteration_count: std::num::NonZeroU64::new(iterations).unwrap(), ..Default::default() };
    let mut out = Vec::new();
    zopfli::compress(opts, zopfli::Format::Gzip, bytes, &mut out).map_err(|e| e.to_string())?;
    Ok(out)
}

/// Optimise an `.svg` (`gz = false`) or `.svgz` (`gz = true`) file. Never returns bytes whose render
/// differs from the input beyond the guard tolerance; returns `Skip` when nothing safe is smaller.
pub fn optimize(data: &[u8], gz: bool, o: &Options) -> Result<Svg, String> {
    let cap = if o.svg_max_size == 0 { GUNZIP_HARD_CAP } else { o.svg_max_size };
    let raw: Vec<u8> = if gz {
        let mut buf = Vec::new();
        flate2::read::MultiGzDecoder::new(data)
            .take(cap + 1)
            .read_to_end(&mut buf)
            .map_err(|e| format!("corrupt input: bad svgz gzip stream ({e})"))?;
        buf
    } else {
        data.to_vec()
    };
    if raw.len() as u64 > cap {
        return Ok(Svg::Skip(format!("svg larger than --svg-max-size ({} KB)", cap / 1024)));
    }
    let text = String::from_utf8(raw).map_err(|_| "unsupported svg: not UTF-8".to_string())?;
    let text = text.strip_prefix('\u{feff}').unwrap_or(&text).to_string();
    // cheap root check before spending a thread on it
    if !text.contains("<svg") {
        return Ok(Svg::Skip("not an SVG document (no <svg> root)".into()));
    }
    let o2 = o.clone();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::Builder::new()
        .name("svg-optimise".into())
        .stack_size(64 << 20) // deeply nested documents recurse
        .spawn(move || {
            let r = std::panic::catch_unwind(|| optimise_text(&text, &o2)).unwrap_or_else(|_| Err("svg optimiser panicked".into()));
            let _ = tx.send(r);
        })
        .map_err(|e| e.to_string())?;
    let res = match rx.recv_timeout(DEADLINE) {
        Ok(r) => r?,
        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => return Ok(Svg::Skip(format!("svg optimiser timed out after {}s", DEADLINE.as_secs()))),
        Err(_) => return Err("svg optimiser thread died".into()),
    };
    let Some((out, tier)) = res else {
        return Ok(Svg::Skip("no safe optimisation (render check failed or nothing smaller)".into()));
    };
    let bytes = if gz {
        let z = gzip(out.as_bytes())?;
        // verify the compressed result before it can replace the original
        let mut back = Vec::new();
        flate2::read::MultiGzDecoder::new(&z[..]).read_to_end(&mut back).map_err(|e| format!("output failed verification: {e}"))?;
        if back != out.as_bytes() {
            return Err("output failed verification: svgz does not round-trip".into());
        }
        z
    } else {
        out.into_bytes()
    };
    Ok(Svg::Done(bytes, tier.label().into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIMPLE: &str = r##"<?xml version="1.0" encoding="UTF-8"?>
<!-- Created with Inkscape -->
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100" width="100" height="100">
  <metadata>junk</metadata>
  <rect x="10.000000" y="10.000000" width="80.000000" height="80.000000" fill="#ff0000" stroke="none"/>
  <circle cx="50" cy="50" r="20.123456789" fill="#0000ff"/>
</svg>"##;

    #[test]
    fn shrinks_and_keeps_render() {
        let Svg::Done(b, _) = optimize(SIMPLE.as_bytes(), false, &Options::default()).unwrap() else { panic!("skipped") };
        let s = String::from_utf8(b).unwrap();
        assert!(s.len() < SIMPLE.len() && !s.contains("Inkscape") && !s.contains("metadata"));
        assert!(renders_match(&render(SIMPLE).unwrap(), &render(&s).unwrap()));
    }

    #[test]
    fn guard_rejects_different_renders() {
        let other = SIMPLE.replace("#ff0000", "#00ff00");
        assert!(!renders_match(&render(SIMPLE).unwrap(), &render(&other).unwrap()));
        let moved = SIMPLE.replace(r#"cx="50""#, r#"cx="70""#);
        assert!(!renders_match(&render(SIMPLE).unwrap(), &render(&moved).unwrap()));
    }

    #[test]
    fn ids_survive_by_default() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" viewBox="0 0 10 10">
  <!-- padding comment to make the output smaller than the input -->
  <defs><linearGradient id="gradientForButton"><stop offset="0" stop-color="#fff"/><stop offset="1" stop-color="#000"/></linearGradient></defs>
  <rect id="theButton" width="10.0000" height="10.0000" fill="url(#gradientForButton)"/>
</svg>"##;
        let Svg::Done(b, _) = optimize(svg.as_bytes(), false, &Options::default()).unwrap() else { panic!("skipped") };
        let s = String::from_utf8(b).unwrap();
        assert!(s.contains("gradientForButton") && s.contains("theButton"), "{s}");
    }

    #[test]
    fn text_documents_use_the_conservative_set() {
        let svg = format!("{SIMPLE}").replace("</svg>", "<text x=\"1\" y=\"9\">hi</text></svg>");
        let Svg::Done(_, n) = optimize(svg.as_bytes(), false, &Options::default()).unwrap() else { panic!("skipped") };
        assert_eq!(n, "oxvg-conservative");
    }

    #[test]
    fn svgz_roundtrip() {
        let gz = gzip(SIMPLE.as_bytes()).unwrap();
        let Svg::Done(b, _) = optimize(&gz, true, &Options::default()).unwrap() else { panic!("skipped") };
        let mut s = String::new();
        flate2::read::GzDecoder::new(&b[..]).read_to_string(&mut s).unwrap();
        assert!(s.starts_with("<svg") && s.len() < SIMPLE.len());
    }

    #[test]
    fn guard_keeps_original_when_every_tier_distorts() {
        // precision 0 on a unit-sized coordinate space moves the shapes by whole canvas fractions
        let svg = "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 1 1\" width=\"100\" height=\"100\">\n  <!-- c -->\n  <circle cx=\"0.37000\" cy=\"0.42000\" r=\"0.30000\" fill=\"#ff0000\"/>\n  <rect x=\"0.55000\" y=\"0.60000\" width=\"0.30000\" height=\"0.30000\" fill=\"#0000ff\"/>\n</svg>";
        let o = Options { svg_precision: 0, ..Options::default() };
        assert!(matches!(optimize(svg.as_bytes(), false, &o).unwrap(), Svg::Skip(m) if m.contains("no safe optimisation")));
    }

    #[test]
    fn rejects_garbage_and_caps() {
        assert!(optimize(b"<svg", false, &Options::default()).is_err());
        assert!(matches!(optimize(b"<html></html>", false, &Options::default()).unwrap(), Svg::Skip(_)));
        assert!(optimize(&[0xff, 0xfe, 0, 1], false, &Options::default()).is_err());
        let o = Options { svg_max_size: 50, ..Options::default() };
        assert!(matches!(optimize(SIMPLE.as_bytes(), false, &o).unwrap(), Svg::Skip(_)));
        assert!(optimize(b"not gzip", true, &Options::default()).is_err());
    }
}
