//! AVIF via libavif (libaom encoder, dav1d decoder).
use crate::pixels::{CResult, Decoded, Frames, Rgba};
use libavif_sys as a;
use std::ffi::CStr;

fn chk(r: a::avifResult, what: &str) -> CResult<()> {
    if r != a::AVIF_RESULT_OK {
        let s = unsafe { CStr::from_ptr(a::avifResultToString(r)) };
        return Err(format!("avif {what}: {}", s.to_string_lossy()));
    }
    Ok(())
}

pub fn versions() -> String {
    let mut buf = [0 as std::os::raw::c_char; 256];
    unsafe { a::avifCodecVersions(buf.as_mut_ptr()) };
    unsafe { CStr::from_ptr(buf.as_ptr()) }.to_string_lossy().into_owned()
}

pub struct EncOpts {
    /// 0..100 (avifenc -q)
    pub quality: i32,
    /// 0..100 (avifenc --qalpha)
    pub alpha_quality: i32,
    /// 0..10 (avifenc -s)
    pub speed: i32,
    /// 8 | 10 | 12
    pub depth: u32,
    /// 0 = all cores
    pub threads: i32,
    pub icc: Option<Vec<u8>>,
    /// Raw TIFF payload.
    pub exif: Option<Vec<u8>>,
    pub xmp: Option<Vec<u8>>,
}

/// Build a YUV444 avifImage from RGBA8. `alpha` decides whether the alpha plane is written (sequences need it
/// to agree across frames).
unsafe fn make_image(im: &Rgba, o: &EncOpts, alpha: bool) -> CResult<*mut a::avifImage> {
    let img = a::avifImageCreate(im.w, im.h, o.depth, a::AVIF_PIXEL_FORMAT_YUV444);
    if img.is_null() {
        return Err("avif alloc".into());
    }
    (*img).yuvRange = a::AVIF_RANGE_FULL;
    (*img).matrixCoefficients = a::AVIF_MATRIX_COEFFICIENTS_BT601 as _;
    match &o.icc {
        Some(icc) => {
            a::avifImageSetProfileICC(img, icc.as_ptr(), icc.len());
        }
        None => {
            (*img).colorPrimaries = a::AVIF_COLOR_PRIMARIES_BT709 as _;
            (*img).transferCharacteristics = a::AVIF_TRANSFER_CHARACTERISTICS_SRGB as _;
        }
    }
    for (set, blob) in [(a::avifImageSetMetadataExif as unsafe extern "C" fn(_, _, _) -> _, &o.exif), (a::avifImageSetMetadataXMP, &o.xmp)] {
        if let Some(b) = blob {
            let r = set(img, b.as_ptr(), b.len());
            if r != a::AVIF_RESULT_OK {
                a::avifImageDestroy(img);
                chk(r, "metadata")?;
            }
        }
    }
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
    Ok(img)
}

unsafe fn new_encoder(o: &EncOpts) -> *mut a::avifEncoder {
    let enc = a::avifEncoderCreate();
    (*enc).codecChoice = a::AVIF_CODEC_CHOICE_AOM;
    (*enc).quality = o.quality;
    (*enc).qualityAlpha = o.alpha_quality;
    (*enc).speed = o.speed;
    (*enc).maxThreads = if o.threads > 0 { o.threads } else { std::thread::available_parallelism().map(|n| n.get() as i32).unwrap_or(4) };
    enc
}

pub fn encode(im: &Rgba, o: &EncOpts) -> CResult<Vec<u8>> {
    unsafe {
        let img = make_image(im, o, im.has_alpha())?;
        let enc = new_encoder(o);
        let mut out: a::avifRWData = std::mem::zeroed();
        let r = a::avifEncoderWrite(enc, img, &mut out);
        let res = if r == a::AVIF_RESULT_OK { Ok(std::slice::from_raw_parts(out.data, out.size).to_vec()) } else { chk(r, "encode").map(|_| vec![]) };
        a::avifRWDataFree(&mut out);
        a::avifEncoderDestroy(enc);
        a::avifImageDestroy(img);
        res
    }
}

/// Encode frames as an AVIF image sequence (`avis`), timescale 1000 (delays in ms). `loop_count` 0 = forever.
pub fn encode_sequence(g: &Frames, o: &EncOpts) -> CResult<Vec<u8>> {
    unsafe {
        let alpha = g.frames.iter().any(|(px, _)| px.chunks_exact(4).any(|p| p[3] != 255));
        let enc = new_encoder(o);
        (*enc).timescale = 1000;
        (*enc).repetitionCount = if g.loop_count <= 0 { a::AVIF_REPETITION_COUNT_INFINITE } else { g.loop_count - 1 };
        let mut out: a::avifRWData = std::mem::zeroed();
        let res = (|| -> CResult<Vec<u8>> {
            for (px, delay) in &g.frames {
                let im = Rgba { w: g.w, h: g.h, px: px.clone() };
                let img = make_image(&im, o, alpha)?;
                let r = a::avifEncoderAddImage(enc, img, (*delay).max(1) as u64, a::AVIF_ADD_IMAGE_FLAG_NONE);
                a::avifImageDestroy(img);
                chk(r, "add frame")?;
            }
            chk(a::avifEncoderFinish(enc, &mut out), "finish")?;
            Ok(std::slice::from_raw_parts(out.data, out.size).to_vec())
        })();
        a::avifRWDataFree(&mut out);
        a::avifEncoderDestroy(enc);
        res
    }
}

