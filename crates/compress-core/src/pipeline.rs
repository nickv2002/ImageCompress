use crate::convert::{self, Conv, Fmt};
use crate::discover::{discover, ext_of};
use crate::events::JobEvent;
use crate::options::{Options, Originals, ReplaceIf};
use crate::outputs::{self, Plan};
use crate::png_codec;
use rayon::prelude::*;
use crate::undo::{self, Undo};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

pub struct Outcome {
    pub processed: usize,
    pub skipped: usize,
    pub failed: usize,
}

enum Res {
    Done { output: PathBuf, in_b: u64, out_b: u64, encoder: String, trace: Vec<(String, u64)>, undo: Option<Undo>, note: Option<String> },
    Skip(String),
}

/// Run a job. `emit` is called for every event (possibly from worker threads).
pub fn run(paths: &[PathBuf], o: &Options, cancel: &AtomicBool, emit: &(dyn Fn(JobEvent) + Sync)) -> Outcome {
    let files = discover(paths, o);
    emit(JobEvent::Queued { files: files.len() });
    let plan = Plan::build(&files, o);
    let pool = rayon::ThreadPoolBuilder::new().num_threads(o.jobs).build().expect("pool");
    let tot = Mutex::new((0usize, 0usize, 0usize, 0u64, 0u64));
    pool.install(|| {
        files.par_iter().for_each(|p| {
            if cancel.load(Ordering::Relaxed) {
                return;
            }
            emit(JobEvent::Started { path: p.clone() });
            let r = process(p, o, &plan, emit);
            let mut t = tot.lock().unwrap();
            match r {
                Ok(Res::Done { output, in_b, out_b, encoder, trace, undo, note }) => {
                    t.0 += 1;
                    t.3 += in_b;
                    t.4 += out_b;
                    emit(JobEvent::Finished { path: p.clone(), output, in_bytes: in_b, out_bytes: out_b, encoder, trace, undo, note });
                }
                Ok(Res::Skip(reason)) => {
                    t.1 += 1;
                    emit(JobEvent::Skipped { path: p.clone(), reason });
                }
                Err(error) => {
                    t.2 += 1;
                    emit(JobEvent::Failed { path: p.clone(), error });
                }
            }
        })
    });
    let t = tot.into_inner().unwrap();
    emit(JobEvent::Totals { processed: t.0, skipped: t.1, failed: t.2, in_bytes: t.3, out_bytes: t.4 });
    Outcome { processed: t.0, skipped: t.1, failed: t.2 }
}

