//! mozjpeg-backed JPEG helpers: lossless (jpegtran-equivalent) optimise and lossy RGB8 encode.
//! bytes -> bytes, no temp files. Errors from libjpeg are turned into `Err(String)` via panic
//! unwinding out of a `C-unwind` error_exit (mozjpeg-sys default `unwinding` feature).
use mozjpeg_sys::*;
use std::os::raw::{c_int, c_ulong, c_void};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::{mem, ptr, slice};

extern "C" {
    fn free(p: *mut c_void);
}

pub type Result<T> = std::result::Result<T, String>;

/// Which ancillary segments to carry over in a lossless pass. JFIF (APP0) / Adobe (APP14)
/// are always preserved by libjpeg itself via copy_critical_parameters.
#[derive(Clone, Copy)]
pub enum MarkerPolicy<'a> {
    /// jpegtran `-copy none`
    None,
    /// Only ICC profile chunks (APP2 "ICC_PROFILE\0")
    Icc,
    /// jpegtran `-copy all` (APP1..APP13, APP15, COM)
    All,
    /// Caller decides per segment: (marker_code e.g. 0xE1, payload) -> keep? Use for "all except GPS"
    /// (e.g. rewrite the EXIF blob before/after with little_exif/img-parts, or filter here).
    Filter(&'a dyn Fn(u8, &[u8]) -> bool),
}

pub struct LosslessOpts<'a> {
    pub progressive: bool,
    pub markers: MarkerPolicy<'a>,
}

unsafe extern "C-unwind" fn error_exit(cinfo: &mut jpeg_common_struct) {
    let mut buf = [0u8; 80];
    if let Some(f) = (*cinfo.err).format_message {
        f(cinfo, &buf);
        let _ = &mut buf;
    }
    let n = buf.iter().position(|&b| b == 0).unwrap_or(80);
    std::panic::resume_unwind(Box::new(String::from_utf8_lossy(&buf[..n]).into_owned()));
}
unsafe extern "C-unwind" fn silent(_: &mut jpeg_common_struct) {}
unsafe extern "C-unwind" fn emit(_: &mut jpeg_common_struct, _: c_int) {}

fn setup_err(e: &mut jpeg_error_mgr) {
    unsafe {
        jpeg_std_error(e);
    }
    e.error_exit = Some(error_exit);
    e.output_message = Some(silent);
    e.emit_message = Some(emit);
}

struct Dec(Box<jpeg_decompress_struct>);
impl Drop for Dec {
    fn drop(&mut self) {
        unsafe { jpeg_destroy_decompress(&mut self.0) }
    }
}
struct Enc(Box<jpeg_compress_struct>);
impl Drop for Enc {
    fn drop(&mut self) {
        unsafe { jpeg_destroy_compress(&mut self.0) }
    }
}
struct OutBuf(*mut u8, c_ulong);
impl Drop for OutBuf {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { free(self.0 as *mut c_void) }
        }
    }
}

fn catch<T>(f: impl FnOnce() -> T) -> Result<T> {
    // Silence the default panic message for our controlled unwinds.
    catch_unwind(AssertUnwindSafe(f)).map_err(|p| {
        p.downcast::<String>().map(|s| *s).unwrap_or_else(|_| "libjpeg error".into())
    })
}

fn wanted(policy: &MarkerPolicy, marker: u8, data: &[u8]) -> bool {
    match policy {
        MarkerPolicy::None => false,
        MarkerPolicy::Icc => marker == 0xE2 && data.starts_with(b"ICC_PROFILE\0"),
        MarkerPolicy::All => true,
        MarkerPolicy::Filter(f) => f(marker, data),
    }
}

