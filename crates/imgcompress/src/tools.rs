//! `imgcompress tools`: versions of the linked encoders and external helpers.
use std::ffi::CStr;

pub struct Tool {
    pub name: &'static str,
    pub version: String,
    /// "linked", "OS" or the helper's path
    pub source: String,
    pub ok: bool,
}

fn dep(name: &str) -> String {
    env!("IMGCOMPRESS_DEP_VERSIONS").split(',').find_map(|kv| kv.strip_prefix(name).and_then(|r| r.strip_prefix('='))).unwrap_or("?").to_string()
}

fn linked(name: &'static str, version: String) -> Tool {
    Tool { name, version, source: "linked".into(), ok: true }
}

pub fn gather() -> Vec<Tool> {
    let mut v = vec![
        linked("imagequant", dep("imagequant")),
        linked("oxipng", dep("oxipng")),
        linked("mozjpeg", dep("mozjpeg-sys")),
        linked("jpegli", format!("libjxl {}", jxl_version())),
        linked("libjxl", jxl_version()),
        linked("libavif", unsafe { CStr::from_ptr(libavif_sys::avifVersion()) }.to_string_lossy().into_owned()),
        linked("libwebp", webp_version()),
    ];
    if cfg!(target_os = "macos") {
        v.push(Tool { name: "heic", version: "ImageIO".into(), source: "OS".into(), ok: true });
    } else {
        // optional on Linux (opt-in feature of compress-core); never makes `--check` fail
        v.push(Tool { name: "heic", version: "not built".into(), source: "-".into(), ok: true });
    }
    v.push(match compress_core::gif_codec::find_gifsicle() {
        Some(p) => {
            let ver = std::process::Command::new(&p).arg("--version").output().ok().and_then(|o| String::from_utf8(o.stdout).ok()).and_then(|s| s.lines().next().map(str::to_string)).unwrap_or_else(|| "?".into());
            Tool { name: "gifsicle", version: ver.trim_start_matches("LCDF Gifsicle ").to_string(), source: p.display().to_string(), ok: true }
        }
        None => Tool { name: "gifsicle", version: "missing".into(), source: "-".into(), ok: false },
    });
    v
}

fn jxl_version() -> String {
    let v = unsafe { jpegxl_sys::encoder::encode::JxlEncoderVersion() };
    format!("{}.{}.{}", v / 1_000_000, (v / 1000) % 1000, v % 1000)
}

fn webp_version() -> String {
    let v = unsafe { libwebp_sys::WebPGetEncoderVersion() };
    format!("{}.{}.{}", (v >> 16) & 0xff, (v >> 8) & 0xff, v & 0xff)
}