fn process(p: &Path, o: &Options, plan: &Plan, emit: &(dyn Fn(JobEvent) + Sync)) -> Result<Res, String> {
    // the output name is taken and `overwrite = skip`: decided before any work is done
    if let Some(Err(reason)) = plan.get(p).map(|e| &e.dest) {
        return Ok(Res::Skip(reason.clone()));
    }
    let data = fs::read(p).map_err(|e| e.to_string())?;
    let in_b = data.len() as u64;
    if in_b == 0 {
        return Ok(Res::Skip("empty file".into()));
    }
    if in_b < o.min_size {
        return Ok(Res::Skip("below min size".into()));
    }
    let ext = ext_of(p).unwrap_or_default();
    let stage = |s: &str| emit(JobEvent::Stage { path: p.to_path_buf(), stage: s.into() });
    let mut trace = vec![("original".to_string(), in_b)];
    let src_fmt = Fmt::from_ext(&ext);
    // --anim-to webp|avif also moves a still GIF to that format; animated inputs are routed by `anim::route` below
    let gif_still_to = match (src_fmt, o.anim_to) {
        (Some(Fmt::Gif), crate::options::AnimTo::Webp) => Some(Fmt::Webp),
        (Some(Fmt::Gif), crate::options::AnimTo::Avif) => Some(Fmt::Avif),
        _ => None,
    };
    let target = Fmt::from_format(o.format).or(gif_still_to).or(src_fmt);
    let converting = match (src_fmt, target) {
        (Some(s), Some(t)) => s != t || !matches!(s, Fmt::Jpeg | Fmt::Png | Fmt::Gif) || crate::extras::pixel_changes_requested(&data, s, o),
        _ => false,
    };
    let mut out_ext = ext.clone();
    // Animated inputs must never reach a still decoder/encoder (they would silently keep frame 1).
    let frames_in = src_fmt.map_or(1, |f| crate::anim::frame_count(&data, f));
    let mut animated_native = false;
    let mut anim = None;
    if frames_in > 1 {
        match crate::anim::route(&data, src_fmt.unwrap(), o, &stage)? {
            crate::anim::Route::Skip(r) => return Ok(Res::Skip(r)),
            crate::anim::Route::Native => animated_native = true,
            crate::anim::Route::Encoded { bytes, ext, encoder } => anim = Some((bytes, ext, encoder)),
        }
    }
    let converting = converting && !animated_native;
    let (bytes, encoder) = if let Some((b, ext, name)) = anim {
        out_ext = ext.into();
        trace.push((name.clone(), b.len() as u64));
        (b, name)
    } else if converting {
        let (s, t) = (src_fmt.unwrap(), target.unwrap());
        if matches!(s, Fmt::Tiff) && s == t {
            return Ok(Res::Skip("unsupported format: .tiff (no recompression)".into()));
        }
        if s != t {
            out_ext = t.ext().to_string();
        }
        match convert::convert(&data, s, t, o, &stage)? {
            Conv::Done(b, n) => {
                trace.push((n.clone(), b.len() as u64));
                (b, n)
            }
            Conv::Skip(r) => return Ok(Res::Skip(r)),
        }
    } else {
        match ext.as_str() {
        "png" => {
            let mut best: Option<(Vec<u8>, &str)> = None;
            // quantizing re-encodes pixels as a single still: never for an APNG
            if let Some(q) = o.png_quantize.filter(|_| frames_in <= 1) {
                stage("quantize");
                if let Some(b) = png_codec::quantize_then_optimize(&data, o, q).map_err(|e| e.to_string())? {
                    trace.push(("imagequant+oxipng".into(), b.len() as u64));
                    best = Some((b, "imagequant+oxipng"));
                }
            }
            if best.is_none() {
                stage("optimize");
                let b = png_codec::optimize_lossless(&data, o).map_err(|e| e.to_string())?;
                trace.push(("oxipng".into(), b.len() as u64));
                best = Some((b, "oxipng"));
            }
            let (b, n) = best.unwrap();
            (b, n.to_string())
        }
        "jpg" | "jpeg" => {
            let tail = data.iter().rposition(|&b| b != 0).map(|i| &data[..=i]).unwrap_or(&[]);
            if !tail.ends_with(&[0xFF, 0xD9]) {
                return Err("corrupt input: truncated JPEG (no EOI marker)".into());
            }
            image::load_from_memory_with_format(&data, image::ImageFormat::Jpeg).map_err(|e| format!("corrupt input: {e}"))?;
            let (b, n) = crate::jpeg_pipeline::process(&data, o, &stage)?;
            trace.push((n.clone(), b.len() as u64));
            (b, n)
        }
        "gif" => {
            stage("gifsicle");
            match crate::gif_codec::optimize(&data, o)? {
                Some(b) => {
                    trace.push(("gifsicle".into(), b.len() as u64));
                    (b, "gifsicle".to_string())
                }
                None => return Err("gifsicle helper not found".into()),
            }
        }
        "svg" | "svgz" => {
            // same-format only: --format does not apply to vector input
            stage("svg");
            match crate::svg_codec::optimize(&data, ext == "svgz", o)? {
                crate::svg_codec::Svg::Done(b, n) => {
                    trace.push((n.clone(), b.len() as u64));
                    (b, n)
                }
                crate::svg_codec::Svg::Skip(r) => return Ok(Res::Skip(r)),
            }
        }
        other => return Ok(Res::Skip(format!("unsupported format: .{other}"))),
        }
    };
    let reencoded = converting || o.jpeg_quality.is_some();
    let bytes = crate::extras::finalize(&data, bytes, src_fmt, &out_ext, o, reencoded);
    let format_changed = Fmt::from_ext(&out_ext) != src_fmt;
    if !size_ok(o.replace_if, bytes.len() as u64, in_b, format_changed) {
        return Ok(Res::Skip("not smaller".into()));
    }
    if o.dry_run {
        return Ok(Res::Skip("dry run".into()));
    }
    // verify the candidate decodes before touching the original
    match Fmt::from_ext(&out_ext) {
        // an animated input must come out with all its frames (animated WebP also needs WebPAnimDecoder, not WebPDecodeRGBA)
        Some(f) if frames_in > 1 => crate::anim::verify(&bytes, f, frames_in)?,
        Some(f @ (Fmt::Webp | Fmt::Avif | Fmt::Jxl | Fmt::Heic)) => {
            convert::decode_any(&bytes, f).map_err(|e| format!("output failed verification: {e}"))?;
        }
        // svg/svgz: the codec's render guard already parsed and rasterised the result
        None if matches!(out_ext.as_str(), "svg" | "svgz") => {}
        _ => {
            image::load_from_memory(&bytes).map_err(|e| format!("output failed verification: {e}"))?;
        }
    }
    let (out_path, undo) = commit(p, &bytes, o, &out_ext, plan)?;
    // the palette was asked for but could not reach its quality floor (photos): the PNG is lossless instead
    let note = (o.png_quantize.is_some() && out_ext == "png" && encoder == "oxipng" && frames_in <= 1).then(|| PNG_NO_PALETTE.to_string());
    Ok(Res::Done { output: out_path, in_b, out_b: bytes.len() as u64, encoder, trace, undo, note })
}

