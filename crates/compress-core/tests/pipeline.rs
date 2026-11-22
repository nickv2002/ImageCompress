use compress_core::{options::*, run};
use std::sync::atomic::AtomicBool;
use std::sync::Mutex;

fn png_bytes(w: u32, h: u32) -> Vec<u8> {
    let img = image::RgbaImage::from_fn(w, h, |x, y| image::Rgba([(x * 7) as u8, (y * 5) as u8, ((x ^ y) * 3) as u8, 255]));
    let mut out = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png).unwrap();
    out
}

fn go(dir: &std::path::Path, o: &Options) -> (compress_core::pipeline::Outcome, Vec<compress_core::JobEvent>) {
    let ev = Mutex::new(vec![]);
    let out = run(&[dir.to_path_buf()], o, &AtomicBool::new(false), &|e| ev.lock().unwrap().push(e));
    (out, ev.into_inner().unwrap())
}

#[test]
fn same_names_in_different_dirs_do_not_collide() {
    let d = tempfile::tempdir().unwrap();
    for sub in ["a", "b"] {
        std::fs::create_dir(d.path().join(sub)).unwrap();
        std::fs::write(d.path().join(sub).join("IMG_0001.png"), png_bytes(64 + if sub == "a" { 0 } else { 8 }, 64)).unwrap();
    }
    let o = Options { destination: "alongside".into(), ..Default::default() };
    let (out, _) = go(d.path(), &o);
    assert_eq!(out.processed, 2);
    assert!(d.path().join("a/IMG_0001-min.png").exists() && d.path().join("b/IMG_0001-min.png").exists());
}

#[test]
fn corrupt_file_fails_without_touching_original() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("bad.png");
    std::fs::write(&p, b"\x89PNG\r\n\x1a\nnot really").unwrap();
    let (out, _) = go(d.path(), &Options::default());
    assert_eq!((out.failed, out.processed), (1, 0));
    assert_eq!(std::fs::read(&p).unwrap(), b"\x89PNG\r\n\x1a\nnot really");
}

#[test]
fn hidden_and_empty_files_skipped() {
    let d = tempfile::tempdir().unwrap();
    std::fs::write(d.path().join(".h.png"), png_bytes(32, 32)).unwrap();
    std::fs::write(d.path().join("z.png"), b"").unwrap();
    let (out, _) = go(d.path(), &Options::default());
    assert_eq!((out.processed, out.failed, out.skipped), (0, 0, 1));
}

#[test]
fn never_grows_file() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("x.png");
    std::fs::write(&p, png_bytes(48, 48)).unwrap();
    let before = std::fs::metadata(&p).unwrap().len();
    go(d.path(), &Options::default());
    assert!(std::fs::metadata(&p).unwrap().len() <= before);
}

#[cfg(unix)]
#[test]
fn output_keeps_source_permissions() {
    use std::os::unix::fs::PermissionsExt;
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("a.png");
    std::fs::write(&p, png_bytes(64, 64)).unwrap();
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o644)).unwrap();
    let (out, _) = go(d.path(), &Options::default());
    assert_eq!(out.processed, 1);
    assert_eq!(std::fs::metadata(&p).unwrap().permissions().mode() & 0o777, 0o644);
}

fn undo_run(o: &Options, dir: &std::path::Path) -> Vec<compress_core::undo::Undo> {
    let (_, ev) = go(dir, o);
    ev.into_iter().filter_map(|e| match e { compress_core::JobEvent::Finished { undo, .. } => undo, _ => None }).collect()
}

#[test]
fn undo_restores_an_in_place_file_and_refuses_after_edits() {
    let d = tempfile::tempdir().unwrap();
    let bk = tempfile::tempdir().unwrap();
    let p = d.path().join("a.png");
    let orig = png_bytes(96, 96);
    std::fs::write(&p, &orig).unwrap();
    let o = Options { backup_dir: Some(bk.path().into()), ..Default::default() };
    let u = undo_run(&o, d.path());
    assert_eq!(u.len(), 1);
    assert_ne!(std::fs::read(&p).unwrap(), orig);
    // edited since: refuse and leave it alone
    let edited = std::fs::read(&p).unwrap();
    std::fs::write(&p, b"changed").unwrap();
    assert!(compress_core::undo::undo(&u[0]).is_err());
    std::fs::write(&p, &edited).unwrap();
    compress_core::undo::undo(&u[0]).unwrap();
    assert_eq!(std::fs::read(&p).unwrap(), orig);
}

