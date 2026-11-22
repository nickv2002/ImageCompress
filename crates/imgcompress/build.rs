// Export versions of the linked Rust crates (from the workspace Cargo.lock) for `imgcompress tools`.
fn main() {
    let lock = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Cargo.lock");
    println!("cargo:rerun-if-changed={}", lock.display());
    let s = std::fs::read_to_string(&lock).unwrap_or_default();
    let mut out = Vec::new();
    let mut name = String::new();
    for l in s.lines() {
        if let Some(n) = l.strip_prefix("name = \"") {
            name = n.trim_end_matches('"').to_string();
        } else if let Some(v) = l.strip_prefix("version = \"") {
            if ["imagequant", "oxipng", "mozjpeg-sys", "image", "png", "jxl", "lcms2", "zune-jpeg"].contains(&name.as_str()) {
                out.push(format!("{name}={}", v.trim_end_matches('"')));
            }
        }
    }
    println!("cargo:rustc-env=IMGCOMPRESS_DEP_VERSIONS={}", out.join(","));
}