/// Note on a PNG that was written lossless because a colour palette would have changed it visibly.
pub const PNG_NO_PALETTE: &str = "no colour palette: it would visibly change this image, so the PNG is lossless. Photos are smaller as JPEG, WebP or JPEG XL";

/// The `replace-if` size guard: may a result of `out_b` bytes made from `in_b` bytes be kept?
fn size_ok(r: ReplaceIf, out_b: u64, in_b: u64, format_changed: bool) -> bool {
    match r {
        ReplaceIf::Always => true,
        ReplaceIf::SmallerOrConverted if format_changed => true,
        ReplaceIf::Smaller | ReplaceIf::SmallerOrConverted => out_b < in_b,
    }
}

fn commit(src: &Path, bytes: &[u8], o: &Options, ext: &str, plan: &Plan) -> Result<(PathBuf, Option<Undo>), String> {
    let (dest, in_place, replaces) = match plan.get(src) {
        Some(e) if e.ext.eq_ignore_ascii_case(ext) => (e.dest.clone().map_err(|r| format!("{r}; not overwriting"))?, e.own, e.replaces),
        // the plan assumed another extension (the input changed since planning): never overwrite anything
        _ => {
            let d = outputs::dest_for(src, ext, o, None);
            let own = d == src;
            (d, own, false)
        }
    };
    let dir = dest.parent().unwrap_or(Path::new(".")).to_path_buf();
    if !in_place && !matches!(o.destination.as_str(), "in-place" | "alongside") {
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    }
    // tempfile's error names the random temp path; say which folder failed instead
    let mut tmp = tempfile::Builder::new().prefix(".imgcompress-").tempfile_in(&dir).map_err(|e| folder_error(&dir, &e))?;
    tmp.write_all(bytes).map_err(|e| folder_error(&dir, &e))?;
    let src_meta = fs::metadata(src).ok();
    let mtime = src_meta.as_ref().and_then(|m| m.modified().ok());
    // tempfiles are created 0600; keep the source's permission bits so compressing a file doesn't make it private
    if let Some(m) = &src_meta {
        let _ = tmp.as_file().set_permissions(m.permissions());
    }
    // Keep what this job is about to replace or remove; if that is not possible the file is left alone.
    let mut restore = vec![];
    if let Some(bd) = &o.backup_dir {
        if in_place || replaces {
            restore.push(undo::Restore { path: dest.clone(), backup: undo::backup(bd, &dest).map_err(|e| format!("{e}; original left untouched"))? });
        }
        if !in_place && !matches!(o.originals, Originals::Keep) {
            restore.push(undo::Restore { path: src.to_path_buf(), backup: undo::backup(bd, src).map_err(|e| format!("{e}; original left untouched"))? });
        }
    }
    undo::copy_xattrs(src, tmp.path());
    if in_place || replaces {
        // atomic rename over the one file this job may replace
        tmp.persist(&dest).map_err(|e| e.to_string())?;
    } else {
        // atomic and refuses to replace: a file that appeared after planning is never overwritten
        undo::persist_new(tmp, &dest).map_err(|e| match e.kind() {
            std::io::ErrorKind::AlreadyExists => format!("{} appeared during the run; not overwriting", dest.display()),
            _ => e.to_string(),
        })?;
    }
    if o.preserve_dates {
        if let (Some(t), Ok(f)) = (mtime, fs::OpenOptions::new().write(true).open(&dest)) {
            let _ = f.set_modified(t);
        }
    }
    if !in_place {
        match o.originals {
            Originals::Keep => {}
            Originals::Trash => {
                if move_to_trash(src).is_err() {
                    // Leave nothing changed: take the new file back out (or put back the one it replaced)
                    // and drop the now unused backup of the original.
                    match restore.iter().find(|r| r.path == dest) {
                        Some(r) => { let _ = undo::place(&r.backup, &dest, true); }
                        None if !replaces => { let _ = fs::remove_file(&dest); }
                        None => {}
                    }
                    for r in restore.iter().filter(|r| r.path == src) {
                        let _ = fs::remove_file(&r.backup);
                        if let Some(slot) = r.backup.parent() { let _ = fs::remove_dir(slot); }
                    }
                    return Err("could not move the original to the Trash, so nothing was changed. Network shares often have no Trash: keep or delete originals there instead".into());
                }
            }
            Originals::Delete => fs::remove_file(src).map_err(|e| e.to_string())?,
        }
    }
    let undo = o.backup_dir.is_some().then(|| Undo { output: dest.clone(), out_size: bytes.len() as u64, out_hash: undo::hash_bytes(bytes), restore });
    Ok((dest, undo))
}

