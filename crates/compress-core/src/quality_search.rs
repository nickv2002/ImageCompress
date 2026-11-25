//! Perceptual quality search: find, per image, the lowest encoder quality whose SSIMULACRA2 score
//! (fast-ssim2) against the source pixels still reaches `target_quality`.
//!
//! The score is monotone in the encoder quality, so the search is a bracketed secant/regula-falsi
//! over integer qualities. Each probe is one encode, one decode and one SSIMULACRA2 compare against a
//! precomputed reference. For large images the probes run on a mosaic of native-resolution tiles
//! (downscaled copies were tried first and hide artefacts by a quality-dependent amount); the
//! winning quality is then confirmed on the full image and nudged up if it falls short.
use crate::convert::{self, Fmt};
use crate::options::Options;
use crate::pixels::{CResult, Decoded, Rgba};
use fast_ssim2::{PixelDescriptor, PixelSlice, Ssimulacra2Reference};

/// Images above this many pixels are searched on a tile mosaic instead of in full.
const PROXY_ABOVE: u64 = 2_500_000;
/// Tile edge (multiple of 64 so tiles keep the JPEG/AV1/WebP block grid) and mosaic grid.
const TILE: u32 = 448;
const GRID: u32 = 3;

/// (min, max, default start, typical SSIM2 points per quality step)
fn range(f: Fmt) -> Option<(u8, u8, u8, f64)> {
    Some(match f {
        Fmt::Jpeg => (10, 99, 85, 0.7),
        Fmt::Webp => (10, 99, 80, 0.7),
        Fmt::Avif => (10, 95, 60, 0.7),
        Fmt::Jxl => (10, 99, 80, 0.7),
        _ => return None,
    })
}

pub fn supports(f: Fmt) -> bool {
    range(f).is_some()
}

/// Composite onto mid grey (as scripts/bench.py does) and drop alpha.
fn flatten_grey(im: &Rgba) -> Vec<u8> {
    let mut v = Vec::with_capacity(im.px.len() / 4 * 3);
    for p in im.px.chunks_exact(4) {
        let a = p[3] as u32;
        for c in &p[..3] {
            v.push(((*c as u32 * a + 128 * (255 - a) + 127) / 255) as u8);
        }
    }
    v
}

struct Scorer {
    reference: Ssimulacra2Reference,
    dims: (u32, u32),
}

fn slice(rgb: &[u8], w: u32, h: u32) -> CResult<PixelSlice<'_>> {
    PixelSlice::new(rgb, w, h, w as usize * 3, PixelDescriptor::RGB8_SRGB).map_err(|e| format!("ssim2: {e:?}"))
}

impl Scorer {
    fn new(src: &Rgba) -> CResult<Scorer> {
        let rgb = flatten_grey(src);
        let reference = Ssimulacra2Reference::new(&slice(&rgb, src.w, src.h)?).map_err(|e| format!("ssim2: {e:?}"))?;
        Ok(Scorer { reference, dims: (src.w, src.h) })
    }

    fn score(&self, cand: &Rgba) -> CResult<f64> {
        if (cand.w, cand.h) != self.dims {
            return Err("quality search: decoded size differs from source".into());
        }
        let rgb = flatten_grey(cand);
        self.reference.compare(&slice(&rgb, cand.w, cand.h)?).map_err(|e| format!("ssim2: {e:?}"))
    }
}

/// SSIMULACRA2 of `cand` against `src`: 100 means identical; about 90 is hard to tell apart even up close,
/// 70 shows small differences on close inspection, 50 and below is clearly different.
pub fn score_pair(src: &Rgba, cand: &Rgba) -> CResult<f64> {
    if (src.w, src.h) != (cand.w, cand.h) {
        return Err(format!("the images are different sizes ({}×{} and {}×{})", src.w, src.h, cand.w, cand.h));
    }
    Scorer::new(src)?.score(cand)
}

/// Decodes two image files (EXIF orientation applied) and scores the second against the first.
pub fn score_files(src: &std::path::Path, cand: &std::path::Path) -> CResult<f64> {
    let load = |p: &std::path::Path| -> CResult<Rgba> {
        let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
        let fmt = convert::Fmt::from_ext(&ext).ok_or_else(|| format!("{}: not a supported image", p.display()))?;
        let data = std::fs::read(p).map_err(|e| format!("{}: {e}", p.display()))?;
        Ok(convert::decode_any(&data, fmt).map_err(|e| format!("{}: {e}", p.display()))?.img)
    };
    score_pair(&load(src)?, &load(cand)?)
}

