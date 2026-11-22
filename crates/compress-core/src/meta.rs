//! Metadata policy: GPS removal, preview stripping, orientation reset, compact (sRGB) ICC.
//!
//! EXIF is edited *in place* inside the TIFF structure: entries are removed or neutralised and the
//! vacated bytes zeroed, but no data is moved, so every other offset (MakerNote, thumbnails of other
//! IFDs, ...) stays valid and nothing is re-serialised (little_exif would drop unknown tags).
use crate::options::{Metadata, Options};
use img_parts::{
    jpeg::{Jpeg, JpegSegment},
    png::Png,
    Bytes, ImageEXIF, ImageICC,
};

// ---------------------------------------------------------------------------------------------
// EXIF (TIFF) in-place editing
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy, Default, Debug)]
pub struct ExifEdit {
    pub strip_gps: bool,
    /// Remove IFD1 (the embedded thumbnail) and its pixel data.
    pub strip_thumbnail: bool,
    /// Set IFD0 Orientation to 1 (pixels were rotated by a re-encode).
    pub reset_orientation: bool,
}

struct Tiff<'a> {
    b: &'a mut Vec<u8>,
    le: bool,
    zeroed: Vec<(usize, usize)>,
}

fn type_size(t: u16) -> usize {
    match t {
        1 | 2 | 6 | 7 => 1,
        3 | 8 => 2,
        4 | 9 | 11 | 13 => 4,
        5 | 10 | 12 => 8,
        _ => 1,
    }
}

impl Tiff<'_> {
    fn u16(&self, o: usize) -> Option<u16> {
        let s: [u8; 2] = self.b.get(o..o + 2)?.try_into().ok()?;
        Some(if self.le { u16::from_le_bytes(s) } else { u16::from_be_bytes(s) })
    }
    fn u32(&self, o: usize) -> Option<u32> {
        let s: [u8; 4] = self.b.get(o..o + 4)?.try_into().ok()?;
        Some(if self.le { u32::from_le_bytes(s) } else { u32::from_be_bytes(s) })
    }
    fn put_u16(&mut self, o: usize, v: u16) {
        let s = if self.le { v.to_le_bytes() } else { v.to_be_bytes() };
        self.b[o..o + 2].copy_from_slice(&s);
    }
    fn zero(&mut self, start: usize, len: usize) -> Option<()> {
        let end = start.checked_add(len)?;
        self.b.get(start..end)?;
        self.b[start..end].fill(0);
        self.zeroed.push((start, end));
        Some(())
    }
    /// (entry offset, tag, type, count, value-field offset) for each entry; ifd offset -> entries.
    fn entries(&self, ifd: usize) -> Option<Vec<(usize, u16, u16, u32)>> {
        let n = self.u16(ifd)? as usize;
        self.b.get(ifd..ifd + 2 + n * 12 + 4)?;
        (0..n)
            .map(|i| {
                let e = ifd + 2 + i * 12;
                Some((e, self.u16(e)?, self.u16(e + 2)?, self.u32(e + 4)?))
            })
            .collect()
    }
    /// Zero an IFD, the out-of-line values of its entries, and (optionally) the data its
    /// offset-valued tags point at (thumbnail bytes). Does not follow the next-IFD link.
    fn wipe_ifd(&mut self, ifd: usize, thumb: bool) -> Option<()> {
        let ents = self.entries(ifd)?;
        let mut thumb_off = None;
        let mut thumb_len = None;
        for &(e, tag, ty, count) in &ents {
            let bytes = (count as usize).checked_mul(type_size(ty))?;
            if bytes > 4 {
                let off = self.u32(e + 8)? as usize;
                self.zero(off, bytes)?;
            }
            match tag {
                0x0201 => thumb_off = self.u32(e + 8),
                0x0202 => thumb_len = self.u32(e + 8),
                _ => {}
            }
        }
        if thumb {
            if let (Some(o), Some(l)) = (thumb_off, thumb_len) {
                self.zero(o as usize, l as usize)?;
            }
        }
        self.zero(ifd, 2 + ents.len() * 12 + 4)
    }
    /// Drop entry `idx` from the IFD at `ifd` by shifting the later entries (and the next-IFD
    /// pointer) up by 12 bytes and zeroing the freed tail.
    fn remove_entry(&mut self, ifd: usize, idx: usize) -> Option<()> {
        let n = self.u16(ifd)? as usize;
        if idx >= n {
            return None;
        }
        let start = ifd + 2 + idx * 12;
        let end = ifd + 2 + n * 12 + 4;
        self.b.copy_within(start + 12..end, start);
        self.b[end - 12..end].fill(0);
        self.put_u16(ifd, (n - 1) as u16);
        Some(())
    }
}

