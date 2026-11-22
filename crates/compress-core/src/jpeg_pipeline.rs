use crate::jpeg_codec::*;
use crate::options::{JpegEncoder, Metadata, Options};
use image::ImageDecoder;
use std::io::Cursor;

/// cjpegli's quality -> butteraugli distance mapping.
pub fn quality_to_distance(q: u8) -> f32 {
    let q = q as f32;
    if q >= 100.0 {
        0.0
    } else if q >= 30.0 {
        0.1 + (100.0 - q) * 0.09
    } else {
        53.0 / 3000.0 * q * q - 23.0 / 20.0 * q + 25.0
    }
}

pub fn process(data: &[u8], o: &Options, stage: &dyn Fn(&str)) -> std::result::Result<(Vec<u8>, String), String> {
    let markers = match o.metadata {
        Metadata::None => MarkerPolicy::None,
        // exif-no-gps: libjpeg copies only ICC; meta::jpeg_post re-adds EXIF with GPS removed
        Metadata::Icc | Metadata::ExifNoGps => MarkerPolicy::Icc,
        Metadata::All => MarkerPolicy::All,
    };
    let Some(q) = o.jpeg_quality else {
        stage("jpegtran");
        let mut best: Option<Vec<u8>> = None;
        for progressive in [true, false] {
            let b = optimize(data, &LosslessOpts { progressive, markers })?;
            if best.as_ref().is_none_or(|x| b.len() < x.len()) {
                best = Some(b);
            }
        }
        return Ok((best.unwrap(), "jpegtran-lossless".into()));
    };
    stage("decode");
    let mut dec = image::ImageReader::with_format(Cursor::new(data), image::ImageFormat::Jpeg)
        .into_decoder()
        .map_err(|e| e.to_string())?;
    let icc = if o.metadata == Metadata::None { None } else { dec.icc_profile().map_err(|e| e.to_string())? };
    let orient = dec.orientation().map_err(|e| e.to_string())?;
    let mut img = image::DynamicImage::from_decoder(dec).map_err(|e| e.to_string())?;
    img.apply_orientation(orient);
    let rgb = img.to_rgb8();
    let (w, h) = rgb.dimensions();
    stage("encode");
    let pick = |auto: Subsampling| match o.jpeg_subsampling {
        crate::options::Subsampling::Auto => auto,
        crate::options::Subsampling::S444 => Subsampling::S444,
        crate::options::Subsampling::S422 => Subsampling::S422,
        crate::options::Subsampling::S420 => Subsampling::S420,
    };
    match o.jpeg_encoder {
        JpegEncoder::Jpegli => {
            let b = encode_rgb8_jpegli(&rgb, w, h, &JpegliOpts { distance: quality_to_distance(q), quality: Some(q), progressive_level: if o.jpeg_progressive { 2 } else { 0 }, subsampling: pick(Subsampling::S444), icc })?;
            Ok((b, "jpegli".into()))
        }
        JpegEncoder::Mozjpeg => {
            let b = encode_rgb8(&rgb, w, h, &LossyOpts { quality: q, progressive: o.jpeg_progressive, subsampling: pick(if q >= 90 { Subsampling::S444 } else { Subsampling::S420 }), icc, ..Default::default() })?;
            Ok((b, "mozjpeg".into()))
        }
    }
}
