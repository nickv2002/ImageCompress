//! Presets: one `<id>.json` file per preset in the Presets folder, shared by the CLI, the Mac app and the
//! Finder extension. A file holds `{"name", "show_in_finder", "options", ...}`; its file name is the id.
use crate::config;
use crate::options::Options;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::path::{Path, PathBuf};

pub const VERSION: u32 = 1;

pub type Partial = Map<String, Value>;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Preset {
    /// Stable key: the file name without `.json`. Not stored inside the file.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub id: String,
    /// Display name; the id when the file has none.
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub show_in_finder: bool,
    /// Partial `Options` (kebab-case keys); missing keys fall back to defaults.
    #[serde(default)]
    pub options: Partial,
    /// Other keys (the Mac app's `toolbar` tags), kept as they are when the CLI rewrites the file.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// A list of presets (`presets list --json`, `presets export` with no name, and `presets import` of a bundle).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PresetFile {
    pub version: u32,
    pub presets: Vec<Preset>,
}

pub fn slug(name: &str) -> String {
    let s: String = name.chars().map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' }).collect();
    s.split('-').filter(|p| !p.is_empty()).collect::<Vec<_>>().join("-")
}

impl Preset {
    pub fn key(&self) -> String {
        if self.id.is_empty() { slug(&self.name) } else { self.id.clone() }
    }
    pub fn matches(&self, k: &str) -> bool {
        self.id == k || self.name == k || self.name.eq_ignore_ascii_case(k) || slug(&self.name) == slug(k)
    }
}

impl PresetFile {
    /// By id, then name or slug; then the ids of the presets that shipped before the Format × Quality set
    /// (`lossless`, `lossy`, `jpg`, ...), so old scripts keep working.
    pub fn find(&self, k: &str) -> Option<&Preset> {
        self.find_exact(k).or_else(|| {
            let id = legacy_alias(k)?;
            self.presets.iter().find(|p| p.id == id)
        })
    }
    /// By id, then name or slug, without the old-id aliases (for writes).
    pub fn find_exact(&self, k: &str) -> Option<&Preset> {
        self.presets.iter().find(|p| p.id == k).or_else(|| self.presets.iter().find(|p| p.matches(k)))
    }
}

fn obj(v: Value) -> Partial {
    match v {
        Value::Object(m) => m,
        _ => unreachable!(),
    }
}

/// Old built-in ids and the preset that replaces each.
pub fn legacy_alias(k: &str) -> Option<&'static str> {
    Some(match k.to_ascii_lowercase().as_str() {
        "lossless" => "keep-lossless",
        "lossy" => "keep-smaller",
        "jpg" | "jpeg" | "convert-to-jpeg" => "jpeg-visually-lossless",
        "png" | "convert-to-png" => "png-visually-lossless",
        "heif" | "heic" | "convert-to-heic" | "convert-to-heif" => "heic-visually-lossless",
        "avif" | "convert-to-avif" => "avif-visually-lossless",
        "jxl" | "convert-to-jpeg-xl" => "jxl-visually-lossless",
        _ => return None,
    })
}

/// The 21 built-in presets: every output format (or Preserve) × three qualities. They are what the Mac app's
/// Format and Quality toolbar controls run; `toolbar` carries those two tags. None touches originals.
/// Must match `Preset.shippedAxisPresets` in the Mac app (a test compares them).
pub fn builtin() -> PresetFile {
    const FORMATS: [(&str, &str); 7] =
        [("keep", "Preserve"), ("jxl", "JPEG XL"), ("avif", "AVIF"), ("heic", "HEIC"), ("webp", "WebP"), ("jpeg", "JPEG"), ("png", "PNG")];
    const QUALITIES: [(&str, &str); 3] = [("lossless", "Lossless"), ("visually-lossless", "Visually lossless"), ("smaller", "Smaller")];
    const IN_FINDER: [&str; 3] = ["keep-lossless", "keep-smaller", "jxl-visually-lossless"];
    let mut presets = vec![];
    for (f, fname) in FORMATS {
        for (q, qname) in QUALITIES {
            let mut o = obj(json!({"format": f}));
            match q {
                "lossless" => {
                    for k in ["target-quality", "png-quantize", "jpeg-quality", "jxl-quality", "webp-quality"] {
                        o.insert(k.into(), Value::Null);
                    }
                }
                "visually-lossless" => { o.insert("target-quality".into(), json!(88)); o.insert("png-quantize".into(), json!([80, 95])); }
                _ => { o.insert("target-quality".into(), json!(70)); o.insert("png-quantize".into(), json!([70, 90])); }
            }
            // AVIF/HEIC have no exact-lossless switch; HEIC ignores the quality search, so it gets fixed qualities.
            if f == "avif" && q == "lossless" { o.insert("avif-quality".into(), json!(100)); }
            if f == "heic" { o.insert("heic-quality".into(), json!(match q { "lossless" => 100, "visually-lossless" => 90, _ => 50 })); }
            let id = format!("{f}-{q}");
            presets.push(Preset {
                show_in_finder: IN_FINDER.contains(&id.as_str()),
                id,
                name: format!("{fname} · {qname}"),
                options: o,
                extra: obj(json!({"toolbar": {"format": f, "quality": q, "favorite": true}})),
            });
        }
    }
    PresetFile { version: VERSION, presets }
}

