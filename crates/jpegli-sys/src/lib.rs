//! Raw jpegli bindings. Types are shared with mozjpeg-sys (libjpeg ABI 6.2).
pub use mozjpeg_sys::*;
use std::os::raw::{c_int, c_ulong, c_uint};
extern "C-unwind" {
    pub fn jpegli_std_error(err: &mut jpeg_error_mgr) -> &mut jpeg_error_mgr;
    pub fn jpegli_CreateCompress(cinfo: *mut jpeg_compress_struct, version: c_int, structsize: usize);
    pub fn jpegli_destroy_compress(cinfo: &mut jpeg_compress_struct);
    pub fn jpegli_mem_dest(cinfo: &mut jpeg_compress_struct, outbuffer: *mut *mut u8, outsize: *mut c_ulong);
    pub fn jpegli_set_defaults(cinfo: &mut jpeg_compress_struct);
    pub fn jpegli_set_quality(cinfo: &mut jpeg_compress_struct, quality: c_int, force_baseline: boolean);
    pub fn jpegli_set_distance(cinfo: &mut jpeg_compress_struct, distance: f32, force_baseline: boolean);
    pub fn jpegli_set_progressive_level(cinfo: &mut jpeg_compress_struct, level: c_int);
    pub fn jpegli_set_xyb_mode(cinfo: &mut jpeg_compress_struct);
    pub fn jpegli_start_compress(cinfo: &mut jpeg_compress_struct, write_all_tables: boolean);
    pub fn jpegli_write_scanlines(cinfo: &mut jpeg_compress_struct, scanlines: *const *const u8, n: c_uint) -> c_uint;
    pub fn jpegli_write_marker(cinfo: &mut jpeg_compress_struct, marker: c_int, data: *const u8, len: c_uint);
    pub fn jpegli_finish_compress(cinfo: &mut jpeg_compress_struct);
}
