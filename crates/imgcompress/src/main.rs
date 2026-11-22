mod extra_args;
mod flags;
mod history;
mod plan;
mod tools;

use clap::{Args, CommandFactory, Parser, Subcommand};
use compress_core::presets::{self, Partial, Preset};
use compress_core::{config, discover::discover, options::Options, JobEvent};
use flags::OptFlags;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

const BUNDLE_ID: &str = "com.nickvance.imagecompress";

/// Exit codes: 0 ok, 1 some files failed, 2 usage error, 3 missing encoder or helper.
#[derive(Debug)]
enum Fail {
    Failed(String),
    Usage(String),
    Missing(String),
}
type R = Result<i32, Fail>;

fn usage<E: ToString>(e: E) -> Fail { Fail::Usage(e.to_string()) }
fn failed<E: ToString>(e: E) -> Fail { Fail::Failed(e.to_string()) }

#[derive(Parser)]
#[command(name = "imgcompress", version, about = "Compress and convert images", arg_required_else_help = true)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Headless processing
    Run(RunArgs),
    /// Hand paths to the Mac app (macOS only)
    Open(OpenArgs),
    /// Revert one finished job from its `undo` record (a JSON file)
    Undo { record: PathBuf },
    /// Encode one file with the effective options and report the size
    Preview(PreviewArgs),
    /// Manage presets (one JSON file each in the Presets folder)
    Presets { #[command(subcommand)] sub: PresetCmd },
    /// Options model
    Options { #[command(subcommand)] sub: OptCmd },
    /// List linked encoders and helpers
    Tools {
        /// Exit 3 if a required encoder or helper is missing
        #[arg(long)]
        check: bool,
        #[arg(long)]
        json: bool,
    },
    /// Processing history
    History { #[command(subcommand)] sub: HistoryCmd },
    /// Print license notices
    Licenses,
    /// Print shell completions
    Completions { shell: clap_complete::Shell },
    /// Print the man page
    Man,
    /// Symlink this executable into a directory on PATH
    Install {
        #[arg(long, default_value = "/usr/local/bin")]
        dir: PathBuf,
    },
    /// Remove the symlink made by `install`
    Uninstall {
        #[arg(long, default_value = "/usr/local/bin")]
        dir: PathBuf,
    },
}

#[derive(Subcommand)]
enum OptCmd {
    /// Print the JSON Schema of Options
    Schema,
}

#[derive(Subcommand)]
enum PresetCmd {
    List { #[arg(long)] json: bool },
    /// Print a preset with all options expanded
    Show { name: String },
    /// Print one preset as a shareable file, or all of them as one bundle
    Export { name: Option<String> },
    /// Copy preset files (or an exported bundle) into the Presets folder
    Import {
        #[arg(required = true)]
        files: Vec<PathBuf>,
        /// Replace a preset with the same name
        #[arg(long)]
        force: bool,
    },
    Delete { name: String },
    /// Restore the built-in presets (keeps custom ones unless --all)
    Reset { #[arg(long)] all: bool },
    /// Print the Presets folder
    Path,
}

#[derive(Subcommand)]
enum HistoryCmd {
    Stats,
    Clear,
}

/// Preset / options-file / flag layering, shared by run, open and preview.
#[derive(Args)]
struct Layer {
    /// Start from a preset: its id or name, or the path of a preset file
    #[arg(long, value_name = "NAME|FILE")]
    preset: Option<String>,
    /// JSON file with (partial) options, applied after --preset
    #[arg(long, value_name = "FILE")]
    options_file: Option<PathBuf>,
    /// Save the effective options as a preset
    #[arg(long, value_name = "NAME")]
    save_preset: Option<String>,
    #[command(flatten)]
    flags: OptFlags,
}

#[derive(Args)]
struct RunArgs {
    paths: Vec<PathBuf>,
    #[command(flatten)]
    layer: Layer,
    /// NDJSON events on stdout
    #[arg(long)]
    json: bool,
    #[arg(short, long)]
    quiet: bool,
    #[arg(short, long)]
    verbose: bool,
    /// Keep a backup of every file this run replaces or removes in DIR, and put an `undo` record in each finished event
    #[arg(long, value_name = "DIR")]
    backup_dir: Option<PathBuf>,
}

#[derive(Args)]
struct OpenArgs {
    paths: Vec<PathBuf>,
    /// Start processing immediately
    #[arg(long)]
    start: bool,
    #[command(flatten)]
    layer: Layer,
}

#[derive(Args)]
struct PreviewArgs {
    file: PathBuf,
    #[arg(long, value_name = "PATH")]
    out: PathBuf,
    #[command(flatten)]
    layer: Layer,
}

struct Resolved {
    options: Options,
    /// options-file + flags, for `open`
    overrides: Partial,
}

fn read_options_file(f: &Path) -> Result<Partial, Fail> {
    let s = std::fs::read_to_string(f).map_err(|e| usage(format!("{}: {e}", f.display())))?;
    match serde_json::from_str(&s) {
        Ok(Value::Object(m)) => {
            presets::check_keys(&m).map_err(|e| usage(format!("{}: {e}", f.display())))?;
            Ok(m)
        }
        Ok(_) => Err(usage(format!("{}: expected a JSON object", f.display()))),
        Err(e) => Err(usage(format!("{}: {e}", f.display()))),
    }
}

/// defaults -> --preset -> --options-file -> flags
fn resolve(l: &Layer) -> Result<Resolved, Fail> {
    let preset = match &l.preset {
        Some(n) => Some(find_preset(n)?.options),
        None => None,
    };
    let file = l.options_file.as_deref().map(read_options_file).transpose()?.unwrap_or_default();
    let empty = Partial::new();
    let base = presets::resolve(&[preset.as_ref().unwrap_or(&empty), &file]).map_err(usage)?;
    let fmt = serde_json::to_value(base.format).ok().and_then(|v| v.as_str().map(String::from)).unwrap_or_else(|| "keep".into());
    let flag_ov = l.flags.overrides(&fmt).map_err(usage)?;
    let options = presets::resolve(&[preset.as_ref().unwrap_or(&empty), &file, &flag_ov]).map_err(usage)?;
    let mut overrides = file;
    presets::merge(&mut overrides, &flag_ov);
    Ok(Resolved { options, overrides })
}

/// A preset by id or name, or a preset file given by path. Names a skipped file's problem when that is the one asked for.
fn find_preset(n: &str) -> Result<Preset, Fail> {
    let path = Path::new(n);
    if n.ends_with(".json") && path.is_file() {
        return presets::read_preset(path).map_err(|e| usage(format!("{n}: {e}")));
    }
    let l = presets::load_all();
    if let Some(p) = l.set.find(n) {
        return Ok(p.clone());
    }
    if let Some(bad) = l.problems.iter().find(|b| b.file.file_stem().and_then(|s| s.to_str()).is_some_and(|s| s == n || presets::slug(s) == presets::slug(n))) {
        return Err(usage(format!("{}: {}", bad.file.display(), bad.message)));
    }
    Err(usage(format!("no preset named '{n}' (see `imgcompress presets list`)")))
}

fn save_preset(name: &str, o: &Options) -> Result<(), Fail> {
    let existing = presets::load().find_exact(name).cloned();
    // run-only fields are not part of a preset
    let mut options = presets::diff_from_default(o);
    for k in ["dry-run"] {
        options.remove(k);
    }
    let p = match existing {
        Some(p) => Preset { options, ..p },
        None => Preset { id: String::new(), name: name.into(), show_in_finder: false, options, extra: Default::default() },
    };
    let path = presets::write(&config::presets_dir(), &p).map_err(failed)?;
    eprintln!("saved preset '{name}' to {}", path.display());
    Ok(())
}

fn print_json<T: serde::Serialize>(v: &T) {
    println!("{}", serde_json::to_string_pretty(v).unwrap());
}

// ---------------------------------------------------------------- run

fn cmd_run(a: RunArgs) -> R {
    let r = resolve(&a.layer)?;
    let mut o = r.options;
    o.backup_dir = a.backup_dir.clone();
    if let Some(n) = &a.layer.save_preset {
        save_preset(n, &o)?;
        if a.paths.is_empty() {
            return Ok(0);
        }
    }
    if a.paths.is_empty() {
        return Err(usage("no paths given"));
    }
    for p in &a.paths {
        if !p.exists() {
            return Err(failed(format!("{}: no such file or directory", p.display())));
        }
    }
    let files = discover(&a.paths, &o);
    let hist = if o.history && !o.dry_run {
        match history::History::open(&config::history_path(), &o) {
            Ok(h) => Some(Arc::new(h)),
            Err(e) => {
                if a.verbose { eprintln!("warning: {e}; continuing without history") }
                None
            }
        }
    } else {
        None
    };
    let (json, quiet, verbose) = (a.json, a.quiet, a.verbose);
    let hist_skipped = std::sync::atomic::AtomicUsize::new(0);
    let missing = AtomicBool::new(false);
    let emit = |ev: JobEvent| {
        match &ev {
            JobEvent::Failed { error, .. } if error.contains("not found") => missing.store(true, Ordering::Relaxed),
            JobEvent::Finished { output, .. } => if let Some(h) = &hist { h.record(output) },
            JobEvent::Skipped { path, reason } if reason == "not smaller" => if let Some(h) = &hist { h.record(path) },
            _ => {}
        }
        // history skips are reported before the engine runs; fold them into the totals
        let ev = match ev {
            JobEvent::Totals { processed, skipped, failed, in_bytes, out_bytes } => JobEvent::Totals { processed, skipped: skipped + hist_skipped.load(Ordering::Relaxed), failed, in_bytes, out_bytes },
            e => e,
        };
        print_event(&ev, json, quiet, verbose);
    };
    let mut todo = Vec::new();
    for f in files {
        if hist.as_ref().is_some_and(|h| h.is_done(&f)) {
            hist_skipped.fetch_add(1, Ordering::Relaxed);
            if o.dry_run { continue }
            print_event(&JobEvent::Skipped { path: f, reason: "unchanged since last run (history)".into() }, json, quiet, verbose);
        } else {
            todo.push(f);
        }
    }
    if o.dry_run {
        plan::print(&todo, &o, json);
        return Ok(0);
    }
    let cancel = Arc::new(AtomicBool::new(false));
    {
        let c = cancel.clone();
        let _ = ctrlc::set_handler(move || c.store(true, Ordering::Relaxed));
    }
    let out = compress_core::run(&todo, &o, &cancel, &emit);
    Ok(if missing.load(Ordering::Relaxed) { 3 } else if cancel.load(Ordering::Relaxed) { 130 } else if out.failed > 0 { 1 } else { 0 })
}

fn print_event(ev: &JobEvent, json: bool, quiet: bool, verbose: bool) {
    if json {
        println!("{}", serde_json::to_string(ev).unwrap());
        return;
    }
    match ev {
        JobEvent::Failed { path, error } => eprintln!("{}: FAILED {error}", path.display()),
        _ if quiet => {}
        JobEvent::Finished { path, output, in_bytes, out_bytes, encoder, trace, .. } => {
            let pct = 100.0 * (1.0 - *out_bytes as f64 / (*in_bytes).max(1) as f64);
            let dst = if output != path { format!(" => {}", output.display()) } else { String::new() };
            eprintln!("{}{dst}: {in_bytes} -> {out_bytes} ({pct:.1}% smaller, {encoder})", path.display());
            if verbose {
                for (s, b) in trace { eprintln!("    {s}: {b}") }
            }
        }
        JobEvent::Skipped { path, reason } => eprintln!("{}: skipped ({reason})", path.display()),
        JobEvent::Stage { path, stage } if verbose => eprintln!("{}: {stage}", path.display()),
        JobEvent::Totals { processed, skipped, failed, in_bytes, out_bytes } => {
            let pct = if *in_bytes > 0 { 100.0 * (1.0 - *out_bytes as f64 / *in_bytes as f64) } else { 0.0 };
            eprintln!("{processed} compressed, {skipped} skipped, {failed} failed; {in_bytes} -> {out_bytes} bytes ({pct:.1}% saved)");
        }
        _ => {}
    }
}

// ---------------------------------------------------------------- preview

fn cmd_preview(a: PreviewArgs) -> R {
    let mut o = resolve(&a.layer)?.options;
    if !a.file.is_file() {
        return Err(failed(format!("{}: not a file", a.file.display())));
    }
    o.history = false;
    o.dry_run = false;
    o.min_size = 0;
    o.jobs = 1;
    o.destination = "in-place".into();
    o.originals = compress_core::options::Originals::Keep;
    o.replace_if = compress_core::options::ReplaceIf::Always;
    let tmp = tempfile::tempdir().map_err(failed)?;
    let src = tmp.path().join(a.file.file_name().ok_or_else(|| usage("bad file name"))?);
    std::fs::copy(&a.file, &src).map_err(failed)?;
    let result: Mutex<Option<Result<(PathBuf, u64, u64, String), String>>> = Mutex::new(None);
    let emit = |ev: JobEvent| match ev {
        JobEvent::Finished { output, in_bytes, out_bytes, encoder, .. } => *result.lock().unwrap() = Some(Ok((output, in_bytes, out_bytes, encoder))),
        JobEvent::Skipped { reason, .. } => *result.lock().unwrap() = Some(Err(reason)),
        JobEvent::Failed { error, .. } => *result.lock().unwrap() = Some(Err(error)),
        _ => {}
    };
    compress_core::run(&[src], &o, &AtomicBool::new(false), &emit);
    match result.into_inner().unwrap() {
        Some(Ok((output, in_bytes, out_bytes, encoder))) => {
            if let Some(d) = a.out.parent().filter(|d| !d.as_os_str().is_empty()) {
                std::fs::create_dir_all(d).map_err(failed)?;
            }
            std::fs::copy(&output, &a.out).map_err(failed)?;
            println!("{}", json!({"input": a.file, "output": a.out, "in_bytes": in_bytes, "out_bytes": out_bytes, "encoder": encoder, "savings_pct": 100.0 * (1.0 - out_bytes as f64 / in_bytes.max(1) as f64)}));
            Ok(0)
        }
        Some(Err(e)) if e.contains("not found") => Err(Fail::Missing(e)),
        Some(Err(e)) => Err(failed(e)),
        None => Err(failed("nothing was produced")),
    }
}

// ---------------------------------------------------------------- open

fn cmd_open(a: OpenArgs) -> R {
    if !cfg!(target_os = "macos") {
        return Err(usage("`open` is only available on macOS"));
    }
    let r = resolve(&a.layer)?;
    if let Some(n) = &a.layer.save_preset {
        save_preset(n, &r.options)?;
    }
    let mut cmd = std::process::Command::new("/usr/bin/open");
    cmd.args(["-b", BUNDLE_ID]);
    if !a.paths.is_empty() {
        let paths: Vec<PathBuf> = a.paths.iter().map(|p| std::fs::canonicalize(p).map_err(|e| failed(format!("{}: {e}", p.display())))).collect::<Result<_, _>>()?;
        let id = uuid::Uuid::new_v4().to_string();
        let job = json!({"preset": a.layer.preset, "overrides": r.overrides, "paths": paths, "start": a.start});
        let dir = config::jobs_dir();
        config::atomic_write(&dir.join(format!("{id}.json")), serde_json::to_string_pretty(&job).unwrap().as_bytes()).map_err(failed)?;
        cmd.arg(format!("imagecompress://job/{id}"));
    }
    let st = cmd.status().map_err(failed)?;
    Ok(if st.success() { 0 } else { 1 })
}

// ---------------------------------------------------------------- presets

fn report_problems(l: &presets::Loaded) {
    for b in &l.problems {
        eprintln!("skipped {}: {}", b.file.display(), b.message);
    }
}

fn cmd_presets(sub: PresetCmd) -> R {
    let dir = config::presets_dir();
    match sub {
        PresetCmd::List { json } => {
            let l = presets::load_all();
            report_problems(&l);
            if json {
                print_json(&l.set);
            } else {
                for p in &l.set.presets {
                    println!("{:<24} {}{}", p.id, p.name, if p.show_in_finder { "  [finder]" } else { "" });
                }
            }
        }
        PresetCmd::Show { name } => {
            let p = find_preset(&name)?;
            let o = presets::resolve(&[&p.options]).map_err(failed)?;
            print_json(&json!({"id": p.id, "name": p.name, "show_in_finder": p.show_in_finder, "options": o}));
        }
        PresetCmd::Export { name } => match name {
            Some(n) => print!("{}", presets::file_json(&find_preset(&n)?)),
            None => print_json(&presets::load()),
        },
        PresetCmd::Import { files, force } => {
            let mut incoming = vec![];
            for file in &files {
                let s = std::fs::read_to_string(file).map_err(|e| usage(format!("{}: {e}", file.display())))?;
                if serde_json::from_str::<Value>(&s).ok().and_then(|v| v.get("presets").cloned()).is_some() {
                    incoming.extend(presets::parse(&s).map_err(|e| usage(format!("{}: {e}", file.display())))?.presets);
                } else {
                    let mut p = presets::read_preset(file).map_err(|e| usage(format!("{}: {e}", file.display())))?;
                    p.id = String::new();   // named after the preset, not the downloaded file
                    incoming.push(p);
                }
            }
            let have = presets::load();
            for mut p in incoming {
                match have.find_exact(&p.name) {
                    Some(old) if !force => return Err(usage(format!("a preset named '{}' already exists ({}.json); use --force to replace it", p.name, old.id))),
                    Some(old) => p.id = old.id.clone(),
                    None => p.id = presets::unique_id(&dir, &if p.id.is_empty() { presets::slug(&p.name) } else { p.id.clone() }),
                }
                let path = presets::write(&dir, &p).map_err(failed)?;
                eprintln!("imported '{}' as {}", p.name, path.display());
            }
        }
        PresetCmd::Delete { name } => {
            let p = find_preset(&name)?;
            presets::delete(&dir, &p.id).map_err(failed)?;
            eprintln!("deleted {}", dir.join(format!("{}.json", p.id)).display());
        }
        PresetCmd::Reset { all } => {
            presets::reset(&dir, &config::order_path(), all).map_err(failed)?;
            eprintln!("restored built-in presets in {}", dir.display());
        }
        PresetCmd::Path => println!("{}", dir.display()),
    }
    Ok(0)
}

// ---------------------------------------------------------------- misc

fn cmd_tools(check: bool, json: bool) -> R {
    let t = tools::gather();
    if json {
        print_json(&t.iter().map(|t| json!({"name": t.name, "version": t.version, "source": t.source, "ok": t.ok})).collect::<Vec<_>>());
    } else {
        for t in &t {
            println!("{:<11} {:<24} {}{}", t.name, t.version, t.source, if t.ok { "" } else { "  MISSING" });
        }
    }
    if check && t.iter().any(|t| !t.ok) {
        eprintln!("error: missing: {}", t.iter().filter(|t| !t.ok).map(|t| t.name).collect::<Vec<_>>().join(", "));
        return Ok(3);
    }
    Ok(0)
}

fn cmd_licenses() -> R {
    println!("imgcompress {} is distributed under the GNU GPL v3 or later (it links libimagequant, GPLv3).", env!("CARGO_PKG_VERSION"));
    println!("The repository's own source is CC0; binaries are GPLv3. gifsicle (GPLv2) runs as a separate helper process.");
    let exe = std::env::current_exe().and_then(|p| p.canonicalize()).ok();
    let dir = exe.as_deref().and_then(Path::parent);
    // App bundle, tarball / Homebrew libexec, /usr/lib/imgcompress (deb/rpm), source tree.
    let cands = dir
        .map(|d| vec![d.join("../Resources/licenses"), d.join("licenses"), d.join("../share/doc/imgcompress/licenses"), d.join("../../share/doc/imgcompress/licenses"), d.join("../../licenses")])
        .unwrap_or_default();
    match cands.iter().find(|p| p.is_dir()) {
        Some(p) => println!("Third-party license texts: {}", p.canonicalize().unwrap_or(p.clone()).display()),
        None => println!("Third-party license texts: not found next to this binary (see the licenses/ folder of the source tree)."),
    }
    Ok(0)
}

fn cmd_history(sub: HistoryCmd) -> R {
    let p = config::history_path();
    match sub {
        HistoryCmd::Stats => {
            let s = history::stats(&p).map_err(failed)?;
            println!("{} files, {} bytes tracked ({})", s.files, s.bytes, p.display());
        }
        HistoryCmd::Clear => {
            let n = history::clear(&p).map_err(failed)?;
            eprintln!("cleared {n} entries");
        }
    }
    Ok(0)
}

fn dispatch(cli: Cli) -> R {
    match cli.cmd {
        Cmd::Run(a) => cmd_run(a),
        Cmd::Open(a) => cmd_open(a),
        Cmd::Preview(a) => cmd_preview(a),
        Cmd::Undo { record } => {
            let u: compress_core::undo::Undo = serde_json::from_str(&std::fs::read_to_string(&record).map_err(failed)?).map_err(failed)?;
            compress_core::undo::undo(&u).map_err(failed)?;
            Ok(0)
        }
        Cmd::Presets { sub } => cmd_presets(sub),
        Cmd::Options { sub: OptCmd::Schema } => {
            print_json(&schemars::schema_for!(Options));
            Ok(0)
        }
        Cmd::Tools { check, json } => cmd_tools(check, json),
        Cmd::History { sub } => cmd_history(sub),
        Cmd::Licenses => cmd_licenses(),
        Cmd::Completions { shell } => {
            clap_complete::generate(shell, &mut Cli::command(), "imgcompress", &mut std::io::stdout());
            Ok(0)
        }
        Cmd::Install { dir } => {
            let exe = std::env::current_exe().and_then(|p| p.canonicalize()).map_err(failed)?;
            let dest = dir.join("imgcompress");
            std::fs::create_dir_all(&dir).map_err(failed)?;
            let _ = std::fs::remove_file(&dest);
            #[cfg(unix)]
            std::os::unix::fs::symlink(&exe, &dest).map_err(|e| failed(format!("{}: {e} (try --dir ~/.local/bin or sudo)", dest.display())))?;
            println!("Linked {} to {}", dest.display(), exe.display());
            Ok(0)
        }
        Cmd::Uninstall { dir } => {
            let dest = dir.join("imgcompress");
            match std::fs::symlink_metadata(&dest) {
                Ok(m) if m.file_type().is_symlink() => {
                    std::fs::remove_file(&dest).map_err(failed)?;
                    println!("Removed {}", dest.display());
                }
                _ => println!("Nothing to remove at {}", dest.display()),
            }
            Ok(0)
        }
        Cmd::Man => {
            clap_mangen::Man::new(Cli::command()).render(&mut std::io::stdout()).map_err(failed)?;
            Ok(0)
        }
    }
}

fn main() {
    // die quietly on a closed pipe (`imgcompress ... | head`) instead of panicking in println!
    #[cfg(unix)]
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }
    let code = match dispatch(Cli::parse()) {
        Ok(c) => c,
        Err(Fail::Usage(m)) => { eprintln!("error: {m}"); 2 }
        Err(Fail::Failed(m)) => { eprintln!("error: {m}"); 1 }
        Err(Fail::Missing(m)) => { eprintln!("error: {m}"); 3 }
    };
    std::process::exit(code);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn flags_of(cmd: &clap::Command) -> HashSet<String> {
        cmd.get_arguments().filter_map(|a| a.get_long().map(String::from)).collect()
    }

    /// Every `Options` field must be reachable from the CLI (`--field`, or `--field`/`--no-field` for booleans),
    /// in every command that takes options.
    #[test]
    fn every_option_has_a_flag() {
        let schema = serde_json::to_value(schemars::schema_for!(Options)).unwrap();
        let props: Vec<String> = schema["properties"].as_object().expect("schema properties").keys().cloned().collect();
        assert!(props.len() > 30);
        let root = Cli::command();
        for sub in ["run", "open", "preview"] {
            let flags = flags_of(root.find_subcommand(sub).unwrap());
            for p in &props {
                let ok = flags.contains(p) || flags.contains(&format!("no-{p}"));
                assert!(ok, "Options field `{p}` has no `--{p}` flag on `{sub}`; add it to crates/imgcompress/src/flags.rs");
                if schema["properties"][p]["type"] == "boolean" {
                    let bare = flags.contains(p);
                    let neg = flags.contains(&format!("no-{p}"));
                    assert!(bare || neg);
                }
            }
        }
    }

    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn layering_order() {
        let cli = Cli::try_parse_from(["imgcompress", "run", "--options-file", "/nonexistent", "x"]).unwrap();
        let Cmd::Run(a) = cli.cmd else { panic!() };
        assert!(resolve(&a.layer).is_err());
        let d = tempfile::tempdir().unwrap();
        let f = d.path().join("o.json");
        std::fs::write(&f, r#"{"jpeg-quality": 60, "jobs": 3}"#).unwrap();
        std::env::set_var(config::ENV_CONFIG, d.path());
        let cli = Cli::try_parse_from(["imgcompress", "run", "--preset", "jpg", "--options-file", f.to_str().unwrap(), "--jobs", "5", "--jpeg-quality", "lossless", "x"]).unwrap();
        let Cmd::Run(a) = cli.cmd else { panic!() };
        let o = resolve(&a.layer).unwrap().options;
        assert_eq!(o.format, compress_core::options::Format::Jpeg); // preset
        assert_eq!(o.jpeg_quality, None); // flag beats options-file
        assert_eq!(o.jobs, 5); // flag beats options-file
        assert_eq!(o.originals, compress_core::options::Originals::Keep); // "jpg" is now JPEG · Visually lossless, which keeps originals
        assert_eq!(o.target_quality, Some(88.0));
    }
}