/// Trash via NSFileManager on macOS (the default Finder/AppleScript path times out
/// when the caller isn't allowed to automate Finder).
pub(crate) fn move_to_trash(p: &Path) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        use trash::macos::{DeleteMethod, TrashContextExtMacos};
        let mut ctx = trash::TrashContext::default();
        ctx.set_delete_method(DeleteMethod::NsFileManager);
        ctx.delete(p).map_err(|e| e.to_string())
    }
    #[cfg(not(target_os = "macos"))]
    {
        trash::delete(p).map_err(|e| e.to_string())
    }
}

/// "cannot write to /some/folder: permission denied", without the internal temp file name.
fn folder_error(dir: &Path, e: &std::io::Error) -> String {
    let why = match e.kind() {
        std::io::ErrorKind::PermissionDenied => "permission denied".to_string(),
        std::io::ErrorKind::NotFound => "the folder does not exist".to_string(),
        std::io::ErrorKind::ReadOnlyFilesystem => "the volume is read-only".to_string(),
        std::io::ErrorKind::StorageFull => "the disk is full".to_string(),
        _ => match e.raw_os_error() {
            Some(n) => std::io::Error::from_raw_os_error(n).to_string(),
            None => e.kind().to_string(),
        },
    };
    format!("cannot write to {}: {why}", dir.display())
}