/// Edit an EXIF payload (TIFF header onward, without the `Exif\0\0` prefix). `None` means the
/// structure could not be parsed or edited safely; callers must then drop EXIF entirely.
pub fn edit_exif(tiff: &[u8], edit: ExifEdit) -> Option<Vec<u8>> {
    let mut buf = tiff.to_vec();
    let le = match buf.get(0..2)? {
        b"II" => true,
        b"MM" => false,
        _ => return None,
    };
    let mut t = Tiff { b: &mut buf, le, zeroed: vec![] };
    if t.u16(2)? != 42 {
        return None;
    }
    let ifd0 = t.u32(4)? as usize;
    if ifd0 == 0 {
        return None;
    }
    if edit.strip_gps {
        // GPS IFD pointer lives in IFD0 (0x8825)
        if let Some(i) = t.entries(ifd0)?.iter().position(|e| e.1 == 0x8825) {
            let ents = t.entries(ifd0)?;
            let gps = t.u32(ents[i].0 + 8)? as usize;
            // GPS IFD may itself be malformed: bail (caller drops EXIF) rather than leave it behind.
            t.wipe_ifd(gps, false)?;
            t.remove_entry(ifd0, i)?;
        }
    }
    if edit.strip_thumbnail {
        let n = t.u16(ifd0)? as usize;
        let next_at = ifd0 + 2 + n * 12;
        let ifd1 = t.u32(next_at)? as usize;
        if ifd1 != 0 {
            t.wipe_ifd(ifd1, true)?;
            t.b[next_at..next_at + 4].fill(0);
        }
    }
    if edit.reset_orientation {
        let ents = t.entries(ifd0)?;
        if let Some(&(e, _, ty, count)) = ents.iter().find(|e| e.1 == 0x0112) {
            if ty == 3 && count == 1 {
                t.put_u16(e + 8, 1);
            }
        }
    }
    // Trim zeroed regions that end the buffer (the thumbnail usually does).
    let mut zr = std::mem::take(&mut t.zeroed);
    zr.sort_by_key(|r| std::cmp::Reverse(r.1));
    let mut len = t.b.len();
    // A single trailing zero pad byte after the last wiped region (EXIF segments are even-sized) is
    // not a live value: values stored out of line are > 4 bytes and never overlap a wiped region.
    if len > 1 && t.b[len - 1] == 0 && zr.iter().any(|r| r.1 == len - 1) {
        len -= 1;
    }
    for (s, e) in zr {
        if e >= len && s < len {
            len = s;
        }
    }
    buf.truncate(len.max(8));
    Some(buf)
}

/// The GPS IFD pointer (0x8825) in IFD0, if present. Used by tests / sanity checks.
pub fn exif_has_gps(tiff: &[u8]) -> bool {
    let mut buf = tiff.to_vec();
    let le = match buf.get(0..2) {
        Some(b"II") => true,
        Some(b"MM") => false,
        _ => return false,
    };
    let t = Tiff { b: &mut buf, le, zeroed: vec![] };
    let Some(ifd0) = t.u32(4) else { return false };
    t.entries(ifd0 as usize).is_some_and(|e| e.iter().any(|x| x.1 == 0x8825))
}

// ---------------------------------------------------------------------------------------------
// JPEG
// ---------------------------------------------------------------------------------------------

pub fn jpeg_icc(data: &[u8]) -> Option<Vec<u8>> {
    Jpeg::from_bytes(Bytes::copy_from_slice(data)).ok()?.icc_profile().map(|b| b.to_vec())
}

fn is_mpf(s: &JpegSegment) -> bool {
    s.marker() == 0xE2 && s.contents().starts_with(b"MPF\0")
}

fn is_xmp(s: &JpegSegment) -> bool {
    s.marker() == 0xE1 && (s.contents().starts_with(b"http://ns.adobe.com/xap/1.0/\0") || s.contents().starts_with(b"http://ns.adobe.com/xmp/extension/\0"))
}

fn xmp_mentions_gps(s: &JpegSegment) -> bool {
    s.contents().windows(6).any(|w| w == b"exif:G" || w == b"GPSLat" || w == b"GPSLon") || s.contents().starts_with(b"http://ns.adobe.com/xmp/extension/\0")
}

