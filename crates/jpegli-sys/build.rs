// Builds jpegli (github.com/google/jpegli, NOT libjxl >=0.12 which dropped it) statically.
// Native API is `jpegli_*` prefixed, so it never clashes with mozjpeg's `jpeg_*`.
// Source: env JPEGLI_SRC (checkout with --recurse-submodules; pinned 031a007) or ./vendor/jpegli.
use std::{env, path::PathBuf, process::Command};
fn run(c: &mut Command) {
    assert!(c.status().expect("spawn").success(), "{:?} failed", c);
}
fn main() {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let src = env::var("JPEGLI_SRC").map(PathBuf::from).unwrap_or(manifest.join("vendor/jpegli"));
    println!("cargo:rerun-if-env-changed=JPEGLI_SRC");
    if !src.join("CMakeLists.txt").exists() {
        // Clean checkout: fetch the pinned commit (scripts/fetch_jpegli.sh).
        let script = manifest.join("../../scripts/fetch_jpegli.sh");
        run(Command::new("bash").arg(script).arg(&src));
    }
    let out = PathBuf::from(env::var("OUT_DIR").unwrap()).join("build");
    run(Command::new("cmake").args(["-S"]).arg(&src).arg("-B").arg(&out).args([
        "-G", "Ninja", "-DCMAKE_BUILD_TYPE=Release", "-DBUILD_SHARED_LIBS=OFF",
        "-DCMAKE_POSITION_INDEPENDENT_CODE=ON", "-DBUILD_TESTING=OFF",
        "-DJPEGLI_ENABLE_TOOLS=OFF", "-DJPEGLI_ENABLE_DEVTOOLS=OFF", "-DJPEGLI_ENABLE_JPEGLI_LIBJPEG=OFF",
        "-DJPEGLI_ENABLE_BENCHMARK=OFF", "-DJPEGLI_ENABLE_MANPAGES=OFF", "-DJPEGLI_ENABLE_DOXYGEN=OFF",
        "-DJPEGLI_ENABLE_JNI=OFF", "-DJPEGLI_ENABLE_SJPEG=OFF",
    ]));
    run(Command::new("cmake").arg("--build").arg(&out).args(["--target", "jpegli-static", "hwy"]));
    println!("cargo:rustc-link-search=native={}", out.join("lib").display());
    println!("cargo:rustc-link-search=native={}", out.join("third_party/highway").display());
    println!("cargo:rustc-link-lib=static=jpegli-static");
    println!("cargo:rustc-link-lib=static=hwy");
    println!("cargo:rustc-link-lib={}", if cfg!(target_os = "macos") { "c++" } else { "stdc++" });
}