/// A preset file that could not be used, and why.
#[derive(Debug, Clone, PartialEq)]
pub struct Problem {
    pub file: PathBuf,
    pub message: String,
}

/// Everything in the Presets folder: the usable presets (in list order) and the files that were skipped.
#[derive(Debug, Clone, PartialEq)]
pub struct Loaded {
    pub set: PresetFile,
    pub problems: Vec<Problem>,
}

/// The presets in the default folder (built-ins when it does not exist yet). Bad files are skipped.
pub fn load() -> PresetFile {
    load_all().set
}

pub fn load_all() -> Loaded {
    load_dir(&config::presets_dir(), &config::order_path())
}

fn is_preset_file(p: &Path) -> bool {
    p.extension().is_some_and(|e| e.eq_ignore_ascii_case("json"))
        && p.file_name().and_then(|n| n.to_str()).is_some_and(|n| !n.starts_with('.'))
}

fn preset_files(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| is_preset_file(p)).collect();
    v.sort();
    v
}

pub fn load_dir(dir: &Path, order: &Path) -> Loaded {
    if !dir.is_dir() {
        return Loaded { set: builtin(), problems: vec![] };
    }
    let mut presets: Vec<Preset> = vec![];
    let mut problems = vec![];
    for path in preset_files(dir) {
        match read_preset(&path) {
            Ok(p) => {
                if let Some(other) = presets.iter().find(|o| o.name.eq_ignore_ascii_case(&p.name) || o.id.eq_ignore_ascii_case(&p.id)) {
                    problems.push(Problem { file: path, message: format!("same name as {}.json", other.id) });
                } else {
                    presets.push(p);
                }
            }
            Err(message) => problems.push(Problem { file: path, message }),
        }
    }
    sort(&mut presets, &read_order(order));
    Loaded { set: PresetFile { version: VERSION, presets }, problems }
}

/// One preset file. Its id is the file name; a missing `name` falls back to it.
pub fn read_preset(path: &Path) -> Result<Preset, String> {
    let s = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let mut p: Preset = serde_json::from_str(&s).map_err(|e| e.to_string())?;
    p.id = path.file_stem().and_then(|s| s.to_str()).unwrap_or_default().to_string();
    if p.name.trim().is_empty() {
        p.name = p.id.clone();
    }
    check_keys(&p.options)?;
    Ok(p)
}

/// The shareable form of a preset: everything but the id.
pub fn file_json(p: &Preset) -> String {
    let mut q = p.clone();
    q.id.clear();
    let mut s = serde_json::to_string_pretty(&q).unwrap_or_default();
    s.push('\n');
    s
}

/// Writes the built-ins when the folder does not exist yet, so a first write never leaves it holding one preset.
pub fn ensure_dir(dir: &Path) -> Result<(), String> {
    if dir.is_dir() {
        return Ok(());
    }
    for p in builtin().presets {
        write_to(dir, &p)?;
    }
    Ok(())
}

fn write_to(dir: &Path, p: &Preset) -> Result<(), String> {
    let path = dir.join(format!("{}.json", p.id));
    config::atomic_write(&path, file_json(p).as_bytes()).map_err(|e| format!("{}: {e}", path.display()))
}

/// Saves one preset as `<id>.json` (an empty id is made from the name, unique in the folder).
pub fn write(dir: &Path, p: &Preset) -> Result<PathBuf, String> {
    ensure_dir(dir)?;
    let mut p = p.clone();
    if p.id.is_empty() {
        p.id = unique_id(dir, &slug(&p.name));
    }
    write_to(dir, &p)?;
    Ok(dir.join(format!("{}.json", p.id)))
}

/// An id not taken by any `.json` file in the folder (readable or not).
pub fn unique_id(dir: &Path, base: &str) -> String {
    let base = if base.is_empty() { "preset" } else { base };
    let taken: Vec<String> = preset_files(dir).iter().filter_map(|p| p.file_stem()?.to_str().map(str::to_ascii_lowercase)).collect();
    let free = |s: &str| !taken.contains(&s.to_ascii_lowercase());
    if free(base) {
        return base.into();
    }
    (2..).map(|n| format!("{base}-{n}")).find(|s| free(s)).unwrap()
}

