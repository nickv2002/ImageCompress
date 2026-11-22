//! JPEG XL: libjxl (jpegxl-rs) for encoding / JPEG transcode, jxl-rs for decoding.
use crate::pixels::{err, CResult, Decoded, Rgba};
use jpegxl_rs::{
    decode::decoder_builder,
    encode::{encoder_builder, EncoderFrame, EncoderSpeed},
    parallel::threads_runner::ThreadsRunner,
};

fn speed(effort: u32) -> EncoderSpeed {
    use EncoderSpeed::*;
    match effort {
        0 | 1 => Lightning, 2 => Thunder, 3 => Falcon, 4 => Cheetah, 5 => Hare,
        6 => Wombat, 7 => Squirrel, 8 => Kitten, 9 => Tortoise, _ => Glacier,
    }
}

/// libjxl's quality (cjxl -q) -> butteraugli distance.
pub fn quality_to_distance(q: u8) -> f32 {
    let q = q as f32;
    if q >= 100.0 {
        0.0
    } else if q >= 30.0 {
        0.1 + (100.0 - q) * 0.09
    } else {
        6.4 + 2.5f32.powf((30.0 - q) / 5.0) / 6.25
    }
}

fn encode_inner(im: &Rgba, lossless: bool, distance: f32, effort: u32, progressive: bool) -> CResult<Vec<u8>> {
    let alpha = im.has_alpha();
    let runner = if (im.w as u64) * (im.h as u64) > 1_000_000 { ThreadsRunner::default().into() } else { None::<ThreadsRunner> };
    let prl: Option<&dyn jpegxl_rs::parallel::ParallelRunner> = runner.as_ref().map(|r| r as &dyn jpegxl_rs::parallel::ParallelRunner);
    let mut enc = encoder_builder()
        .has_alpha(alpha)
        .lossless(lossless)
        .speed(speed(effort))
        .uses_original_profile(lossless)
        .maybe_quality(if lossless { None } else { Some(distance) })
        .maybe_parallel_runner(prl)
        .build()
        .map_err(err)?;
    if progressive {
        use jpegxl_sys::encoder::encode::JxlEncoderFrameSettingId as F;
        enc.set_frame_option(F::ProgressiveAc, 1);
    }
    // has_alpha(true) <=> 4 channels; opaque input must be fed 3 channels.
    if alpha {
        enc.encode_frame(&EncoderFrame::new(&im.px).num_channels(4), im.w, im.h).map_err(err)
    } else {
        let rgb = im.rgb3();
        enc.encode_frame(&EncoderFrame::new(&rgb).num_channels(3), im.w, im.h).map_err(err)
    }
    
}

/// Colour profile and metadata blobs to embed. jpegxl-rs has no ICC setter and `add_metadata` lacks
/// the Exif box's TIFF-offset prefix, so a non-empty `Embed` takes the raw libjxl path below.
#[derive(Default, Clone, Copy)]
pub struct Embed<'a> {
    pub icc: Option<&'a [u8]>,
    /// Raw TIFF payload (no `Exif\0\0`); the 4-byte box offset prefix is added here.
    pub exif: Option<&'a [u8]>,
    pub xmp: Option<&'a [u8]>,
}

impl Embed<'_> {
    fn is_empty(&self) -> bool {
        self.icc.is_none() && self.exif.is_none() && self.xmp.is_none()
    }
}

fn box_type(t: &[u8; 4]) -> jpegxl_sys::common::types::JxlBoxType {
    jpegxl_sys::common::types::JxlBoxType(t.map(|c| c as std::ffi::c_char))
}