#[test]
fn undo_of_an_alongside_output_removes_it_and_keeps_the_original() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("a.png");
    std::fs::write(&p, png_bytes(96, 96)).unwrap();
    let o = Options { destination: "alongside".into(), backup_dir: Some(d.path().join("bk")), ..Default::default() };
    let u = undo_run(&o, &d.path().join("a.png").parent().unwrap().to_path_buf());
    assert_eq!(u.len(), 1);
    assert!(u[0].restore.is_empty());
    assert!(d.path().join("a-min.png").exists());
    compress_core::undo::undo(&u[0]).unwrap();
    assert!(!d.path().join("a-min.png").exists() && p.exists());
}

#[test]
fn no_backup_dir_means_no_undo_record() {
    let d = tempfile::tempdir().unwrap();
    std::fs::write(d.path().join("a.png"), png_bytes(96, 96)).unwrap();
    assert!(undo_run(&Options::default(), d.path()).is_empty());
}

#[cfg(target_os = "macos")]
#[test]
fn extended_attributes_survive_compression_and_undo() {
    let get = |p: &std::path::Path| String::from_utf8_lossy(&std::process::Command::new("/usr/bin/xattr").args(["-p", "user.tag"]).arg(p).output().unwrap().stdout).trim().to_string();
    let d = tempfile::tempdir().unwrap();
    let bk = tempfile::tempdir().unwrap();
    let p = d.path().join("a.png");
    std::fs::write(&p, png_bytes(96, 96)).unwrap();
    assert!(std::process::Command::new("/usr/bin/xattr").args(["-w", "user.tag", "red"]).arg(&p).status().unwrap().success());
    let u = undo_run(&Options { backup_dir: Some(bk.path().into()), ..Default::default() }, d.path());
    assert_eq!(get(&p), "red", "compressed file lost its attribute");
    compress_core::undo::undo(&u[0]).unwrap();
    assert_eq!(get(&p), "red", "restored file lost its attribute");
}

#[cfg(unix)]
#[test]
fn unwritable_folder_error_names_the_folder_not_the_temp_file() {
    use std::os::unix::fs::PermissionsExt;
    let d = tempfile::tempdir().unwrap();
    std::fs::write(d.path().join("a.png"), png_bytes(256, 256)).unwrap();
    std::fs::set_permissions(d.path(), std::fs::Permissions::from_mode(0o555)).unwrap();
    let (_, ev) = go(d.path(), &Options::default());
    std::fs::set_permissions(d.path(), std::fs::Permissions::from_mode(0o755)).unwrap();
    let err = ev.iter().find_map(|e| match e { compress_core::JobEvent::Failed { error, .. } => Some(error.clone()), _ => None }).expect("a failure");
    assert!(!err.contains(".imgcompress-"), "{err}");
    assert!(err.contains("cannot write to") && err.contains("permission denied"), "{err}");
}

#[cfg(unix)]
#[test]
fn trash_twin_of_a_restored_original_is_found_by_inode() {
    use std::os::unix::fs::MetadataExt;
    let d = tempfile::tempdir().unwrap();
    let trash = d.path().join("Trash");
    std::fs::create_dir(&trash).unwrap();
    let orig = d.path().join("a.png");
    std::fs::write(&orig, b"x").unwrap();
    std::fs::write(trash.join("other.png"), b"x").unwrap();
    std::fs::hard_link(&orig, trash.join("a.png")).unwrap();
    let m = std::fs::metadata(&orig).unwrap();
    assert!(compress_core::undo::remove_twin_in(&[trash.clone()], m.dev(), m.ino()));
    assert!(!trash.join("a.png").exists() && trash.join("other.png").exists() && orig.exists());
    assert!(!compress_core::undo::remove_twin_in(&[trash], m.dev(), m.ino()));
}