fn insert_after_app_run(j: &mut Jpeg, seg: JpegSegment) {
    // after SOI and the leading APPn run (JFIF/Exif/...), before DQT/SOF
    let segs = j.segments_mut();
    let mut at = 0;
    for (i, s) in segs.iter().enumerate() {
        if s.marker() == 0xD8 || (0xE0..=0xEF).contains(&s.marker()) {
            at = i + 1;
        } else {
            break;
        }
    }
    segs.insert(at, seg);
}

fn set_exif_segment(j: &mut Jpeg, tiff: Option<Vec<u8>>) {
    j.segments_mut().retain(|s| !(s.marker() == 0xE1 && s.contents().starts_with(b"Exif\0\0")));
    if let Some(t) = tiff {
        let mut c = b"Exif\0\0".to_vec();
        c.extend_from_slice(&t);
        if c.len() <= 65533 {
            insert_after_app_run(j, JpegSegment::new_with_contents(0xE1, Bytes::from(c)));
        }
    }
}

/// Apply the metadata policy to a finished JPEG. `src` is the original file, `out` the produced one.
/// `reencoded`: pixels went through a decode (orientation applied to the pixels, EXIF not carried by
/// the encoder). Returns `out` untouched when the policy requires nothing.
pub fn jpeg_post(src: &[u8], out: Vec<u8>, o: &Options, reencoded: bool) -> Vec<u8> {
    let privacy = o.metadata == Metadata::ExifNoGps;
    let strip_prev = o.strip_previews || privacy;
    let keeps_icc = o.metadata != Metadata::None;
    let wants_exif = matches!(o.metadata, Metadata::ExifNoGps) || (o.metadata == Metadata::All && reencoded);
    let tweak_existing = o.metadata == Metadata::All && !reencoded && strip_prev;
    let compact = o.compact_icc && keeps_icc;
    if !(wants_exif || tweak_existing || strip_prev || compact) {
        return out;
    }
    let Ok(mut j) = Jpeg::from_bytes(Bytes::from(out.clone())) else { return out };
    if compact {
        if let Some(icc) = j.icc_profile() {
            if crate::color::is_srgb_equivalent(&icc) {
                j.set_icc_profile(None);
            }
        }
    }
    if wants_exif {
        let edit = ExifEdit { strip_gps: privacy, strip_thumbnail: strip_prev, reset_orientation: reencoded };
        let new = Jpeg::from_bytes(Bytes::copy_from_slice(src)).ok().and_then(|s| s.exif()).and_then(|e| edit_exif(&e, edit));
        // privacy: if the EXIF cannot be edited safely it is dropped, never copied verbatim
        set_exif_segment(&mut j, new);
        if privacy {
            // keep XMP unless it may carry location data
            if let Ok(s) = Jpeg::from_bytes(Bytes::copy_from_slice(src)) {
                j.segments_mut().retain(|x| !is_xmp(x));
                let keep: Vec<_> = s.segments().iter().filter(|x| is_xmp(x) && !xmp_mentions_gps(x)).cloned().collect();
                for k in keep {
                    insert_after_app_run(&mut j, k);
                }
            }
        }
    } else if tweak_existing {
        if let Some(e) = j.exif() {
            let new = edit_exif(&e, ExifEdit { strip_thumbnail: true, ..Default::default() });
            set_exif_segment(&mut j, new.or(Some(e.to_vec())));
        }
    }
    if strip_prev {
        j.segments_mut().retain(|s| s.marker() != 0xED && !is_mpf(s));
    }
    let mut v = Vec::with_capacity(out.len());
    if j.encoder().write_to(&mut v).is_err() {
        return out;
    }
    v
}

// ---------------------------------------------------------------------------------------------
// PNG
// ---------------------------------------------------------------------------------------------

/// oxipng chunk policy for the metadata setting.
///
/// An APNG's `acTL`/`fcTL`/`fdAT` chunks are its frames, not metadata: `animated` makes sure no mode drops them.
pub fn png_strip(m: Metadata, animated: bool) -> oxipng::StripChunks {
    use oxipng::StripChunks;
    let set = |names: &[&[u8; 4]]| names.iter().map(|n| **n).collect();
    match m {
        Metadata::None if animated => StripChunks::Keep(set(&[b"acTL", b"fcTL", b"fdAT"])),
        Metadata::None => StripChunks::All,
        Metadata::Icc => StripChunks::Safe,
        Metadata::All => StripChunks::None,
        Metadata::ExifNoGps => StripChunks::Keep(set(&[b"iCCP", b"sRGB", b"cICP", b"pHYs", b"acTL", b"fcTL", b"fdAT", b"eXIf"])),
    }
}

