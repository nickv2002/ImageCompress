mod jpeg_lossless;
use jpeg_lossless::*;
use std::time::Instant;

fn decode(b: &[u8]) -> (u32, u32, Vec<u8>) {
    let im = image::load_from_memory_with_format(b, image::ImageFormat::Jpeg).unwrap().to_rgb8();
    (im.width(), im.height(), im.into_raw())
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args[0].as_str() {
        // lossless <in> <out_prog> <out_base>
        "lossless" => {
            let b = std::fs::read(&args[1]).unwrap();
            let t = Instant::now();
            let p = optimize(&b, &LosslessOpts { progressive: true, markers: MarkerPolicy::None }).unwrap();
            let bl = optimize(&b, &LosslessOpts { progressive: false, markers: MarkerPolicy::None }).unwrap();
            let icc = optimize(&b, &LosslessOpts { progressive: true, markers: MarkerPolicy::Icc }).unwrap();
            let all = optimize(&b, &LosslessOpts { progressive: true, markers: MarkerPolicy::All }).unwrap();
            let el = t.elapsed();
            std::fs::write(&args[2], &p).unwrap();
            std::fs::write(&args[3], &bl).unwrap();
            std::fs::write(&args[4], &all).unwrap();
            let same = decode(&b) == decode(&p) && decode(&b) == decode(&bl) && decode(&b) == decode(&icc) && decode(&b) == decode(&all);
            println!("{} orig={} prog={} base={} icc={} all={} pixel_identical={} ({:?} for 4)", args[1], b.len(), p.len(), bl.len(), icc.len(), all.len(), same, el);
        }
        // lossy <in.jpg> <q> <out>
        "lossy" => {
            let (w, h, rgb) = decode(&std::fs::read(&args[1]).unwrap());
            let q: u8 = args[2].parse().unwrap();
            let t = Instant::now();
            let o = encode_rgb8(&rgb, w, h, &LossyOpts { quality: q, ..Default::default() }).unwrap();
            println!("{} q{} -> {} bytes ({:?})", args[1], q, o.len(), t.elapsed());
            std::fs::write(&args[3], o).unwrap();
        }
        // ppm out for cjpeg: pnm <in.jpg> <out.ppm>
        "ppm" => {
            let (w, h, rgb) = decode(&std::fs::read(&args[1]).unwrap());
            let mut v = format!("P6\n{} {}\n255\n", w, h).into_bytes();
            v.extend(rgb);
            std::fs::write(&args[2], v).unwrap();
        }
        // jpegli <in.jpg> <distance> <out>
        "jpegli" => {
            let (w, h, rgb) = decode(&std::fs::read(&args[1]).unwrap());
            let d: f32 = args[2].parse().unwrap();
            let t = Instant::now();
            let o = encode_rgb8_jpegli(&rgb, w, h, &JpegliOpts { distance: d, ..Default::default() }).unwrap();
            println!("{} d{} -> {} bytes ({:?})", args[1], d, o.len(), t.elapsed());
            std::fs::write(&args[3], o).unwrap();
        }
        _ => {}
    }
}