/// Removes `<id>.json`. False when there is no such file.
pub fn delete(dir: &Path, id: &str) -> Result<bool, String> {
    ensure_dir(dir)?;
    let path = dir.join(format!("{id}.json"));
    match std::fs::remove_file(&path) {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(format!("{}: {e}", path.display())),
    }
}

/// Rewrites the built-ins. With `all`, every other preset file (and the saved order) goes first.
pub fn reset(dir: &Path, order: &Path, all: bool) -> Result<(), String> {
    if all {
        for f in preset_files(dir) {
            std::fs::remove_file(&f).map_err(|e| format!("{}: {e}", f.display()))?;
        }
        let _ = std::fs::remove_file(order);
    }
    for p in builtin().presets {
        write_to(dir, &p)?;
    }
    Ok(())
}

fn read_order(path: &Path) -> Vec<String> {
    std::fs::read_to_string(path).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

/// List order: ids in `preset-order.json` first, then the built-ins in their order, then the rest by name.
pub fn sort(presets: &mut [Preset], order: &[String]) {
    let builtin_ids: Vec<String> = builtin().presets.into_iter().map(|p| p.id).collect();
    presets.sort_by_cached_key(|p| {
        if let Some(i) = order.iter().position(|o| *o == p.id) {
            (0, i, String::new())
        } else if let Some(i) = builtin_ids.iter().position(|b| *b == p.id) {
            (1, i, String::new())
        } else {
            (2, 0, p.name.to_lowercase())
        }
    });
}

/// A bundle (`{"version", "presets": [...]}`), as `presets export` prints with no name.
pub fn parse(s: &str) -> Result<PresetFile, String> {
    let mut f: PresetFile = serde_json::from_str(s).map_err(|e| e.to_string())?;
    if f.version != VERSION {
        return Err(format!("unsupported presets version {} (expected {VERSION})", f.version));
    }
    for p in &mut f.presets {
        if p.name.trim().is_empty() {
            p.name = p.id.clone();
        }
        check_keys(&p.options).map_err(|e| format!("preset '{}': {e}", p.name))?;
    }
    Ok(f)
}

/// Reject keys that are not `Options` fields (catches typos).
pub fn check_keys(p: &Partial) -> Result<(), String> {
    let known = match serde_json::to_value(Options::default()) {
        Ok(Value::Object(m)) => m,
        _ => unreachable!(),
    };
    for k in p.keys() {
        if !known.contains_key(k) {
            return Err(format!("unknown option '{k}'"));
        }
    }
    Ok(())
}

/// Merge `over` onto `base` key by key (a `null` in `over` sets the field to null, i.e. "lossless"/"off").
pub fn merge(base: &mut Partial, over: &Partial) {
    for (k, v) in over {
        base.insert(k.clone(), v.clone());
    }
}

/// Defaults with `layers` applied in order.
pub fn resolve(layers: &[&Partial]) -> Result<Options, String> {
    let mut m = match serde_json::to_value(Options::default()).map_err(|e| e.to_string())? {
        Value::Object(m) => m,
        _ => unreachable!(),
    };
    for l in layers {
        merge(&mut m, l);
    }
    let mut o: Options = serde_json::from_value(Value::Object(m)).map_err(|e| e.to_string())?;
    // A null jpeg-quality in a layer is a request for lossless; the default null is not (see convert::encode_fixed).
    o.jpeg_lossless_requested = layers.iter().rev().find_map(|l| l.get("jpeg-quality")).is_some_and(Value::is_null);
    Ok(o)
}

/// Only the fields of `o` that differ from `Options::default()`.
pub fn diff_from_default(o: &Options) -> Partial {
    let (Ok(Value::Object(d)), Ok(Value::Object(v))) = (serde_json::to_value(Options::default()), serde_json::to_value(o)) else { unreachable!() };
    v.into_iter().filter(|(k, x)| d.get(k) != Some(x)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtins_resolve() {
        let b = builtin();
        assert_eq!(b.presets.len(), 21);
        for p in &b.presets {
            let o = resolve(&[&p.options]).unwrap_or_else(|e| panic!("{}: {e}", p.name));
            assert_eq!(o.originals, crate::options::Originals::Keep, "{}", p.name);
        }
        let o = resolve(&[&b.find("jpg").unwrap().options]).unwrap();   // old id still resolves
        assert_eq!(o.format, crate::options::Format::Jpeg);
        assert_eq!(b.find("lossy").unwrap().id, "keep-smaller");
        assert_eq!(b.presets.iter().filter(|p| p.show_in_finder).count(), 3);
    }

    fn mine(name: &str) -> Preset {
        Preset { id: String::new(), name: name.into(), show_in_finder: true, options: obj(json!({"jpeg-quality": null, "jobs": 2})), extra: obj(json!({"toolbar": {"favorite": false}})) }
    }

    #[test]
    fn folder_roundtrip() {
        let d = tempfile::tempdir().unwrap();
        let (dir, order) = (d.path().join("Presets"), d.path().join("preset-order.json"));
        assert_eq!(load_dir(&dir, &order).set, builtin());   // no folder yet: built-ins, nothing written
        assert!(!dir.exists());
        let path = write(&dir, &mine("My Preset")).unwrap();   // the first write puts the built-ins beside it
        assert_eq!(path.file_name().unwrap(), "my-preset.json");
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(!text.contains("\"id\""), "{text}");
        let g = load_dir(&dir, &order);
        assert_eq!(g.set.presets.len(), 22);
        assert_eq!(g.set.presets.last().unwrap().id, "my-preset");
        assert_eq!(g.set.find("My Preset").unwrap().options["jobs"], json!(2));
        assert_eq!(g.set.find("my-preset").unwrap().extra["toolbar"], json!({"favorite": false}));   // unknown keys survive
        // same name again gets its own file; temp files and dotfiles are ignored
        assert_eq!(unique_id(&dir, "my-preset"), "my-preset-2");
        std::fs::write(dir.join(".tmp-abc"), "{").unwrap();
        std::fs::write(dir.join(".hidden.json"), "{").unwrap();
        assert!(load_dir(&dir, &order).problems.is_empty());
        // a preset really named "lossy" wins over the alias
        write(&dir, &Preset { id: "lossy".into(), name: "Lossy".into(), show_in_finder: false, options: Partial::new(), extra: Map::new() }).unwrap();
        assert_eq!(load_dir(&dir, &order).set.find("lossy").unwrap().id, "lossy");
        assert!(delete(&dir, "lossy").unwrap());
        assert!(!delete(&dir, "lossy").unwrap());
        reset(&dir, &order, true).unwrap();
        assert_eq!(load_dir(&dir, &order).set, builtin());
    }

    #[test]
    fn bad_files_are_skipped() {
        let d = tempfile::tempdir().unwrap();
        let (dir, order) = (d.path().join("Presets"), d.path().join("preset-order.json"));
        ensure_dir(&dir).unwrap();
        std::fs::write(dir.join("broken.json"), r#"{"name": "A" "options": {}}"#).unwrap();
        std::fs::write(dir.join("typo.json"), r#"{"name": "T", "options": {"jpeg-qualty": 1}}"#).unwrap();
        std::fs::write(dir.join("twin.json"), r#"{"name": "preserve · lossless"}"#).unwrap();
        std::fs::write(dir.join("Hand Made.json"), r#"{"options": {"jobs": 1}}"#).unwrap();
        let g = load_dir(&dir, &order);
        assert_eq!(g.set.presets.len(), 22);
        assert_eq!(g.set.find("Hand Made").unwrap().id, "Hand Made");   // name falls back to the file name
        let mut bad: Vec<_> = g.problems.iter().map(|p| p.file.file_name().unwrap().to_str().unwrap().to_string()).collect();
        bad.sort();
        assert_eq!(bad, ["broken.json", "twin.json", "typo.json"]);
        assert_eq!(unique_id(&dir, "broken"), "broken-2");   // never overwrites a file it could not read
        // saved order first, then built-ins, then the rest by name
        std::fs::write(&order, r#"["Hand Made", "jxl-smaller"]"#).unwrap();
        let ids: Vec<_> = load_dir(&dir, &order).set.presets.into_iter().map(|p| p.id).collect();
        assert_eq!(&ids[..3], ["Hand Made", "jxl-smaller", "keep-lossless"]);
    }

    #[test]
    fn null_overrides_and_unknown_keys() {
        let a = obj(json!({"jpeg-quality": 70}));
        let b = obj(json!({"jpeg-quality": null}));
        assert_eq!(resolve(&[&a, &b]).unwrap().jpeg_quality, None);
        assert!(check_keys(&obj(json!({"jpeg-qualty": 1}))).is_err());
        assert!(parse(r#"{"version":2,"presets":[]}"#).is_err());
        assert!(parse(r#"{"version":1,"presets":[{"name":"x","options":{"nope":1}}]}"#).is_err());
    }

    #[test]
    fn diff() {
        let mut o = Options::default();
        o.jobs = 3;
        assert_eq!(diff_from_default(&o), obj(json!({"jobs": 3})));
    }
}