/// Post-pass on a finished PNG: GPS removal from eXIf, drop sRGB-equivalent iCCP.
pub fn png_post(out: Vec<u8>, o: &Options) -> Vec<u8> {
    let has = |k: &[u8; 4]| out.windows(4).any(|w| w == k);
    let compact = o.compact_icc && o.metadata != Metadata::None && has(b"iCCP");
    let privacy = o.metadata == Metadata::ExifNoGps && has(b"eXIf");
    if !(compact || privacy) {
        return out;
    }
    let Ok(mut p) = Png::from_bytes(Bytes::from(out.clone())) else { return out };
    if compact {
        if let Some(icc) = p.icc_profile() {
            if crate::color::is_srgb_equivalent(&icc) {
                p.set_icc_profile(None);
            }
        }
    }
    if privacy {
        let new = p.exif().and_then(|e| {
            // PNG eXIf has no "Exif\0\0" prefix
            edit_exif(&e, ExifEdit { strip_gps: true, strip_thumbnail: true, reset_orientation: false })
        });
        p.set_exif(new.map(Bytes::from));
    }
    let mut v = Vec::with_capacity(out.len());
    if p.encoder().write_to(&mut v).is_err() {
        return out;
    }
    v
}

// ---------------------------------------------------------------------------------------------
// Carrying EXIF / XMP across a format conversion
// ---------------------------------------------------------------------------------------------

/// EXIF (raw TIFF, no `Exif\0\0`) and XMP of a source file, for the formats we parse here.
/// AVIF and JXL fill `Decoded` themselves; HEIC sources are not extracted.
pub fn extract(data: &[u8], fmt: crate::convert::Fmt) -> (Option<Vec<u8>>, Option<Vec<u8>>) {
    use crate::convert::Fmt;
    match fmt {
        Fmt::Jpeg => {
            let Ok(j) = Jpeg::from_bytes(Bytes::copy_from_slice(data)) else { return (None, None) };
            const XAP: &[u8] = b"http://ns.adobe.com/xap/1.0/\0";
            let xmp = j.segments().iter().find(|s| s.marker() == 0xE1 && s.contents().starts_with(XAP)).map(|s| s.contents()[XAP.len()..].to_vec());
            (j.exif().map(|b| b.to_vec()), xmp)
        }
        Fmt::Png => {
            let Ok(p) = Png::from_bytes(Bytes::copy_from_slice(data)) else { return (None, None) };
            let xmp = p.chunks().iter().filter(|c| c.kind() == *b"iTXt").find_map(|c| png_itxt_xmp(c.contents()));
            (p.exif().map(|b| b.to_vec()), xmp)
        }
        Fmt::Webp => crate::webp_codec::exif_xmp_of(data),
        _ => (None, None),
    }
}

/// Text of an uncompressed `iTXt` chunk whose keyword is `XML:com.adobe.xmp`.
fn png_itxt_xmp(c: &[u8]) -> Option<Vec<u8>> {
    let rest = c.strip_prefix(b"XML:com.adobe.xmp\0")?;
    let (&compressed, rest) = rest.split_first()?;
    let rest = rest.get(1..)?; // compression method
    if compressed != 0 {
        return None;
    }
    let rest = &rest[rest.iter().position(|&b| b == 0)? + 1..]; // language tag
    let rest = &rest[rest.iter().position(|&b| b == 0)? + 1..]; // translated keyword
    Some(rest.to_vec())
}

fn xmp_has_gps(x: &[u8]) -> bool {
    x.windows(6).any(|w| w == b"exif:G" || w == b"GPSLat" || w == b"GPSLon")
}

/// XMP can restate the orientation; the pixels are already upright, so neutralise it.
fn xmp_reset_orientation(x: &[u8]) -> Vec<u8> {
    let mut s = x.to_vec();
    let mut i = 0;
    while let Some(p) = s[i..].windows(16).position(|w| w == b"tiff:Orientation") {
        let at = i + p + 16;
        // attribute form: tiff:Orientation="6"   element form: <tiff:Orientation>6</...>
        let digit = match (s.get(at), s.get(at + 1), s.get(at + 2)) {
            (Some(b'='), Some(b'"' | b'\''), Some(_)) => at + 2,
            (Some(b'>'), Some(_), _) => at + 1,
            _ => {
                i = at;
                continue;
            }
        };
        if s[digit].is_ascii_digit() && !s.get(digit + 1).is_some_and(|b| b.is_ascii_digit()) {
            s[digit] = b'1';
        }
        i = digit;
    }
    s
}

