//! `score <source> <output>`: SSIMULACRA2 of `output` against `source` (both decoded with the engine's
//! own decoders, composited on mid grey like the quality search). Prints `n/a` if the sizes differ
//! (e.g. an EXIF-rotated source whose orientation the engine baked in).
use compress_core::convert::{decode_any, Fmt};
use fast_ssim2::{PixelDescriptor, PixelSlice, Ssimulacra2Reference};

fn load(p: &str) -> compress_core::pixels::Rgba {
    let ext = std::path::Path::new(p).extension().unwrap().to_str().unwrap().to_ascii_lowercase();
    decode_any(&std::fs::read(p).unwrap(), Fmt::from_ext(&ext).unwrap()).unwrap().img
}

fn flat(im: &compress_core::pixels::Rgba) -> Vec<u8> {
    let mut v = Vec::with_capacity(im.px.len() / 4 * 3);
    for p in im.px.chunks_exact(4) {
        let a = p[3] as u32;
        for c in &p[..3] {
            v.push(((*c as u32 * a + 128 * (255 - a) + 127) / 255) as u8);
        }
    }
    v
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let (s, o) = (load(&a[1]), load(&a[2]));
    if (s.w, s.h) != (o.w, o.h) {
        println!("n/a");
        return;
    }
    let (rs, ro) = (flat(&s), flat(&o));
    let stride = s.w as usize * 3;
    let a_sl = PixelSlice::new(&rs, s.w, s.h, stride, PixelDescriptor::RGB8_SRGB).unwrap();
    let b_sl = PixelSlice::new(&ro, s.w, s.h, stride, PixelDescriptor::RGB8_SRGB).unwrap();
    let r = Ssimulacra2Reference::new(&a_sl).unwrap();
    println!("{:.1}", r.compare(&b_sl).unwrap());
}