/// GRID x GRID native-resolution tiles sampled across the image, stitched into one small image.
fn mosaic(im: &Rgba) -> Rgba {
    let tw = TILE.min(im.w);
    let th = TILE.min(im.h);
    let (mw, mh) = (tw * GRID, th * GRID);
    let mut px = vec![0u8; (mw * mh * 4) as usize];
    for gy in 0..GRID {
        for gx in 0..GRID {
            // tile centres at 1/6, 3/6, 5/6 of the image, origin snapped down to a multiple of 64
            let cx = im.w as u64 * (2 * gx as u64 + 1) / (2 * GRID as u64);
            let cy = im.h as u64 * (2 * gy as u64 + 1) / (2 * GRID as u64);
            let ox = ((cx.saturating_sub(tw as u64 / 2)) as u32 / 64 * 64).min(im.w - tw);
            let oy = ((cy.saturating_sub(th as u64 / 2)) as u32 / 64 * 64).min(im.h - th);
            for row in 0..th {
                let s = (((oy + row) * im.w + ox) * 4) as usize;
                let d = (((gy * th + row) * mw + gx * tw) * 4) as usize;
                px[d..d + (tw * 4) as usize].copy_from_slice(&im.px[s..s + (tw * 4) as usize]);
            }
        }
    }
    Rgba { w: mw, h: mh, px }
}

struct Probe {
    q: u8,
    bytes: Vec<u8>,
    name: String,
    score: f64,
}

fn debug() -> bool {
    std::env::var_os("IMGCOMPRESS_QS_DEBUG").is_some()
}

fn with_quality(o: &Options, f: Fmt, q: u8) -> Options {
    let mut o2 = o.clone();
    o2.target_quality = None;
    match f {
        Fmt::Jpeg => o2.jpeg_quality = Some(q),
        Fmt::Webp => o2.webp_quality = Some(q),
        Fmt::Avif => o2.avif_quality = q,
        Fmt::Jxl => o2.jxl_quality = Some(q),
        _ => {}
    }
    o2
}

fn probe(d: &Decoded, f: Fmt, o: &Options, scorer: &Scorer, q: u8) -> CResult<Probe> {
    let (bytes, name) = convert::encode_fixed(d, f, &with_quality(o, f, q), &|_| {})?;
    let dec = convert::decode_any(&bytes, f)?;
    let score = scorer.score(&dec.img)?;
    if debug() {
        eprintln!("  qs {:?} q={q} {}x{} bytes={} score={score:.2}", f, d.img.w, d.img.h, bytes.len());
    }
    Ok(Probe { q, bytes, name, score })
}

/// Bracketed secant search on `d`. Returns every probe plus the slope (SSIM2 points per quality step).
fn search(d: &Decoded, f: Fmt, o: &Options, target: f64, tol: f64, budget: usize) -> CResult<(Vec<Probe>, f64)> {
    let (qmin, qmax, qstart, slope0) = range(f).expect("supported");
    let scorer = Scorer::new(&d.img)?;
    let mut probes: Vec<Probe> = Vec::new();
    // aim for the middle of [target, target + tol]
    let aim = target + tol * 0.5;
    let mut lo: Option<(u8, f64)> = None; // highest quality known to miss the target
    let mut hi: Option<(u8, f64)> = None; // lowest quality known to reach it
    let mut q = qstart.clamp(qmin, qmax);
    let mut slope = slope0;
    for _ in 0..budget {
        if probes.iter().any(|p| p.q == q) {
            break;
        }
        let p = probe(d, f, o, &scorer, q)?;
        let s = p.score;
        probes.push(p);
        if s >= target {
            if hi.is_none_or(|(hq, _)| q < hq) {
                hi = Some((q, s));
            }
            if s <= target + tol {
                break;
            }
        } else if lo.is_none_or(|(lq, _)| q > lq) {
            lo = Some((q, s));
        }
        if let (Some((lq, ls)), Some((hq, hs))) = (lo, hi) {
            if hq - lq <= 1 {
                break;
            }
            let sl = (hs - ls) / (hq - lq) as f64;
            if sl > 0.05 {
                slope = sl;
            }
            let est = lq as f64 + (aim - ls) / slope;
            q = (est.round() as i64).clamp(lq as i64 + 1, hq as i64 - 1) as u8;
        } else {
            // one-sided: extrapolate with the typical slope, moving at least 1 and at most 25 steps
            let delta = (aim - s) / slope;
            let step = delta.abs().clamp(1.0, 25.0).round() as i32 * if delta >= 0.0 { 1 } else { -1 };
            let nq = (q as i32 + step).clamp(qmin as i32, qmax as i32) as u8;
            if nq == q {
                break; // pinned at a bound
            }
            q = nq;
        }
    }
    Ok((probes, slope))
}

/// Smallest passing probe, else the best-scoring one.
fn pick(probes: &[Probe], target: f64) -> Option<usize> {
    probes
        .iter()
        .enumerate()
        .filter(|(_, p)| p.score >= target)
        .min_by_key(|(_, p)| p.bytes.len())
        .map(|(i, _)| i)
        .or_else(|| probes.iter().enumerate().max_by(|a, b| a.1.score.total_cmp(&b.1.score)).map(|(i, _)| i))
}

