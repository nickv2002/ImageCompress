//! Output naming: where every input of a run writes its result, decided up front.
//!
//! The plan is computed once, single-threaded and from the sorted input list, so the names depend only on
//! the set of inputs, the options and what is on disk, never on worker timing.
//!
//! * A job whose output path is its own source (same-format `in-place`, empty suffix) keeps that path and is
//!   replaced atomically. Nothing else may take that name.
//! * Two or more inputs that map to one name are *colliding*. A collider whose format changes keeps its
//!   source extension in the name (`a.png`, `a.avif` -> `a-w.png.webp`, `a-w.avif.webp`), so neither is
//!   privileged. A collider whose format does not change keeps the plain name. If names still clash (same
//!   extension from different folders into one destination folder) the later input in path order gets
//!   `-2`, `-3`, ...
//! * A name taken by a file that is not this job's source (on disk, or another input of the run) follows
//!   `Options::overwrite`: skip (default), rename to the first free `-N`, or replace. `replace` never
//!   replaces an input of the same run.
//! * Names are compared case-insensitively (APFS and NTFS are), so `A.webp` and `a.webp` are one name.
use crate::convert::Fmt;
use crate::discover::ext_of;
use crate::options::{AnimTo, Format, Options, Overwrite};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

/// The planned output of one input.
#[derive(Debug, Clone)]
pub struct Entry {
    /// Extension the plan assumed (lower-case unless it is the source's own spelling).
    pub ext: String,
    /// Where the result goes, or why the input is skipped (the name is taken and `overwrite` is `skip`).
    pub dest: Result<PathBuf, String>,
    /// The destination is the input itself: replaced atomically, the original is not trashed or deleted.
    pub own: bool,
    /// `dest` exists and `overwrite = replace` allows replacing it.
    pub replaces: bool,
}

#[derive(Debug, Default)]
pub struct Plan {
    entries: HashMap<PathBuf, Entry>,
}

impl Plan {
    pub fn get(&self, src: &Path) -> Option<&Entry> {
        self.entries.get(src)
    }

    pub fn build(files: &[PathBuf], o: &Options) -> Plan {
        let mut items: Vec<Item> = files
            .iter()
            .filter_map(|src| {
                let ext = predict_ext(src, o)?;
                let src_ext = ext_of(src).unwrap_or_default();
                let tagged = changed(src, &ext).then(|| dest_for(src, &ext, o, Some(&src_ext)));
                Some(Item { src: src.clone(), natural: dest_for(src, &ext, o, None), tagged, ext })
            })
            .collect();
        items.sort_by(|a, b| a.src.cmp(&b.src));
        items.dedup_by(|a, b| a.src == b.src);
        let inputs: HashSet<String> = items.iter().map(|i| key(&i.src)).collect();
        let mut st = State { claimed: HashSet::new(), inputs, policy: o.overwrite };
        let mut plan = Plan::default();

        // 1. jobs that write over their own source keep that name
        let mut rest = Vec::new();
        for it in items {
            if same_file(&it.natural, &it.src) {
                st.claimed.insert(key(&it.natural));
                let e = Entry { ext: it.ext.clone(), dest: Ok(it.natural.clone()), own: true, replaces: false };
                plan.entries.insert(it.src.clone(), e);
            } else {
                rest.push(it);
            }
        }
        // 2. group by natural name; a name claimed in step 1 makes its group a collision too
        let mut groups: BTreeMap<String, Vec<Item>> = BTreeMap::new();
        for it in rest {
            groups.entry(key(&it.natural)).or_default().push(it);
        }
        let (mut single, mut colliding) = (Vec::new(), Vec::new());
        for (k, g) in groups {
            if g.len() == 1 && !st.claimed.contains(&k) { single.push(g) } else { colliding.push(g) }
        }
        // 3. unique names first, so a disambiguated name can never take one that is someone's natural name
        for g in single {
            for it in g {
                let cand = it.natural.clone();
                plan.place(&mut st, it, cand);
            }
        }
        for g in colliding {
            for it in g {
                let cand = it.tagged.clone().unwrap_or_else(|| it.natural.clone());
                plan.place(&mut st, it, cand);
            }
        }
        plan
    }

    fn place(&mut self, st: &mut State, it: Item, cand: PathBuf) {
        let entry = match st.place(cand) {
            Ok((dest, replaces)) => Entry { ext: it.ext, dest: Ok(dest), own: false, replaces },
            Err(reason) => Entry { ext: it.ext, dest: Err(reason), own: false, replaces: false },
        };
        self.entries.insert(it.src, entry);
    }
}

struct Item {
    src: PathBuf,
    ext: String,
    natural: PathBuf,
    /// Name with the source extension kept, for format-changing inputs.
    tagged: Option<PathBuf>,
}

