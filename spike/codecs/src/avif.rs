use crate::{has_alpha, Rgba};
use anyhow::{bail, Result};
use libavif_sys as a;
use std::ffi::CStr;

fn chk(r: a::avifResult, what: &str) -> Result<()> {
    if r != a::AVIF_RESULT_OK {
        let s = unsafe { CStr::from_ptr(a::avifResultToString(r)) };
        bail!("{what}: {}", s.to_string_lossy());
    }
    Ok(())
}

pub fn versions() -> String {
    let mut buf = [0i8; 256];
    unsafe { a::avifCodecVersions(buf.as_mut_ptr()) };
    unsafe { CStr::from_ptr(buf.as_ptr()) }.to_string_lossy().into_owned()
}

/// quality 0..100 (libavif scale, same as avifenc -q), speed 0..10 (avifenc -s), depth 8|10|12, threads 0 = all.
pub fn encode(im: &Rgba, quality: i32, speed: i32, depth: u32, threads: i32) -> Result<Vec<u8>> {
    unsafe {
        let img = a::avifImageCreate(im.w, im.h, depth, a::AVIF_PIXEL_FORMAT_YUV420);
        if img.is_null() {
            bail!("alloc");
        }
        (*img).yuvRange = a::AVIF_RANGE_FULL;
        (*img).matrixCoefficients = a::AVIF_MATRIX_COEFFICIENTS_BT601 as _;
        let alpha = has_alpha(im);
        let mut rgb: a::avifRGBImage = std::mem::zeroed();
        a::avifRGBImageSetDefaults(&mut rgb, img);
        rgb.format = a::AVIF_RGB_FORMAT_RGBA;
        rgb.depth = 8;
        rgb.ignoreAlpha = (!alpha) as _;
        rgb.pixels = im.px.as_ptr() as *mut u8; // read-only use
        rgb.rowBytes = im.w * 4;
        let r = a::avifImageRGBToYUV(img, &rgb);
        if r != a::AVIF_RESULT_OK {
            a::avifImageDestroy(img);
            chk(r, "rgb->yuv")?;
        }
        let enc = a::avifEncoderCreate();
        (*enc).codecChoice = a::AVIF_CODEC_CHOICE_AOM;
        (*enc).quality = quality;
        (*enc).qualityAlpha = quality;
        (*enc).speed = speed;
        (*enc).maxThreads = if threads > 0 { threads } else { std::thread::available_parallelism().map(|n| n.get() as i32).unwrap_or(4) };
        let mut out: a::avifRWData = std::mem::zeroed();
        let r = a::avifEncoderWrite(enc, img, &mut out);
        let res = if r == a::AVIF_RESULT_OK { Ok(std::slice::from_raw_parts(out.data, out.size).to_vec()) } else { chk(r, "encode").map(|_| vec![]) };
        a::avifRWDataFree(&mut out);
        a::avifEncoderDestroy(enc);
        a::avifImageDestroy(img);
        res
    }
}

pub fn decode(data: &[u8]) -> Result<Rgba> {
    unsafe {
        let d = a::avifDecoderCreate();
        (*d).maxThreads = 8;
        // AUTO prefers dav1d when linked; force aom with SPIKE_AOM_DEC=1 for comparison.
        (*d).codecChoice = if std::env::var_os("SPIKE_AOM_DEC").is_some() { a::AVIF_CODEC_CHOICE_AOM } else { a::AVIF_CODEC_CHOICE_AUTO };
        let mut res = (|| -> Result<Rgba> {
            chk(a::avifDecoderSetIOMemory(d, data.as_ptr(), data.len()), "io")?;
            chk(a::avifDecoderParse(d), "parse")?;
            chk(a::avifDecoderNextImage(d), "decode")?;
            let img = (*d).image;
            let mut rgb: a::avifRGBImage = std::mem::zeroed();
            a::avifRGBImageSetDefaults(&mut rgb, img);
            rgb.format = a::AVIF_RGB_FORMAT_RGBA;
            rgb.depth = 8;
            chk(a::avifRGBImageAllocatePixels(&mut rgb), "alloc")?;
            let r = a::avifImageYUVToRGB(img, &mut rgb);
            let mut px = Vec::with_capacity((rgb.width * rgb.height * 4) as usize);
            if r == a::AVIF_RESULT_OK {
                for y in 0..rgb.height {
                    px.extend_from_slice(std::slice::from_raw_parts(rgb.pixels.add((y * rgb.rowBytes) as usize), (rgb.width * 4) as usize));
                }
            }
            let (w, h) = (rgb.width, rgb.height);
            a::avifRGBImageFreePixels(&mut rgb);
            chk(r, "yuv->rgb")?;
            Ok(Rgba { w, h, px })
        })();
        a::avifDecoderDestroy(d);
        if let Err(e) = &mut res {
            let _ = e;
        }
        res
    }
}

/// ravif: quality 1..100, speed 1..10
pub fn encode_ravif(im: &Rgba, quality: f32, speed: u8) -> Result<Vec<u8>> {
    use rgb::FromSlice;
    let img = ravif::Img::new(im.px.as_rgba(), im.w as usize, im.h as usize);
    let r = ravif::Encoder::new().with_quality(quality).with_alpha_quality(quality).with_speed(speed).encode_rgba(img)?;
    Ok(r.avif_file)
}