fn encode_raw(im: &Rgba, lossless: bool, distance: f32, effort: u32, progressive: bool, emb: &Embed) -> CResult<Vec<u8>> {
    use jpegxl_sys::{common::types::*, encoder::encode::*, metadata::codestream_header::*, threads::thread_parallel_runner::*};
    use std::ptr::null;
    struct Guard(*mut JxlEncoder, *mut std::ffi::c_void);
    impl Drop for Guard {
        fn drop(&mut self) {
            unsafe {
                JxlEncoderDestroy(self.0);
                if !self.1.is_null() {
                    JxlThreadParallelRunnerDestroy(self.1);
                }
            }
        }
    }
    unsafe {
        let enc = JxlEncoderCreate(null());
        if enc.is_null() {
            return Err("jxl: encoder alloc".into());
        }
        let mut g = Guard(enc, std::ptr::null_mut());
        let chk = |s: JxlEncoderStatus, what: &str| -> CResult<()> {
            if s == JxlEncoderStatus::Success {
                Ok(())
            } else {
                Err(format!("jxl {what}: {:?}", JxlEncoderGetError(enc)))
            }
        };
        if (im.w as u64) * (im.h as u64) > 1_000_000 {
            g.1 = JxlThreadParallelRunnerCreate(null(), JxlThreadParallelRunnerDefaultNumWorkerThreads());
            chk(JxlEncoderSetParallelRunner(enc, JxlThreadParallelRunner, g.1), "runner")?;
        }
        let boxes = emb.exif.is_some() || emb.xmp.is_some();
        if boxes {
            chk(JxlEncoderUseContainer(enc, true.into()), "container")?;
            chk(JxlEncoderUseBoxes(enc), "boxes")?;
        }
        let alpha = im.has_alpha();
        let mut bi_u = std::mem::MaybeUninit::<JxlBasicInfo>::uninit();
        JxlEncoderInitBasicInfo(bi_u.as_mut_ptr());
        let mut bi = bi_u.assume_init();
        bi.xsize = im.w;
        bi.ysize = im.h;
        bi.bits_per_sample = 8;
        bi.exponent_bits_per_sample = 0;
        bi.uses_original_profile = lossless.into();
        bi.num_color_channels = 3;
        if alpha {
            bi.num_extra_channels = 1;
            bi.alpha_bits = 8;
            bi.alpha_exponent_bits = 0;
        }
        chk(JxlEncoderSetBasicInfo(enc, &bi), "basic info")?;
        match emb.icc {
            Some(icc) => chk(JxlEncoderSetICCProfile(enc, icc.as_ptr(), icc.len()), "icc profile")?,
            None => {
                let mut ce = std::mem::MaybeUninit::uninit();
                JxlColorEncodingSetToSRGB(ce.as_mut_ptr(), false.into());
                chk(JxlEncoderSetColorEncoding(enc, ce.as_ptr()), "color encoding")?;
            }
        }
        if let Some(x) = emb.exif {
            let mut b = vec![0u8; 4];
            b.extend_from_slice(x);
            chk(JxlEncoderAddBox(enc, &box_type(b"Exif"), b.as_ptr(), b.len(), true.into()), "exif box")?;
        }
        if let Some(x) = emb.xmp {
            chk(JxlEncoderAddBox(enc, &box_type(b"xml "), x.as_ptr(), x.len(), true.into()), "xmp box")?;
        }
        if boxes {
            JxlEncoderCloseBoxes(enc);
        }
        let fs = JxlEncoderFrameSettingsCreate(enc, null());
        chk(JxlEncoderFrameSettingsSetOption(fs, JxlEncoderFrameSettingId::Effort, effort.clamp(1, 10) as i64), "effort")?;
        if progressive {
            chk(JxlEncoderFrameSettingsSetOption(fs, JxlEncoderFrameSettingId::ProgressiveAc, 1), "progressive")?;
        }
        if lossless {
            chk(JxlEncoderSetFrameLossless(fs, true.into()), "lossless")?;
        } else {
            chk(JxlEncoderSetFrameDistance(fs, distance), "distance")?;
        }
        let rgb;
        let (buf, nch): (&[u8], u32) = if alpha {
            (&im.px, 4)
        } else {
            rgb = im.rgb3();
            (&rgb, 3)
        };
        let pf = JxlPixelFormat { num_channels: nch, data_type: JxlDataType::Uint8, endianness: JxlEndianness::Native, align: 0 };
        chk(JxlEncoderAddImageFrame(fs, &pf, buf.as_ptr().cast(), buf.len()), "add frame")?;
        JxlEncoderCloseInput(enc);
        let mut out: Vec<u8> = vec![0; 1 << 16];
        let mut used = 0usize;
        loop {
            let mut next = out.as_mut_ptr().add(used);
            let mut avail = out.len() - used;
            let st = JxlEncoderProcessOutput(enc, &mut next, &mut avail);
            used = out.len() - avail;
            match st {
                JxlEncoderStatus::Success => break,
                JxlEncoderStatus::NeedMoreOutput => out.resize(out.len() * 2, 0),
                JxlEncoderStatus::Error => return Err(format!("jxl encode: {:?}", JxlEncoderGetError(enc))),
            }
        }
        out.truncate(used);
        Ok(out)
    }
}

/// Lossy (`quality_distance`, butteraugli) or lossless encode that also embeds `emb`.
pub fn encode_embedded(im: &Rgba, lossless: bool, distance: f32, effort: u32, progressive: bool, emb: &Embed) -> CResult<Vec<u8>> {
    if emb.is_empty() {
        encode_inner(im, lossless, distance, effort, progressive)
    } else {
        encode_raw(im, lossless, distance, effort, progressive, emb)
    }
}

/// distance: butteraugli distance. 1.0 ~ visually lossless.
pub fn encode_lossy(im: &Rgba, distance: f32, effort: u32, progressive: bool) -> CResult<Vec<u8>> {
    encode_inner(im, false, distance, effort, progressive)
}

pub fn encode_lossless(im: &Rgba, effort: u32) -> CResult<Vec<u8>> {
    encode_inner(im, true, 0.0, effort, false)
}

/// Reversible JPEG recompression (what cjxl does on a .jpg by default).
pub fn transcode_jpeg(jpeg: &[u8], effort: u32) -> CResult<Vec<u8>> {
    let mut enc = encoder_builder().use_container(true).uses_original_profile(true).speed(speed(effort)).build().map_err(err)?;
    enc.encode_jpeg(jpeg).map_err(err)
}