struct State {
    claimed: HashSet<String>,
    inputs: HashSet<String>,
    policy: Overwrite,
}

impl State {
    /// First acceptable name at or after `cand`: not claimed by another job, and not an existing file
    /// unless the policy lets this job take it.
    fn place(&mut self, cand: PathBuf) -> Result<(PathBuf, bool), String> {
        for n in 1u32.. {
            let p = numbered(&cand, n);
            let k = key(&p);
            if self.claimed.contains(&k) {
                continue;
            }
            let is_input = self.inputs.contains(&k);
            if is_input || fs::symlink_metadata(&p).is_ok() {
                match self.policy {
                    Overwrite::Skip => return Err(format!("output exists: {}", p.display())),
                    Overwrite::Rename => continue,
                    Overwrite::Replace if is_input => continue,
                    Overwrite::Replace => {
                        self.claimed.insert(k);
                        return Ok((p, true));
                    }
                }
            }
            self.claimed.insert(k);
            return Ok((p, false));
        }
        unreachable!()
    }
}

fn key(p: &Path) -> String {
    p.to_string_lossy().to_lowercase()
}

fn same_file(a: &Path, b: &Path) -> bool {
    a == b || matches!((fs::canonicalize(a), fs::canonicalize(b)), (Ok(x), Ok(y)) if x == y)
}

/// `name.ext` -> `name-N.ext` (N = 1 is the name itself).
pub fn numbered(p: &Path, n: u32) -> PathBuf {
    if n <= 1 {
        return p.to_path_buf();
    }
    let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("out");
    match p.extension().and_then(|e| e.to_str()) {
        Some(e) => p.with_file_name(format!("{stem}-{n}.{e}")),
        None => p.with_file_name(format!("{stem}-{n}")),
    }
}

/// Does writing `ext` change the file's format (jpg and jpeg are the same format)?
fn changed(src: &Path, ext: &str) -> bool {
    let src_ext = src.extension().and_then(|s| s.to_str()).unwrap_or("");
    !ext.eq_ignore_ascii_case(src_ext) && !(matches!(ext, "jpg" | "jpeg") && matches!(src_ext.to_ascii_lowercase().as_str(), "jpg" | "jpeg"))
}

/// The name an input writes to before any collision handling. `tag` inserts the source extension
/// (`stem[suffix].tag.ext`).
pub fn dest_for(src: &Path, ext: &str, o: &Options, tag: Option<&str>) -> PathBuf {
    let dir = src.parent().unwrap_or(Path::new("."));
    let stem = src.file_stem().and_then(|s| s.to_str()).unwrap_or("out");
    let name = src.file_name().map(|n| n.to_owned()).unwrap_or_default();
    let is_changed = changed(src, ext);
    let file = |stem: String| match tag {
        Some(t) => format!("{stem}.{t}.{ext}"),
        None => format!("{stem}.{ext}"),
    };
    match o.destination.as_str() {
        "in-place" if is_changed => dir.join(file(stem.to_string())),
        "in-place" => src.to_path_buf(),
        "alongside" => dir.join(file(format!("{stem}{}", o.suffix))),
        d if is_changed => PathBuf::from(d).join(file(stem.to_string())),
        d => PathBuf::from(d).join(name),
    }
}