/// jpegtran `-optimize [-progressive]` equivalent: coefficient copy, optimal Huffman tables.
pub fn optimize(input: &[u8], opts: &LosslessOpts) -> Result<Vec<u8>> {
    let mut serr: jpeg_error_mgr = unsafe { mem::zeroed() };
    let mut derr: jpeg_error_mgr = unsafe { mem::zeroed() };
    setup_err(&mut serr);
    setup_err(&mut derr);
    let mut out = OutBuf(ptr::null_mut(), 0);
    catch(|| unsafe {
        let mut src = Dec(Box::new(mem::zeroed()));
        src.0.common.err = &mut serr;
        jpeg_create_decompress(&mut *src.0);
        jpeg_mem_src(&mut src.0, input.as_ptr(), input.len() as c_ulong);
        if !matches!(opts.markers, MarkerPolicy::None) {
            for m in 0xE1..=0xEFu8 {
                if m != 0xEE {
                    jpeg_save_markers(&mut src.0, m as c_int, 0xFFFF);
                }
            }
            jpeg_save_markers(&mut src.0, 0xFE, 0xFFFF);
        }
        jpeg_read_header(&mut src.0, 1);
        let coefs = jpeg_read_coefficients(&mut src.0);

        let mut dst = Enc(Box::new(mem::zeroed()));
        dst.0.common.err = &mut derr;
        jpeg_create_compress(&mut *dst.0);
        jpeg_copy_critical_parameters(&src.0, &mut dst.0);
        dst.0.optimize_coding = 1;
        // mozjpeg's copy_critical_parameters runs jpeg_set_defaults (max-compression profile =>
        // progressive + optimised scans). Make the choice explicit.
        if opts.progressive {
            jpeg_simple_progression(&mut dst.0);
        } else {
            dst.0.num_scans = 0;
            dst.0.scan_info = ptr::null();
            jpeg_c_set_bool_param(&mut dst.0, J_BOOLEAN_PARAM::JBOOLEAN_OPTIMIZE_SCANS, 0);
        }
        jpeg_mem_dest(&mut dst.0, &mut out.0, &mut out.1);
        jpeg_write_coefficients(&mut dst.0, coefs);
        let mut m = src.0.marker_list;
        while !m.is_null() {
            let mk = &*m;
            let data = slice::from_raw_parts(mk.data, mk.data_length as usize);
            if wanted(&opts.markers, mk.marker, data) {
                jpeg_write_marker(&mut dst.0, mk.marker as c_int, mk.data, mk.data_length);
            }
            m = mk.next;
        }
        jpeg_finish_compress(&mut dst.0);
        jpeg_finish_decompress(&mut src.0);
    })?;
    Ok(unsafe { slice::from_raw_parts(out.0, out.1 as usize) }.to_vec())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Subsampling {
    S444,
    S422,
    S420,
}

#[derive(Clone, Debug)]
pub struct LossyOpts {
    /// 0..=100
    pub quality: u8,
    pub progressive: bool,
    pub subsampling: Subsampling,
    pub trellis: bool,
    /// Optional ICC profile to embed (APP2 chunks written by caller side or here).
    pub icc: Option<Vec<u8>>,
}
impl Default for LossyOpts {
    fn default() -> Self {
        Self { quality: 75, progressive: true, subsampling: Subsampling::S420, trellis: true, icc: None }
    }
}

/// mozjpeg lossy encode from tightly packed RGB8 (`w*h*3` bytes).
pub fn encode_rgb8(rgb: &[u8], width: u32, height: u32, o: &LossyOpts) -> Result<Vec<u8>> {
    if rgb.len() != width as usize * height as usize * 3 || width == 0 || height == 0 {
        return Err("buffer size mismatch".into());
    }
    let mut err: jpeg_error_mgr = unsafe { mem::zeroed() };
    setup_err(&mut err);
    let mut out = OutBuf(ptr::null_mut(), 0);
    catch(|| unsafe {
        let mut c = Enc(Box::new(mem::zeroed()));
        c.0.common.err = &mut err;
        jpeg_create_compress(&mut *c.0);
        c.0.image_width = width;
        c.0.image_height = height;
        c.0.input_components = 3;
        c.0.in_color_space = J_COLOR_SPACE::JCS_RGB;
        jpeg_set_defaults(&mut c.0); // mozjpeg: max-compression profile (progressive+trellis+scan opt)
        jpeg_set_quality(&mut c.0, o.quality as c_int, 0);
        let (h, v) = match o.subsampling {
            Subsampling::S444 => (1, 1),
            Subsampling::S422 => (2, 1),
            Subsampling::S420 => (2, 2),
        };
        let ci = slice::from_raw_parts_mut(c.0.comp_info, 3);
        ci[0].h_samp_factor = h;
        ci[0].v_samp_factor = v;
        ci[1].h_samp_factor = 1;
        ci[1].v_samp_factor = 1;
        ci[2].h_samp_factor = 1;
        ci[2].v_samp_factor = 1;
        let t = o.trellis as boolean;
        jpeg_c_set_bool_param(&mut c.0, J_BOOLEAN_PARAM::JBOOLEAN_TRELLIS_QUANT, t);
        jpeg_c_set_bool_param(&mut c.0, J_BOOLEAN_PARAM::JBOOLEAN_TRELLIS_QUANT_DC, t);
        if !o.progressive {
            // Baseline: drop mozjpeg's progressive scan script, keep optimal Huffman + trellis.
            jpeg_c_set_bool_param(&mut c.0, J_BOOLEAN_PARAM::JBOOLEAN_OPTIMIZE_SCANS, 0);
            c.0.num_scans = 0;
            c.0.scan_info = ptr::null();
            c.0.optimize_coding = 1;
        }
        jpeg_mem_dest(&mut c.0, &mut out.0, &mut out.1);
        jpeg_start_compress(&mut c.0, 1);
        if let Some(icc) = &o.icc {
            write_icc(&mut c.0, icc);
        }
        let stride = width as usize * 3;
        while c.0.next_scanline < height {
            let mut row = rgb.as_ptr().add(c.0.next_scanline as usize * stride);
            jpeg_write_scanlines(&mut c.0, &mut row as *mut *const u8 as *mut *const u8 as _, 1);
        }
        jpeg_finish_compress(&mut c.0);
    })?;
    Ok(unsafe { slice::from_raw_parts(out.0, out.1 as usize) }.to_vec())
}

unsafe fn write_icc(c: &mut jpeg_compress_struct, icc: &[u8]) {
    const MAX: usize = 65533 - 14;
    let n = icc.len().div_ceil(MAX);
    for (i, chunk) in icc.chunks(MAX).enumerate() {
        let mut d = Vec::with_capacity(chunk.len() + 14);
        d.extend_from_slice(b"ICC_PROFILE\0");
        d.push(i as u8 + 1);
        d.push(n as u8);
        d.extend_from_slice(chunk);
        jpeg_write_marker(c, 0xE2, d.as_ptr(), d.len() as _);
    }
}

// ---------------------------------------------------------------------------------------------
// jpegli encode (linked in-process alongside mozjpeg; native `jpegli_*` symbols => no clash)
// ---------------------------------------------------------------------------------------------
#[derive(Clone, Debug)]
pub struct JpegliOpts {
    /// Butteraugli distance (1.0 ~ visually lossless ~ q90; cjpegli -d). Quality 75 ~ d 2.2.
    pub distance: f32,
    /// If set, use jpegli_set_quality (cjpegli -q semantics) instead of `distance`.
    pub quality: Option<u8>,
    /// 0..=2 (2 = most progressive/smallest, jpegli default)
    pub progressive_level: i32,
    pub subsampling: Subsampling,
    pub icc: Option<Vec<u8>>,
}
impl Default for JpegliOpts {
    fn default() -> Self {
        Self { distance: 1.0, quality: None, progressive_level: 2, subsampling: Subsampling::S444, icc: None }
    }
}

pub fn encode_rgb8_jpegli(rgb: &[u8], width: u32, height: u32, o: &JpegliOpts) -> Result<Vec<u8>> {
    if rgb.len() != width as usize * height as usize * 3 || width == 0 || height == 0 {
        return Err("buffer size mismatch".into());
    }
    let mut err: jpeg_error_mgr = unsafe { mem::zeroed() };
    unsafe { jpegli_sys::jpegli_std_error(&mut err) };
    err.error_exit = Some(error_exit);
    err.output_message = Some(silent);
    err.emit_message = Some(emit);
    let mut out = OutBuf(ptr::null_mut(), 0);
    struct JEnc(Box<jpeg_compress_struct>);
    impl Drop for JEnc {
        fn drop(&mut self) {
            unsafe { jpegli_sys::jpegli_destroy_compress(&mut self.0) }
        }
    }
    catch(|| unsafe {
        use jpegli_sys::*;
        let mut c = JEnc(Box::new(mem::zeroed()));
        c.0.common.err = &mut err;
        jpegli_CreateCompress(&mut *c.0, JPEG_LIB_VERSION, mem::size_of::<jpeg_compress_struct>());
        c.0.image_width = width;
        c.0.image_height = height;
        c.0.input_components = 3;
        c.0.in_color_space = J_COLOR_SPACE::JCS_RGB;
        jpegli_set_defaults(&mut c.0);
        let (h, v) = match o.subsampling {
            Subsampling::S444 => (1, 1),
            Subsampling::S422 => (2, 1),
            Subsampling::S420 => (2, 2),
        };
        let ci = slice::from_raw_parts_mut(c.0.comp_info, 3);
        ci[0].h_samp_factor = h;
        ci[0].v_samp_factor = v;
        ci[1].h_samp_factor = 1;
        ci[1].v_samp_factor = 1;
        ci[2].h_samp_factor = 1;
        ci[2].v_samp_factor = 1;
        match o.quality {
            Some(q) => jpegli_set_distance(&mut c.0, super_q2d(q), 1),
            None => jpegli_set_distance(&mut c.0, o.distance, 1),
        }
        jpegli_set_progressive_level(&mut c.0, o.progressive_level);
        c.0.optimize_coding = 1;
        jpegli_mem_dest(&mut c.0, &mut out.0, &mut out.1);
        jpegli_start_compress(&mut c.0, 1);
        if let Some(icc) = &o.icc {
            // Same chunking as write_icc(); jpegli_write_marker shares the libjpeg signature.
            const MAX: usize = 65533 - 14;
            let n = icc.len().div_ceil(MAX);
            for (i, chunk) in icc.chunks(MAX).enumerate() {
                let mut d = b"ICC_PROFILE\0".to_vec();
                d.push(i as u8 + 1);
                d.push(n as u8);
                d.extend_from_slice(chunk);
                jpegli_write_marker(&mut c.0, 0xE2, d.as_ptr(), d.len() as _);
            }
        }
        let stride = width as usize * 3;
        while c.0.next_scanline < height {
            let row = rgb.as_ptr().add(c.0.next_scanline as usize * stride);
            jpegli_write_scanlines(&mut c.0, &row, 1);
        }
        jpegli_finish_compress(&mut c.0);
    })?;
    Ok(unsafe { slice::from_raw_parts(out.0, out.1 as usize) }.to_vec())
}

fn super_q2d(q: u8) -> f32 {
    crate::jpeg_pipeline::quality_to_distance(q)
}