/// JXL -> original JPEG bytes (needs the jbrd box).
pub fn reconstruct_jpeg(jxl: &[u8]) -> CResult<Vec<u8>> {
    let dec = decoder_builder().build().map_err(err)?;
    let (_m, data) = dec.reconstruct(jxl).map_err(err)?;
    match data {
        jpegxl_rs::decode::Data::Jpeg(b) => Ok(b),
        _ => Err("no jpeg reconstruction data".into()),
    }
}

/// ICC bytes for the colour space jxl-rs writes its output pixels in. `None` for sRGB-equivalent,
/// grayscale (replicated to RGB) or unrepresentable encodings.
fn output_icc(dec: &jxl::api::JxlDecoder) -> Option<Vec<u8>> {
    use jxl::api::JxlColorProfile;
    let icc = match dec.output_color_profile()? {
        JxlColorProfile::Icc(b) => b.clone(),
        JxlColorProfile::Simple(enc) => enc.maybe_create_profile().ok().flatten()?,
    };
    // gray / non-RGB profiles cannot describe the RGBA buffer we hand out
    let rgb = lcms2::Profile::new_icc(&icc).is_ok_and(|p| p.color_space() == lcms2::ColorSpaceSignature::RgbData);
    (rgb && !crate::color::is_srgb_equivalent(&icc)).then_some(icc)
}

/// Pure-Rust decode with jxl-rs (first frame, RGBA8). Pixels stay in the file's output colour space,
/// reported as `icc` unless it is sRGB; callers colour-convert (see `color::convert_to_srgb`).
pub fn decode(data: &[u8]) -> CResult<Decoded> {
    use jxl::api::{Event, JxlDecoder, JxlDecoderOptions, JxlOutputBuffer, JxlPixelFormat};
    let e = |x: jxl::error::Error| format!("jxl: {x:?}");
    use jxl::api::JxlAuxBoxType as BoxTy;
    let mut opts = JxlDecoderOptions::default();
    opts.request_aux_boxes = vec![BoxTy::EXIF, BoxTy::XMP];
    let mut dec = JxlDecoder::new(opts);
    let mut input: &[u8] = data;
    let mut px: Vec<u8> = Vec::new();
    let (mut w, mut h) = (0usize, 0usize);
    let mut icc = None;
    let mut still = true;
    let mut ready = false;
    loop {
        if !ready {
            match dec.process(&mut input, None, None).map_err(e)? {
                Event::BasicInfo => {
                    let bi = dec.basic_info().ok_or("jxl: no basic info")?;
                    (w, h) = bi.size;
                    still = bi.animation.is_none();
                    dec.set_pixel_format(JxlPixelFormat::rgba8(bi.extra_channels.len())).map_err(e)?;
                    icc = output_icc(&dec);
                    px = vec![0u8; w * h * 4];
                    ready = true;
                }
                Event::NeedMoreInput { .. } => return Err("jxl: truncated".into()),
                _ => {}
            }
        } else {
            let mut bufs = [JxlOutputBuffer::new(&mut px, h, w * 4)];
            match dec.process(&mut input, Some(&mut bufs), None).map_err(e)? {
                Event::FrameComplete { .. } | Event::Complete => break,
                Event::NeedMoreInput { .. } => return Err("jxl: truncated".into()),
                _ => {}
            }
        }
    }
    let mut out = Decoded::new(Rgba { w: w as u32, h: h as u32, px }, icc);
    if still {
        // Metadata boxes may follow the codestream: keep reading (best effort) until the file ends.
        dec.close_input();
        for _ in 0..16 {
            match dec.process(&mut input, None, None) {
                Ok(Event::Complete) | Err(_) => break,
                Ok(_) => {}
            }
        }
        let boxed = |t: BoxTy| dec.aux_boxes(t).first().and_then(|b| b.data(&[]).ok().map(|d| d.into_owned()));
        // The Exif box starts with a big-endian offset to the TIFF header.
        out.exif = boxed(BoxTy::EXIF).and_then(|b| {
            let off = u32::from_be_bytes(b.get(..4)?.try_into().ok()?) as usize;
            let t = b.get(4usize.checked_add(off)?..)?;
            Some(t.strip_prefix(b"Exif\0\0").unwrap_or(t).to_vec())
        });
        out.xmp = boxed(BoxTy::XMP);
    }
    Ok(out)
}

/// True when the codestream declares an animation (jxl-rs basic info).
pub fn is_animated(data: &[u8]) -> bool {
    use jxl::api::{Event, JxlDecoder, JxlDecoderOptions};
    let mut dec = JxlDecoder::new(JxlDecoderOptions::default());
    let mut input: &[u8] = data;
    loop {
        match dec.process(&mut input, None, None) {
            Ok(Event::BasicInfo) => return dec.basic_info().is_some_and(|b| b.animation.is_some()),
            Ok(Event::NeedMoreInput { .. }) | Err(_) => return false,
            Ok(_) => {}
        }
    }
}