/// EXIF/XMP to write into a converted file, after applying the metadata policy.
#[derive(Debug, Default, Clone)]
pub struct Carried {
    pub exif: Option<Vec<u8>>,
    pub xmp: Option<Vec<u8>>,
}

/// Largest XMP packet carried into a converted file (extended XMP is JPEG-only and not carried).
const MAX_XMP: usize = 65_000;

/// Apply `o.metadata` to the EXIF/XMP found in the source. Pixels were decoded upright, so orientation
/// is reset to 1. EXIF that cannot be edited safely is dropped, never copied verbatim.
pub fn carry(d: &crate::pixels::Decoded, o: &Options) -> Carried {
    let privacy = o.metadata == Metadata::ExifNoGps;
    if !privacy && o.metadata != Metadata::All {
        return Carried::default();
    }
    let edit = ExifEdit { strip_gps: privacy, strip_thumbnail: privacy || o.strip_previews, reset_orientation: true };
    Carried {
        exif: d.exif.as_deref().and_then(|e| edit_exif(e, edit)),
        xmp: d.xmp.as_deref().filter(|x| x.len() <= MAX_XMP && !(privacy && xmp_has_gps(x))).map(xmp_reset_orientation),
    }
}

/// A 1x1 JPEG carrying `c`'s EXIF and XMP, for APIs that only accept metadata via an image (ImageIO).
pub fn carrier_jpeg(c: &Carried) -> Option<Vec<u8>> {
    let base = crate::jpeg_codec::encode_rgb8(&[128, 128, 128], 1, 1, &Default::default()).ok()?;
    let mut j = Jpeg::from_bytes(Bytes::from(base)).ok()?;
    set_exif_segment(&mut j, c.exif.clone());
    if let Some(x) = &c.xmp {
        let mut b = b"http://ns.adobe.com/xap/1.0/\0".to_vec();
        b.extend_from_slice(x);
        if b.len() <= 65533 {
            insert_after_app_run(&mut j, JpegSegment::new_with_contents(0xE1, Bytes::from(b)));
        }
    }
    let mut v = Vec::new();
    j.encoder().write_to(&mut v).ok()?;
    Some(v)
}

