//! Manrope through FreeType. The face stays open for the process; glyph bitmaps
//! are cached so a frame of Russian text does not re-rasterize.

use std::collections::HashMap;
use std::os::raw::{c_int, c_uchar, c_void};

#[derive(Clone)]
pub struct Glyph {
    pub w: i32,
    pub h: i32,
    pub left: i32,
    pub top: i32,
    pub advance: i32,
    pub pixels: Vec<u8>,
}

pub struct Font {
    face: *mut c_void,
    cache: HashMap<(char, i32, i32), Glyph>,
}

unsafe impl Send for Font {}

impl Font {
    pub fn open(bytes: &[u8]) -> Option<Self> {
        let face = unsafe { fv_font_open(bytes.as_ptr(), bytes.len() as i64) };
        if face.is_null() { None } else { Some(Self { face, cache: HashMap::new() }) }
    }

    pub fn glyph(&mut self, ch: char, px: i32, weight: i32) -> Glyph {
        let key = (ch, px.max(1), weight);
        if let Some(found) = self.cache.get(&key) {
            return found.clone();
        }
        let glyph = self.raster(ch, key.1, weight);
        self.cache.insert(key, glyph.clone());
        glyph
    }

    pub fn measure(&mut self, text: &str, px: i32, weight: i32) -> i32 {
        let mut width = 0;
        for ch in text.chars() {
            width += self.glyph(ch, px, weight).advance;
        }
        width
    }

    fn raster(&self, ch: char, px: i32, weight: i32) -> Glyph {
        let mut out = std::ptr::null_mut();
        let mut w = 0;
        let mut h = 0;
        let mut left = 0;
        let mut top = 0;
        let mut advance = px / 2;
        let rc = unsafe {
            fv_glyph(self.face, ch as u32, px, weight, &mut out, &mut w, &mut h, &mut left, &mut top, &mut advance)
        };
        if rc != 0 {
            return Glyph { w: 0, h: 0, left: 0, top: 0, advance, pixels: Vec::new() };
        }
        let pixels = if out.is_null() || w <= 0 || h <= 0 {
            Vec::new()
        } else {
            unsafe { std::slice::from_raw_parts(out, (w * h) as usize).to_vec() }
        };
        if !out.is_null() {
            unsafe { fv_free(out as *mut c_void) };
        }
        Glyph { w, h, left, top, advance, pixels }
    }
}

impl Drop for Font {
    fn drop(&mut self) {
        unsafe { fv_font_close(self.face) };
    }
}

extern "C" {
    fn fv_font_open(data: *const c_uchar, len: i64) -> *mut c_void;
    fn fv_font_close(font: *mut c_void);
    fn fv_glyph(
        font: *mut c_void,
        code: u32,
        px: c_int,
        weight: c_int,
        out: *mut *mut c_uchar,
        w: *mut c_int,
        h: *mut c_int,
        left: *mut c_int,
        top: *mut c_int,
        advance: *mut c_int,
    ) -> c_int;
    fn fv_free(p: *mut c_void);
}