/// Returns `Ok(None)` when the search does not apply (no target set, or a format without a lossy
/// quality knob), so the caller falls back to the fixed-quality encode.
pub fn try_search(d: &Decoded, f: Fmt, o: &Options, stage: &dyn Fn(&str)) -> CResult<Option<(Vec<u8>, String)>> {
    let Some(target) = o.target_quality else { return Ok(None) };
    let Some((_, qmax, _, _)) = range(f) else { return Ok(None) };
    if d.img.w < 8 || d.img.h < 8 {
        return Ok(None);
    }
    stage("quality-search");
    let target = target as f64;
    let tol = (o.target_tolerance as f64).max(0.0);
    let budget = o.max_encodes.max(1) as usize;
    let big = d.img.w as u64 * d.img.h as u64 > PROXY_ABOVE;

    let (mut probes, slope) = if big {
        let proxy = Decoded::new(mosaic(&d.img), None);
        search(&proxy, f, o, target, tol, budget)?
    } else {
        search(d, f, o, target, tol, budget)?
    };
    let Some(bi) = pick(&probes, target) else { return Err("quality search produced no candidate".into()) };
    let mut encodes = probes.len();
    let mut win = probes.swap_remove(bi);

    if big {
        // Confirm on the full image; raise quality while it falls short (at most twice).
        let scorer = Scorer::new(&d.img)?;
        let mut q = win.q;
        for attempt in 0..3 {
            let p = probe(d, f, o, &scorer, q)?;
            encodes += 1;
            let ok = p.score >= target;
            let miss = target + tol * 0.5 - p.score;
            win = p;
            if ok || attempt == 2 || q >= qmax {
                break;
            }
            q = (q as f64 + (miss / slope).ceil().max(1.0)).min(qmax as f64) as u8;
        }
    }
    stage(&format!("quality-search q={} ssim2={:.1} encodes={encodes}", win.q, win.score));
    Ok(Some((win.bytes, win.name)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synth(w: u32, h: u32) -> Rgba {
        let mut px = Vec::new();
        for y in 0..h {
            for x in 0..w {
                let n = ((x * 7919 + y * 104729) % 31) as u8;
                px.extend_from_slice(&[(x * 255 / w) as u8 ^ n, (y * 255 / h) as u8, ((x + y) % 64 * 4) as u8, 255]);
            }
        }
        Rgba { w, h, px }
    }

    #[test]
    fn search_meets_target_and_beats_fixed() {
        let d = Decoded::new(synth(256, 256), None);
        let mut o = Options::default();
        o.target_quality = Some(70.0);
        o.max_encodes = 8;
        let (b, _) = try_search(&d, Fmt::Jpeg, &o, &|_| {}).unwrap().unwrap();
        let dec = convert::decode_any(&b, Fmt::Jpeg).unwrap();
        let s = Scorer::new(&d.img).unwrap().score(&dec.img).unwrap();
        assert!(s >= 70.0, "score {s}");
        o.target_quality = Some(50.0);
        let (b2, _) = try_search(&d, Fmt::Jpeg, &o, &|_| {}).unwrap().unwrap();
        assert!(b2.len() <= b.len());
    }

    #[test]
    fn no_target_means_no_search() {
        let d = Decoded::new(synth(64, 64), None);
        assert!(try_search(&d, Fmt::Jpeg, &Options::default(), &|_| {}).unwrap().is_none());
        let mut o = Options::default();
        o.target_quality = Some(70.0);
        assert!(try_search(&d, Fmt::Png, &o, &|_| {}).unwrap().is_none(), "png has no quality knob");
    }

    #[test]
    fn webp_search_runs() {
        let d = Decoded::new(synth(128, 96), None);
        let mut o = Options::default();
        o.target_quality = Some(60.0);
        let (b, _) = try_search(&d, Fmt::Webp, &o, &|_| {}).unwrap().unwrap();
        assert!(convert::decode_any(&b, Fmt::Webp).is_ok());
    }

    #[test]
    fn large_image_uses_mosaic_and_confirms_full() {
        let d = Decoded::new(synth(1920, 1400), None); // 2.7 MP > PROXY_ABOVE
        let m = mosaic(&d.img);
        assert_eq!((m.w, m.h), (TILE * GRID, TILE * GRID));
        let mut o = Options::default();
        o.target_quality = Some(70.0);
        let (b, _) = try_search(&d, Fmt::Jpeg, &o, &|_| {}).unwrap().unwrap();
        let dec = convert::decode_any(&b, Fmt::Jpeg).unwrap();
        let s = Scorer::new(&d.img).unwrap().score(&dec.img).unwrap();
        assert!(s >= 70.0, "full-res score {s}");
    }

    #[test]
    fn score_pair_identical_and_mismatched() {
        let px: Vec<u8> = (0..64 * 64).flat_map(|i| [(i % 251) as u8, (i * 7 % 256) as u8, (i / 64 * 4) as u8, 255]).collect();
        let a = Rgba { w: 64, h: 64, px };
        assert!(score_pair(&a, &a.clone()).unwrap() > 99.0);
        let b = Rgba { w: 32, h: 64, px: a.px[..32 * 64 * 4].to_vec() };
        assert!(score_pair(&a, &b).unwrap_err().contains("different sizes"));
    }
}