/// Orientation (1..8) stored in an EXIF TIFF payload.
pub fn exif_orientation(tiff: &[u8]) -> Option<image::metadata::Orientation> {
    image::metadata::Orientation::from_exif_chunk(tiff)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(n: &str) -> Vec<u8> {
        std::fs::read(format!("{}/../../fixtures/{n}", env!("CARGO_MANIFEST_DIR"))).unwrap()
    }

    fn exif_of(j: &[u8]) -> Option<Vec<u8>> {
        Jpeg::from_bytes(Bytes::copy_from_slice(j)).unwrap().exif().map(|b| b.to_vec())
    }

    #[test]
    fn gps_removed_other_tags_kept() {
        let src = fixture("jpeg/exif_gps_orient6.jpg");
        let e = exif_of(&src).unwrap();
        assert!(exif_has_gps(&e));
        let ne = edit_exif(&e, ExifEdit { strip_gps: true, ..Default::default() }).unwrap();
        assert!(!exif_has_gps(&ne));
        // Make and orientation survive and the bytes of the GPS rational are gone
        assert!(ne.windows(10).any(|w| w == b"FixtureCam"));
        let o = ExifEdit { strip_gps: true, strip_thumbnail: true, reset_orientation: true };
        let ne2 = edit_exif(&e, o).unwrap();
        assert!(ne2.len() < ne.len(), "thumbnail trimmed: {} vs {}", ne2.len(), ne.len());
    }

    #[test]
    fn garbage_exif_is_rejected() {
        assert!(edit_exif(b"nonsense", ExifEdit { strip_gps: true, ..Default::default() }).is_none());
        assert!(edit_exif(b"II*\0\xff\xff\xff\xff", ExifEdit { strip_gps: true, ..Default::default() }).is_none());
    }

    #[test]
    fn jpeg_post_exif_no_gps_roundtrips_through_decoder() {
        let src = fixture("jpeg/exif_gps_orient6.jpg");
        let mut o = Options::default();
        o.metadata = Metadata::ExifNoGps;
        // pretend the encoder produced a bare re-encode of the same pixels (no EXIF)
        let bare = {
            let mut j = Jpeg::from_bytes(Bytes::from(src.clone())).unwrap();
            set_exif_segment(&mut j, None);
            let mut v = vec![];
            j.encoder().write_to(&mut v).unwrap();
            v
        };
        assert!(exif_of(&bare).is_none());
        let out = jpeg_post(&src, bare, &o, false);
        let e = exif_of(&out).expect("exif re-added");
        assert!(!exif_has_gps(&e));
        image::load_from_memory(&out).unwrap();
        // orientation kept when pixels were not rotated, reset when re-encoded
        let ori = |t: &[u8]| {
            let mut b = t.to_vec();
            let le = &b[0..2] == b"II";
            let t = Tiff { b: &mut b, le, zeroed: vec![] };
            let ifd0 = t.u32(4).unwrap() as usize;
            let (e, ..) = t.entries(ifd0).unwrap().into_iter().find(|x| x.1 == 0x112).unwrap();
            t.u16(e + 8).unwrap()
        };
        assert_eq!(ori(&e), 6);
        let out2 = jpeg_post(&src, Jpeg::from_bytes(Bytes::from(out)).map(|j| { let mut v = vec![]; j.encoder().write_to(&mut v).unwrap(); v }).unwrap(), &o, true);
        assert_eq!(ori(&exif_of(&out2).unwrap()), 1);
    }

    #[test]
    fn xmp_orientation_is_neutralised_and_gps_detected() {
        let attr = br#"<rdf:Description tiff:Orientation="6" tiff:Make="x"/>"#;
        assert_eq!(xmp_reset_orientation(attr), br#"<rdf:Description tiff:Orientation="1" tiff:Make="x"/>"#.to_vec());
        assert_eq!(xmp_reset_orientation(b"<tiff:Orientation>8</tiff:Orientation>"), b"<tiff:Orientation>1</tiff:Orientation>".to_vec());
        assert_eq!(xmp_reset_orientation(b"<x:y>untouched</x:y>"), b"<x:y>untouched</x:y>".to_vec());
        assert!(xmp_has_gps(b"<exif:GPSLatitude>1</exif:GPSLatitude>"));
        assert!(!xmp_has_gps(b"<dc:title>t</dc:title>"));
    }

    #[test]
    fn extract_and_carry_policy() {
        let src = fixture("jpeg/exif_gps_orient6.jpg");
        let (exif, xmp) = extract(&src, crate::convert::Fmt::Jpeg);
        assert!(exif.as_deref().is_some_and(exif_has_gps) && xmp.is_none());
        let mut d = crate::pixels::Decoded::new(crate::pixels::Rgba { w: 1, h: 1, px: vec![0; 4] }, None);
        d.exif = exif;
        d.xmp = Some(b"<dc:title>t</dc:title>".to_vec());
        let mut o = Options::default();
        for (m, exif, gps, xmp) in [(Metadata::None, false, false, false), (Metadata::Icc, false, false, false), (Metadata::ExifNoGps, true, false, true), (Metadata::All, true, true, true)] {
            o.metadata = m;
            let c = carry(&d, &o);
            assert_eq!((c.exif.is_some(), c.exif.as_deref().is_some_and(exif_has_gps), c.xmp.is_some()), (exif, gps, xmp), "{m:?}");
        }
        // GPS in the XMP drops the packet under the privacy policy
        d.xmp = Some(b"<exif:GPSLatitude>1</exif:GPSLatitude>".to_vec());
        o.metadata = Metadata::ExifNoGps;
        assert!(carry(&d, &o).xmp.is_none());
        o.metadata = Metadata::All;
        assert!(carry(&d, &o).xmp.is_some());
        // a carrier JPEG round-trips both blobs
        let c = carry(&d, &o);
        let carrier = carrier_jpeg(&c).unwrap();
        let (e2, x2) = extract(&carrier, crate::convert::Fmt::Jpeg);
        assert!(e2.is_some() && x2 == c.xmp);
    }

    #[test]
    fn png_policy_maps() {
        assert!(matches!(png_strip(Metadata::Icc, false), oxipng::StripChunks::Safe));
        assert!(matches!(png_strip(Metadata::None, false), oxipng::StripChunks::All));
        assert!(matches!(png_strip(Metadata::All, false), oxipng::StripChunks::None));
        // animation chunks are never "metadata"
        assert!(matches!(png_strip(Metadata::None, true), oxipng::StripChunks::Keep(k) if k.len() == 3));
    }
}