/// Extension the pipeline will write for `src`, from the options alone (plus the frame count for the
/// few animated inputs that `anim_to` re-routes). `None`: not an image this tool handles, no output.
pub fn predict_ext(src: &Path, o: &Options) -> Option<String> {
    let ext = ext_of(src)?;
    if matches!(ext.as_str(), "svg" | "svgz") {
        return Some(ext);
    }
    let s = Fmt::from_ext(&ext)?;
    let gif_still_to = match (s, o.anim_to) {
        (Fmt::Gif, AnimTo::Webp) => Some(Fmt::Webp),
        (Fmt::Gif, AnimTo::Avif) => Some(Fmt::Avif),
        _ => None,
    };
    let mut t = Fmt::from_format(o.format).or(gif_still_to).unwrap_or(s);
    if o.format == Format::Keep && o.anim_to != AnimTo::Keep && matches!(s, Fmt::Png | Fmt::Webp | Fmt::Avif) {
        if let Ok(data) = fs::read(src) {
            if crate::anim::frame_count(&data, s) > 1 {
                t = if o.anim_to == AnimTo::Webp { Fmt::Webp } else { Fmt::Avif };
            }
        }
    }
    Some(if t != s { t.ext().to_string() } else { ext })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn touch(d: &Path, n: &str) -> PathBuf {
        let p = d.join(n);
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&p, b"x").unwrap();
        p
    }
    fn webp_alongside() -> Options {
        Options { format: Format::Webp, destination: "alongside".into(), suffix: "-w".into(), ..Default::default() }
    }
    fn dest(p: &Plan, s: &Path) -> String {
        p.get(s).unwrap().dest.as_ref().map(|d| d.file_name().unwrap().to_string_lossy().into_owned()).unwrap_or_else(|e| format!("SKIP {e}"))
    }

    #[test]
    fn colliders_keep_their_source_extension() {
        let d = tempfile::tempdir().unwrap();
        let files = vec![touch(d.path(), "a.png"), touch(d.path(), "a.avif"), touch(d.path(), "b.png")];
        let p = Plan::build(&files, &webp_alongside());
        assert_eq!(dest(&p, &files[0]), "a-w.png.webp");
        assert_eq!(dest(&p, &files[1]), "a-w.avif.webp");
        assert_eq!(dest(&p, &files[2]), "b-w.webp");
    }

    #[test]
    fn same_format_member_keeps_plain_name() {
        let d = tempfile::tempdir().unwrap();
        let files = vec![touch(d.path(), "a.png"), touch(d.path(), "a.webp")];
        let p = Plan::build(&files, &webp_alongside());
        assert_eq!(dest(&p, &files[0]), "a-w.png.webp");
        assert_eq!(dest(&p, &files[1]), "a-w.webp");
    }

    #[test]
    fn same_extension_from_different_folders_is_numbered_in_path_order() {
        let d = tempfile::tempdir().unwrap();
        let out = d.path().join("out");
        let files = vec![touch(d.path(), "x/IMG.png"), touch(d.path(), "y/IMG.png"), touch(d.path(), "z/IMG.png")];
        let o = Options { destination: out.to_string_lossy().into(), ..Default::default() };
        let p = Plan::build(&files, &o);
        assert_eq!([dest(&p, &files[0]), dest(&p, &files[1]), dest(&p, &files[2])], ["IMG.png", "IMG-2.png", "IMG-3.png"]);
    }

    #[test]
    fn plan_is_independent_of_input_order() {
        let d = tempfile::tempdir().unwrap();
        let a = vec![touch(d.path(), "a.png"), touch(d.path(), "a.avif"), touch(d.path(), "a.jpg")];
        let mut b = a.clone();
        b.reverse();
        let (pa, pb) = (Plan::build(&a, &webp_alongside()), Plan::build(&b, &webp_alongside()));
        for f in &a {
            assert_eq!(dest(&pa, f), dest(&pb, f));
        }
    }

    #[test]
    fn existing_file_follows_policy() {
        let d = tempfile::tempdir().unwrap();
        let src = touch(d.path(), "a.png");
        touch(d.path(), "a-w.webp");
        let mut o = webp_alongside();
        let s = vec![src];
        assert!(dest(&Plan::build(&s, &o), &s[0]).starts_with("SKIP output exists"));
        o.overwrite = Overwrite::Rename;
        assert_eq!(dest(&Plan::build(&s, &o), &s[0]), "a-w-2.webp");
        o.overwrite = Overwrite::Replace;
        let p = Plan::build(&s, &o);
        assert_eq!(dest(&p, &s[0]), "a-w.webp");
        assert!(p.get(&s[0]).unwrap().replaces);
    }

    #[test]
    fn replace_never_takes_another_input() {
        let d = tempfile::tempdir().unwrap();
        // a-w.webp is both an input of the run and the natural name of a.png
        let files = vec![touch(d.path(), "a.png"), touch(d.path(), "a-w.webp")];
        let o = Options { overwrite: Overwrite::Replace, ..webp_alongside() };
        let p = Plan::build(&files, &o);
        assert_eq!(dest(&p, &files[0]), "a-w-2.webp");
        assert!(!p.get(&files[0]).unwrap().replaces);
    }

    #[test]
    fn in_place_same_format_is_anchored() {
        let d = tempfile::tempdir().unwrap();
        // format webp, in place: a.webp stays a.webp, the png must not take that name
        let files = vec![touch(d.path(), "a.png"), touch(d.path(), "a.webp")];
        let o = Options { format: Format::Webp, ..Default::default() };
        let p = Plan::build(&files, &o);
        let e = p.get(&files[1]).unwrap();
        assert!(e.own && e.dest.as_ref().unwrap() == &files[1]);
        assert_eq!(dest(&p, &files[0]), "a.png.webp");
    }

    #[test]
    fn name_comparison_ignores_case() {
        let d = tempfile::tempdir().unwrap();
        let files = vec![touch(d.path(), "a.png"), touch(d.path(), "A.avif")];
        let p = Plan::build(&files, &webp_alongside());
        assert_ne!(dest(&p, &files[0]).to_lowercase(), dest(&p, &files[1]).to_lowercase());
    }
}
