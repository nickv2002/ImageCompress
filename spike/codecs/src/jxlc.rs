use crate::Rgba;
use anyhow::{anyhow, Result};
use jpegxl_rs::{
    decode::decoder_builder,
    encode::{encoder_builder, EncoderFrame, EncoderSpeed},
};

fn speed(effort: u32) -> EncoderSpeed {
    use EncoderSpeed::*;
    match effort {
        1 => Lightning, 2 => Thunder, 3 => Falcon, 4 => Cheetah, 5 => Hare,
        6 => Wombat, 7 => Squirrel, 8 => Kitten, 9 => Tortoise, _ => Glacier,
    }
}

/// distance: butteraugli distance (cjxl -d). 1.0 ~ visually lossless.
pub fn encode_lossy(im: &Rgba, distance: f32, effort: u32) -> Result<Vec<u8>> {
    let alpha = crate::has_alpha(im);
    let mut enc = encoder_builder()
        .has_alpha(alpha)
        .lossless(false)
        .quality(distance)
        .speed(speed(effort))
        .build()?;
    frame_encode(&mut enc, im, alpha)
}

fn frame_encode(enc: &mut jpegxl_rs::encode::JxlEncoder, im: &Rgba, alpha: bool) -> Result<Vec<u8>> {
    // has_alpha(true) <=> 4 channels; opaque input must be passed as 3 channels (libjxl API misuse error otherwise).
    if alpha {
        let frame = EncoderFrame::new(&im.px).num_channels(4);
        Ok(enc.encode_frame(&frame, im.w, im.h)?)
    } else {
        let rgb = crate::rgb3(im);
        let frame = EncoderFrame::new(&rgb).num_channels(3);
        Ok(enc.encode_frame(&frame, im.w, im.h)?)
    }
}

pub fn encode_lossless(im: &Rgba, effort: u32) -> Result<Vec<u8>> {
    let alpha = crate::has_alpha(im);
    let mut enc = encoder_builder().has_alpha(alpha).lossless(true).uses_original_profile(true).speed(speed(effort)).build()?;
    frame_encode(&mut enc, im, alpha)
}

/// Reversible JPEG recompression (cjxl on a .jpg default).
pub fn transcode_jpeg(jpeg: &[u8]) -> Result<Vec<u8>> {
    let mut enc = encoder_builder().use_container(true).uses_original_profile(true).speed(EncoderSpeed::Squirrel).build()?;
    Ok(enc.encode_jpeg(jpeg)?)
}

/// JXL -> original JPEG bytes (needs the jbrd box).
pub fn reconstruct_jpeg(jxl: &[u8]) -> Result<Vec<u8>> {
    let dec = decoder_builder().build()?;
    let (_m, data) = dec.reconstruct(jxl)?;
    match data {
        jpegxl_rs::decode::Data::Jpeg(b) => Ok(b),
        _ => Err(anyhow!("no jpeg reconstruction data")),
    }
}

pub fn decode_libjxl(jxl: &[u8]) -> Result<Rgba> {
    let dec = decoder_builder().pixel_format(jpegxl_rs::decode::PixelFormat { num_channels: 4, ..Default::default() }).build()?;
    let (meta, px) = dec.decode_with::<u8>(jxl)?;
    Ok(Rgba { w: meta.width, h: meta.height, px })
}

/// Pure-Rust decode with jxl-rs (first frame, RGBA8; colour managed to sRGB by default).
pub fn decode_jxl_rs(data: &[u8]) -> Result<Rgba> {
    use jxl::api::{Event, JxlDecoder, JxlDecoderOptions, JxlOutputBuffer, JxlPixelFormat};
    let mut dec = JxlDecoder::new(JxlDecoderOptions::default());
    let mut input: &[u8] = data;
    let mut px: Vec<u8> = Vec::new();
    let (mut w, mut h) = (0usize, 0usize);
    let mut ready = false;
    loop {
        if !ready {
            match dec.process(&mut input, None, None).map_err(|e| anyhow!("{e:?}"))? {
                Event::BasicInfo => {
                    let bi = dec.basic_info().unwrap();
                    (w, h) = bi.size;
                    dec.set_pixel_format(JxlPixelFormat::rgba8(bi.extra_channels.len())).map_err(|e| anyhow!("{e:?}"))?;
                    px = vec![0u8; w * h * 4];
                    ready = true;
                }
                Event::NeedMoreInput { .. } => return Err(anyhow!("truncated")),
                _ => {}
            }
        } else {
            let mut bufs = [JxlOutputBuffer::new(&mut px, h, w * 4)];
            match dec.process(&mut input, Some(&mut bufs), None).map_err(|e| anyhow!("{e:?}"))? {
                Event::FrameComplete { .. } | Event::Complete => break,
                Event::NeedMoreInput { .. } => return Err(anyhow!("truncated")),
                _ => {}
            }
        }
    }
    Ok(Rgba { w: w as u32, h: h as u32, px })
}