/// Current decoder image -> RGBA8 with irot/imir applied (container transforms: rotation first, then mirror).
unsafe fn current_image(img: *mut a::avifImage) -> CResult<Decoded> {
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
    let icc = if (*img).icc.size > 0 { Some(std::slice::from_raw_parts((*img).icc.data, (*img).icc.size).to_vec()) } else { None };
    let flags = (*img).transformFlags;
    let mut im = image::RgbaImage::from_raw(w, h, px).ok_or("avif size mismatch")?;
    if flags & (a::AVIF_TRANSFORM_IROT as u32) != 0 {
        im = match (*img).irot.angle & 3 {
            1 => image::imageops::rotate270(&im), // 90 degrees counter-clockwise
            2 => image::imageops::rotate180(&im),
            3 => image::imageops::rotate90(&im),
            _ => im,
        };
    }
    if flags & (a::AVIF_TRANSFORM_IMIR as u32) != 0 {
        im = if (*img).imir.axis == 0 { image::imageops::flip_vertical(&im) } else { image::imageops::flip_horizontal(&im) };
    }
    let blob = |d: &a::avifRWData| (d.size > 0).then(|| std::slice::from_raw_parts(d.data, d.size).to_vec());
    let mut dec = Decoded::new(Rgba { w: im.width(), h: im.height(), px: im.into_raw() }, icc);
    dec.exif = blob(&(*img).exif);
    dec.xmp = blob(&(*img).xmp);
    Ok(dec)
}

unsafe fn open_decoder(data: &[u8]) -> CResult<*mut a::avifDecoder> {
    let d = a::avifDecoderCreate();
    (*d).maxThreads = std::thread::available_parallelism().map(|n| n.get() as i32).unwrap_or(4);
    let r = chk(a::avifDecoderSetIOMemory(d, data.as_ptr(), data.len()), "io").and_then(|_| chk(a::avifDecoderParse(d), "parse"));
    if let Err(e) = r {
        a::avifDecoderDestroy(d);
        return Err(e);
    }
    Ok(d)
}

/// Decode the first frame (irot/imir applied) to RGBA8, plus the number of frames in the file.
pub fn decode(data: &[u8]) -> CResult<(Decoded, i32)> {
    unsafe {
        let d = open_decoder(data)?;
        let res = (|| -> CResult<(Decoded, i32)> {
            let frames = (*d).imageCount;
            chk(a::avifDecoderNextImage(d), "decode")?;
            Ok((current_image((*d).image)?, frames))
        })();
        a::avifDecoderDestroy(d);
        res
    }
}

/// Decode every frame of an image sequence to RGBA8 with delays in ms. The ICC profile of the first frame, if any,
/// is returned alongside.
pub fn decode_sequence(data: &[u8]) -> CResult<(Frames, Option<Vec<u8>>)> {
    unsafe {
        let d = open_decoder(data)?;
        let res = (|| -> CResult<(Frames, Option<Vec<u8>>)> {
            let expect = (*d).imageCount.max(1) as usize;
            let lc = match (*d).repetitionCount {
                a::AVIF_REPETITION_COUNT_INFINITE | a::AVIF_REPETITION_COUNT_UNKNOWN => 0,
                n => n.max(0) + 1,
            };
            let mut out = Frames { w: 0, h: 0, frames: vec![], loop_count: lc };
            let mut icc = None;
            loop {
                let r = a::avifDecoderNextImage(d);
                if r == a::AVIF_RESULT_NO_IMAGES_REMAINING {
                    break;
                }
                chk(r, "decode frame")?;
                let ms = ((*d).imageTiming.duration * 1000.0).round() as i32;
                let dec = current_image((*d).image)?;
                if out.frames.is_empty() {
                    (out.w, out.h, icc) = (dec.img.w, dec.img.h, dec.icc);
                } else if (dec.img.w, dec.img.h) != (out.w, out.h) {
                    return Err("avif sequence frames differ in size".into());
                }
                out.frames.push((dec.img.px, ms.max(1)));
            }
            if out.frames.len() != expect {
                return Err(format!("avif sequence: decoded {} of {expect} frames", out.frames.len()));
            }
            Ok((out, icc))
        })();
        a::avifDecoderDestroy(d);
        res
    }
}

/// Number of images (frames of an image sequence) after parsing only; 1 on any error.
pub fn frame_count(data: &[u8]) -> u32 {
    unsafe {
        let d = a::avifDecoderCreate();
        let n = if a::avifDecoderSetIOMemory(d, data.as_ptr(), data.len()) == a::AVIF_RESULT_OK && a::avifDecoderParse(d) == a::AVIF_RESULT_OK {
            ((*d).imageCount.max(1)) as u32
        } else {
            1
        };
        a::avifDecoderDestroy(d);
        n
    }
}
