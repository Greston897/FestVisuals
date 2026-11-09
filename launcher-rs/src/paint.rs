//! Software frame. Glass is Telegram's recipe, redrawn for a buffer: blur the
//! backdrop under a rounded panel, warm-tint it, then light a thin iridescent
//! rim from the top-left. Nothing here depends on a window system.

use crate::font::Font;

#[derive(Clone)]
pub struct Image {
    pub w: i32,
    pub h: i32,
    pub rgba: Vec<u8>,
}

impl Image {
    pub fn ppm(bytes: &[u8]) -> Option<Self> {
        if !bytes.starts_with(b"P6") { return None; }
        let mut i = 2;
        let mut nums: Vec<u32> = Vec::new();
        while nums.len() < 3 && i < bytes.len() {
            while i < bytes.len() && bytes[i].is_ascii_whitespace() { i += 1; }
            if bytes.get(i) == Some(&b'#') {
                while i < bytes.len() && bytes[i] != b'\n' { i += 1; }
                continue;
            }
            let start = i;
            while i < bytes.len() && bytes[i].is_ascii_digit() { i += 1; }
            nums.push(std::str::from_utf8(&bytes[start..i]).ok()?.parse().ok()?);
        }
        while i < bytes.len() && bytes[i].is_ascii_whitespace() { i += 1; }
        let (w, h) = (*nums.first()? as i32, *nums.get(1)? as i32);
        let rgb = &bytes[i..];
        if rgb.len() < (w * h * 3) as usize { return None; }
        let mut rgba = vec![255u8; (w * h * 4) as usize];
        for p in 0..(w * h) as usize {
            rgba[p * 4] = rgb[p * 3];
            rgba[p * 4 + 1] = rgb[p * 3 + 1];
            rgba[p * 4 + 2] = rgb[p * 3 + 2];
        }
        Some(Self { w, h, rgba })
    }

    pub fn rgba(bytes: &[u8], w: i32, h: i32) -> Option<Self> {
        if bytes.len() < (w * h * 4) as usize { return None; }
        Some(Self { w, h, rgba: bytes[..(w * h * 4) as usize].to_vec() })
    }

    /// Soft edge only. The world stays bright so the glass has color to bend.
    pub fn ink(&mut self) {
        if self.w <= 1 || self.h <= 1 { return; }
        for y in 0..self.h {
            let vy = y as f32 / self.h as f32;
            let vert = if vy < 0.08 { (1.0 - vy / 0.08) * 80.0 } else if vy > 0.9 { ((vy - 0.9) / 0.1) * 46.0 } else { 0.0 };
            for x in 0..self.w {
                let vx = x as f32 / self.w as f32;
                let edge = if vx < 0.34 { (1.0 - vx / 0.34) * 42.0 } else { 0.0 };
                let a = (vert + edge).min(96.0) as u32;
                if a == 0 { continue; }
                let i = ((y * self.w + x) * 4) as usize;
                let inv = 255 - a;
                self.rgba[i] = ((6 * a + self.rgba[i] as u32 * inv) / 255) as u8;
                self.rgba[i + 1] = ((4 * a + self.rgba[i + 1] as u32 * inv) / 255) as u8;
                self.rgba[i + 2] = ((8 * a + self.rgba[i + 2] as u32 * inv) / 255) as u8;
            }
        }
    }
}

pub struct Canvas {
    pub w: i32,
    pub h: i32,
    pub px: Vec<u8>,
    scene: Vec<u8>,
    blur: Vec<u8>,
    blur_w: i32,
    blur_h: i32,
}

impl Canvas {
    pub fn new(w: i32, h: i32) -> Self {
        Self { w, h, px: vec![0; (w * h * 4).max(0) as usize], scene: Vec::new(), blur: Vec::new(), blur_w: 0, blur_h: 0 }
    }

    pub fn resize(&mut self, w: i32, h: i32) {
        if self.w == w && self.h == h { return; }
        *self = Self::new(w, h);
    }

    pub fn fill(&mut self, c: [u8; 4]) {
        for p in self.px.chunks_exact_mut(4) {
            p.copy_from_slice(&c);
        }
    }

    fn idx(&self, x: i32, y: i32) -> Option<usize> {
        if x < 0 || y < 0 || x >= self.w || y >= self.h { None } else { Some((y * self.w + x) as usize * 4) }
    }

    pub fn blend(&mut self, x: i32, y: i32, c: [u8; 4]) {
        let Some(i) = self.idx(x, y) else { return };
        let dst = &mut self.px[i..i + 4];
        let a = c[3] as u32;
        if a == 0 { return; }
        if a == 255 {
            dst[..3].copy_from_slice(&c[..3]);
            dst[3] = 255;
            return;
        }
        let inv = 255 - a;
        for k in 0..3 {
            dst[k] = ((c[k] as u32 * a + dst[k] as u32 * inv) / 255) as u8;
        }
        dst[3] = 255;
    }

    pub fn screen(&mut self, x: i32, y: i32, c: [u8; 4]) {
        let Some(i) = self.idx(x, y) else { return };
        let a = c[3] as u32;
        if a == 0 { return; }
        let dst = &mut self.px[i..i + 4];
        for k in 0..3 {
            let s = 255 - ((255 - dst[k] as u32) * (255 - c[k] as u32) / 255);
            dst[k] = (dst[k] as u32 + (s - dst[k] as u32) * a / 255) as u8;
        }
    }

    pub fn cover(&mut self, image: &Image, zoom: f32, ox: f32, oy: f32) {
        if image.w <= 0 || image.h <= 0 { return; }
        let scale = (self.w as f32 / image.w as f32).max(self.h as f32 / image.h as f32) * zoom;
        let dw = image.w as f32 * scale;
        let dh = image.h as f32 * scale;
        let x0 = (self.w as f32 - dw) * 0.62 + ox;
        let y0 = (self.h as f32 - dh) * 0.42 + oy;
        for y in 0..self.h {
            let v = ((y as f32 - y0) / dh).clamp(0.0, 0.9999);
            let sy = (v * image.h as f32) as i32;
            for x in 0..self.w {
                let u = ((x as f32 - x0) / dw).clamp(0.0, 0.9999);
                let sx = (u * image.w as f32) as i32;
                let s = ((sy * image.w + sx) * 4) as usize;
                let i = ((y * self.w + x) * 4) as usize;
                self.px[i..i + 4].copy_from_slice(&image.rgba[s..s + 4]);
            }
        }
    }

    pub fn radial_screen(&mut self, cx: f32, cy: f32, radius: f32, color: [u8; 3], alpha: f32) {
        if radius <= 1.0 { return; }
        let x0 = (cx - radius).floor().max(0.0) as i32;
        let y0 = (cy - radius).floor().max(0.0) as i32;
        let x1 = (cx + radius).ceil().min(self.w as f32) as i32;
        let y1 = (cy + radius).ceil().min(self.h as f32) as i32;
        let r2 = radius * radius;
        for y in y0..y1 {
            for x in x0..x1 {
                let dx = x as f32 + 0.5 - cx;
                let dy = y as f32 + 0.5 - cy;
                let d = dx * dx + dy * dy;
                if d >= r2 { continue; }
                let t = 1.0 - (d / r2).sqrt();
                let a = (t * t * alpha * 255.0) as u8;
                self.screen(x, y, [color[0], color[1], color[2], a]);
            }
        }
    }

    pub fn vignette(&mut self) {
        let cx = self.w as f32 * 0.62;
        let cy = self.h as f32 * 0.48;
        let rx = self.w as f32 * 0.72;
        let ry = self.h as f32 * 0.72;
        for y in 0..self.h {
            for x in 0..self.w {
                let nx = (x as f32 - cx) / rx;
                let ny = (y as f32 - cy) / ry;
                let d = (nx * nx + ny * ny).sqrt();
                let v = ((d - 0.42) / 0.58).clamp(0.0, 1.0);
                let a = (v * v * 150.0) as u8;
                self.blend(x, y, [7, 6, 11, a]);
            }
        }
    }

    pub fn veil(&mut self) {
        for y in 0..self.h {
            let vy = y as f32 / self.h as f32;
            let vert = if vy < 0.2 { (1.0 - vy / 0.2) * 80.0 } else if vy > 0.68 { ((vy - 0.68) / 0.32) * 108.0 } else { 0.0 };
            for x in 0..self.w {
                let vx = x as f32 / self.w as f32;
                let horiz = if vx < 0.42 { (1.0 - vx / 0.42) * 70.0 } else if vx > 0.72 { ((vx - 0.72) / 0.28) * 50.0 } else { 8.0 };
                let a = (horiz + vert).min(190.0) as u8;
                self.blend(x, y, [7, 6, 11, a]);
            }
        }
    }

    pub fn grain(&mut self, seed: u32) {
        for y in (0..self.h).step_by(2) {
            for x in (0..self.w).step_by(2) {
                let n = hash(x, y, seed);
                let a = (n % 28) as u8;
                self.blend(x, y, [255, 244, 230, a]);
            }
        }
    }

    pub fn round_rect(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32, color: [u8; 4]) {
        self.shade(x, y, w, h, r, |_, _, cov| {
            [color[0], color[1], color[2], (color[3] as f32 * cov) as u8]
        });
    }

    pub fn shade(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32, mut f: impl FnMut(f32, f32, f32) -> [u8; 4]) {
        if w < 1.0 || h < 1.0 { return; }
        let x0 = x.floor().max(0.0) as i32;
        let y0 = y.floor().max(0.0) as i32;
        let x1 = (x + w).ceil().min(self.w as f32) as i32;
        let y1 = (y + h).ceil().min(self.h as f32) as i32;
        for py in y0..y1 {
            for px in x0..x1 {
                let lx = px as f32 + 0.5 - x;
                let ly = py as f32 + 0.5 - y;
                let sd = sd_round(lx, ly, w, h, r);
                if sd > 0.8 { continue; }
                let cov = (0.55 - sd).clamp(0.0, 1.0);
                let c = f(lx, ly, cov);
                self.blend(px, py, c);
            }
        }
    }

    /// One cheap blur of the scene. Panels sample this instead of blurring again.
    pub fn freeze(&mut self) {
        self.scene = self.px.clone();
        let bw = (self.w / 2).max(2);
        let bh = (self.h / 2).max(2);
        let mut small = vec![0u8; (bw * bh * 4) as usize];
        for y in 0..bh {
            let sy = (y * self.h / bh).min(self.h - 1);
            for x in 0..bw {
                let sx = (x * self.w / bw).min(self.w - 1);
                let s = ((sy * self.w + sx) * 4) as usize;
                let d = ((y * bw + x) * 4) as usize;
                small[d..d + 4].copy_from_slice(&self.px[s..s + 4]);
            }
        }
        self.blur = box_blur(&small, bw, bh, 1);
        self.blur_w = bw;
        self.blur_h = bh;
    }

    /// Liquid glass. Call `freeze` after the scene and before the panels.
    pub fn glass(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32, strong: bool, iris: f32, spot: (f32, f32), glare: Option<(f32, f32)>) {
        if w < 4.0 || h < 4.0 { return; }
        if self.blur.is_empty() { self.freeze(); }
        let fw = self.w as f32;
        let fh = self.h as f32;
        let (spot_x, spot_y) = spot;
        let x0 = x.floor().max(0.0) as i32;
        let y0 = y.floor().max(0.0) as i32;
        let x1 = (x + w).ceil().min(fw) as i32;
        let y1 = (y + h).ceil().min(fh) as i32;
        for py in y0..y1 {
            for px in x0..x1 {
                let lx = px as f32 + 0.5 - x;
                let ly = py as f32 + 0.5 - y;
                let sd = sd_round(lx, ly, w, h, r);
                if sd > 1.2 { continue; }
                let cov = (1.1 - sd).clamp(0.0, 1.0);
                // Sharp world in the middle. Refraction and the split live at the rim,
                // which is what a thick lens actually does.
                let nx = (lx / w - 0.5) * 2.0;
                let ny = (ly / h - 0.5) * 2.0;
                let reach = if strong { 26.0 } else { 16.0 };
                let rim = ((sd + reach) / reach).clamp(0.0, 1.0);
                let rim2 = rim * rim;
                let pull = if strong { 9.0 } else { 13.0 };
                let sx = px as f32 - nx * rim2 * pull;
                let sy = py as f32 - ny * rim2 * pull * 0.7;
                let sharp = sample_buf(&self.scene, self.w, self.h, sx, sy, fw, fh);
                let soft = sample_buf(&self.blur, self.blur_w, self.blur_h, sx, sy, fw, fh);
                let mix = rim2 * if strong { 0.7 } else { 0.4 };
                let mut c = [0u8; 3];
                for k in 0..3 {
                    c[k] = (sharp[k] as f32 * (1.0 - mix) + soft[k] as f32 * mix) as u8;
                }
                if rim2 > 0.02 {
                    let shift = rim2 * if strong { 8.0 } else { 11.0 };
                    let red = sample_buf(&self.scene, self.w, self.h, sx + shift, sy, fw, fh);
                    let blue = sample_buf(&self.scene, self.w, self.h, sx - shift, sy, fw, fh);
                    c[0] = (c[0] as f32 * (1.0 - rim2) + red[0] as f32 * rim2) as u8;
                    c[2] = (c[2] as f32 * (1.0 - rim2) + blue[2] as f32 * rim2) as u8;
                }
                let veil = rim2 * if strong { 0.42 } else { 0.16 };
                for k in 0..3 {
                    c[k] = (c[k] as f32 * (1.0 - veil) + 250.0 * veil) as u8;
                }
                let dx = lx / w - spot_x;
                let dy = ly / h - spot_y;
                let crest = (1.0 - (dx * dx * 9.0 + dy * dy * 40.0).sqrt() / 0.22).clamp(0.0, 1.0);
                let crest_a = crest * crest * if strong { 0.22 } else { 0.3 };
                for k in 0..3 {
                    c[k] = (c[k] as f32 + (255.0 - c[k] as f32) * crest_a) as u8;
                }
                let sweep = (iris * 0.0022).rem_euclid(1.7) - 0.35;
                let band = ((lx / w) * 0.96 + (ly / h) * 0.05 - sweep).abs();
                if band < 0.006 {
                    let a = (1.0 - band / 0.006) * 0.7;
                    for k in 0..3 { c[k] = (c[k] as f32 + (255.0 - c[k] as f32) * a) as u8; }
                }
                let _ = sharp;
                let color = [c[0], c[1], c[2], (cov * 255.0) as u8];
                if sd < -1.4 {
                    let i = ((py * self.w + px) * 4) as usize;
                    self.px[i] = color[0];
                    self.px[i + 1] = color[1];
                    self.px[i + 2] = color[2];
                    self.px[i + 3] = 255;
                } else {
                    self.blend(px, py, color);
                }
            }
        }
        self.prism(x, y, w, h, r, iris, if strong { 1.0 } else { 0.92 });
        if let Some((gx, gy)) = glare {
            self.shade(x, y, w, h, r, |lx, ly, cov| {
                let dx = (lx - gx) / 140.0;
                let dy = (ly - gy) / 100.0;
                let d = (dx * dx + dy * dy).sqrt();
                let a = ((1.0 - d).clamp(0.0, 1.0)).powi(2) * 0.28 * cov;
                [255, 255, 255, (a * 255.0) as u8]
            });
        }
    }

    pub fn rim(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32, iris: f32, opacity: f32) {
        self.prism(x, y, w, h, r, iris, opacity);
    }

    pub fn blit_round(&mut self, image: &Image, x: f32, y: f32, w: f32, h: f32, r: f32, ox: f32, oy: f32) {
        if image.w <= 1 || image.h <= 1 || w < 2.0 || h < 2.0 { return; }
        let scale = (w / image.w as f32).max(h / image.h as f32);
        let dw = image.w as f32 * scale;
        let dh = image.h as f32 * scale;
        let ix = x + (w - dw) * 0.5 + ox;
        let iy = y + (h - dh) * 0.5 + oy;
        let x0 = x.floor().max(0.0) as i32;
        let y0 = y.floor().max(0.0) as i32;
        let x1 = (x + w).ceil().min(self.w as f32) as i32;
        let y1 = (y + h).ceil().min(self.h as f32) as i32;
        for py in y0..y1 {
            let v = ((py as f32 - iy) / dh).clamp(0.0, 0.9999);
            let sy = (v * image.h as f32) as i32;
            for px in x0..x1 {
                let lx = px as f32 + 0.5 - x;
                let ly = py as f32 + 0.5 - y;
                let sd = sd_round(lx, ly, w, h, r);
                if sd > 0.7 { continue; }
                let cov = (0.6 - sd).clamp(0.0, 1.0);
                let u = ((px as f32 - ix) / dw).clamp(0.0, 0.9999);
                let sx = (u * image.w as f32) as i32;
                let s = ((sy * image.w + sx) * 4) as usize;
                if sd < -1.2 {
                    let i = ((py * self.w + px) * 4) as usize;
                    self.px[i..i + 4].copy_from_slice(&image.rgba[s..s + 4]);
                } else {
                    let a = (image.rgba[s + 3] as f32 * cov) as u8;
                    self.blend(px, py, [image.rgba[s], image.rgba[s + 1], image.rgba[s + 2], a]);
                }
            }
        }
    }

    pub fn scrim(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32) {
        self.shade(x, y, w, h, r, |_, ly, cov| {
            let t = (ly / h).clamp(0.0, 1.0);
            let top = if t < 0.28 { (0.28 - t) / 0.28 * 70.0 } else { 0.0 };
            let bot = if t > 0.38 { ((t - 0.38) / 0.62).powf(1.35) * 200.0 } else { 0.0 };
            [4, 3, 8, ((top + bot) * cov) as u8]
        });
    }

    pub fn grade(&mut self) {
        for y in 0..self.h {
            let vy = y as f32 / self.h as f32;
            let vert = if vy < 0.10 {
                (1.0 - vy / 0.10) * 110.0
            } else if vy > 0.86 {
                ((vy - 0.86) / 0.14) * 90.0
            } else {
                12.0
            };
            for x in 0..self.w {
                let vx = x as f32 / self.w as f32;
                let edge = {
                    let e = (vx - 0.5).abs() * 2.0;
                    if e > 0.82 { ((e - 0.82) / 0.18) * 36.0 } else { 0.0 }
                };
                let a = (vert + edge).min(150.0) as u8;
                self.blend(x, y, [5, 4, 8, a]);
            }
        }
    }

    fn prism(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32, iris: f32, opacity: f32) {
        let x0 = x.floor().max(0.0) as i32;
        let y0 = y.floor().max(0.0) as i32;
        let x1 = (x + w).ceil().min(self.w as f32) as i32;
        let y1 = (y + h).ceil().min(self.h as f32) as i32;
        let cx = w * 0.5;
        let cy = h * 0.5;
        let band = (r + 8.0) as i32;
        for py in y0..y1 {
            let ly = py as f32 + 0.5 - y;
            let near_y = ly < r + 8.0 || ly > h - r - 8.0;
            let spans = if near_y { [(x0, x1), (x0, x0)] } else { [(x0, (x0 + band).min(x1)), ((x1 - band).max(x0), x1)] };
            for (xa, xb) in spans {
                for px in xa..xb {
                let lx = px as f32 + 0.5 - x;
                let ly = py as f32 + 0.5 - y;
                let sd = sd_round(lx, ly, w, h, r);
                if sd > 1.4 || sd < -4.6 { continue; }
                let edge = (1.0 - (sd + 1.2).abs() / 2.0).clamp(0.0, 1.0);
                if edge <= 0.0 { continue; }
                let angle = (ly - cy).atan2(lx - cx).to_degrees() + iris;
                let deg = angle.rem_euclid(360.0);
                let (rgb, stop_a) = conic(deg);
                // Sides of the ring carry more of the dark stroke, matching GlassRim.
                let side = ((lx / w - 0.5).abs() * 2.0).powf(1.4);
                let dark = side * 0.33 * edge;
                let a = (edge * stop_a * opacity * 255.0) as u8;
                self.screen(px, py, [rgb[0], rgb[1], rgb[2], a]);
                if dark > 0.02 {
                    self.blend(px, py, [0, 0, 0, (dark * 180.0) as u8]);
                }
                }
            }
        }
    }

    pub fn text(&mut self, font: &mut Font, text: &str, x: f32, y: f32, size: i32, weight: i32, color: [u8; 4]) -> f32 {
        let mut pen = x;
        let ascender = size as f32 * 0.80;
        for ch in text.chars() {
            let glyph = font.glyph(ch, size, weight);
            if !glyph.pixels.is_empty() {
                let gx = pen + glyph.left as f32;
                let gy = y + ascender - glyph.top as f32;
                for row in 0..glyph.h {
                    for col in 0..glyph.w {
                        let cov = glyph.pixels[(row * glyph.w + col) as usize];
                        if cov == 0 { continue; }
                        let a = (cov as u32 * color[3] as u32 / 255) as u8;
                        self.blend(gx as i32 + col, gy as i32 + row, [color[0], color[1], color[2], a]);
                    }
                }
            }
            pen += glyph.advance as f32;
        }
        pen - x
    }

    pub fn text_iris(&mut self, font: &mut Font, text: &str, x: f32, y: f32, size: i32, weight: i32, time: f32) -> f32 {
        let mut pen = x;
        let ascender = size as f32 * 0.80;
        for ch in text.chars() {
            let glyph = font.glyph(ch, size, weight);
            let rgb = iris_at(pen, time);
            if !glyph.pixels.is_empty() {
                let gx = pen + glyph.left as f32;
                let gy = y + ascender - glyph.top as f32;
                for row in 0..glyph.h {
                    for col in 0..glyph.w {
                        let cov = glyph.pixels[(row * glyph.w + col) as usize];
                        if cov == 0 { continue; }
                        self.blend(gx as i32 + col, gy as i32 + row, [rgb[0], rgb[1], rgb[2], cov]);
                    }
                }
            }
            pen += glyph.advance as f32;
        }
        pen - x
    }

    pub fn circle(&mut self, cx: f32, cy: f32, radius: f32, color: [u8; 4]) {
        let x0 = (cx - radius - 1.0).floor().max(0.0) as i32;
        let y0 = (cy - radius - 1.0).floor().max(0.0) as i32;
        let x1 = (cx + radius + 1.0).ceil().min(self.w as f32) as i32;
        let y1 = (cy + radius + 1.0).ceil().min(self.h as f32) as i32;
        for y in y0..y1 {
            for x in x0..x1 {
                let d = ((x as f32 + 0.5 - cx).powi(2) + (y as f32 + 0.5 - cy).powi(2)).sqrt();
                let cov = (radius + 0.5 - d).clamp(0.0, 1.0);
                if cov <= 0.0 { continue; }
                self.blend(x, y, [color[0], color[1], color[2], (color[3] as f32 * cov) as u8]);
            }
        }
    }

    pub fn stroke_circle(&mut self, cx: f32, cy: f32, radius: f32, width: f32, color: [u8; 4]) {
        let x0 = (cx - radius - width).floor().max(0.0) as i32;
        let y0 = (cy - radius - width).floor().max(0.0) as i32;
        let x1 = (cx + radius + width).ceil().min(self.w as f32) as i32;
        let y1 = (cy + radius + width).ceil().min(self.h as f32) as i32;
        for y in y0..y1 {
            for x in x0..x1 {
                let d = ((x as f32 + 0.5 - cx).powi(2) + (y as f32 + 0.5 - cy).powi(2)).sqrt();
                let cov = (width * 0.5 - (d - radius).abs()).clamp(0.0, 1.0);
                if cov <= 0.0 { continue; }
                self.blend(x, y, [color[0], color[1], color[2], (color[3] as f32 * cov) as u8]);
            }
        }
    }

    pub fn line(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, width: f32, color: [u8; 4]) {
        let steps = ((x1 - x0).hypot(y1 - y0) * 2.0).ceil() as i32 + 1;
        for i in 0..=steps {
            let t = i as f32 / steps as f32;
            self.circle(x0 + (x1 - x0) * t, y0 + (y1 - y0) * t, width * 0.5, color);
        }
    }

    pub fn write_ppm(&self, path: &std::path::Path) -> std::io::Result<()> {
        use std::io::Write;
        let mut file = std::fs::File::create(path)?;
        write!(file, "P6\n{} {}\n255\n", self.w, self.h)?;
        let mut rgb = Vec::with_capacity((self.w * self.h * 3) as usize);
        for p in self.px.chunks_exact(4) {
            rgb.extend_from_slice(&p[..3]);
        }
        file.write_all(&rgb)
    }
}

fn sample_buf(buf: &[u8], w: i32, h: i32, x: f32, y: f32, fw: f32, fh: f32) -> [u8; 3] {
    if buf.is_empty() || w <= 1 || h <= 1 || fw <= 1.0 || fh <= 1.0 {
        return [18, 12, 10];
    }
    let u = (x / fw).clamp(0.0, 0.999) * (w - 1) as f32;
    let v = (y / fh).clamp(0.0, 0.999) * (h - 1) as f32;
    let x0 = u.floor() as i32;
    let y0 = v.floor() as i32;
    let x1 = (x0 + 1).min(w - 1);
    let y1 = (y0 + 1).min(h - 1);
    let tx = u - x0 as f32;
    let ty = v - y0 as f32;
    let at = |px: i32, py: i32| {
        let i = ((py * w + px) * 4) as usize;
        [buf[i] as f32, buf[i + 1] as f32, buf[i + 2] as f32]
    };
    let a = at(x0, y0);
    let b = at(x1, y0);
    let c = at(x0, y1);
    let d = at(x1, y1);
    let mut out = [0u8; 3];
    for k in 0..3 {
        let top = a[k] + (b[k] - a[k]) * tx;
        let bot = c[k] + (d[k] - c[k]) * tx;
        out[k] = (top + (bot - top) * ty) as u8;
    }
    out
}

fn blur_half(src: &[u8], w: i32, h: i32) -> Vec<u8> {
    let hw = (w / 2).max(1);
    let hh = (h / 2).max(1);
    let mut small = vec![0u8; (hw * hh * 4) as usize];
    for y in 0..hh {
        for x in 0..hw {
            let sx = (x * 2).min(w - 1);
            let sy = (y * 2).min(h - 1);
            let s = ((sy * w + sx) * 4) as usize;
            let d = ((y * hw + x) * 4) as usize;
            small[d..d + 4].copy_from_slice(&src[s..s + 4]);
        }
    }
    let blurred = box_blur(&small, hw, hh, 5);
    let mut up = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        let sy = ((y / 2).min(hh - 1)) as usize;
        for x in 0..w {
            let sx = ((x / 2).min(hw - 1)) as usize;
            let s = (sy * hw as usize + sx) * 4;
            let d = ((y * w + x) as usize) * 4;
            up[d..d + 4].copy_from_slice(&blurred[s..s + 4]);
        }
    }
    up
}

fn box_blur(src: &[u8], w: i32, h: i32, radius: i32) -> Vec<u8> {
    let mut tmp = vec![0u8; src.len()];
    let mut out = vec![0u8; src.len()];
    let r = radius.max(1) as i32;
    let span = (r * 2 + 1) as u32;
    for y in 0..h {
        for x in 0..w {
            let mut acc = [0u32; 3];
            for k in -r..=r {
                let sx = (x + k).clamp(0, w - 1);
                let i = ((y * w + sx) * 4) as usize;
                acc[0] += src[i] as u32;
                acc[1] += src[i + 1] as u32;
                acc[2] += src[i + 2] as u32;
            }
            let i = ((y * w + x) * 4) as usize;
            tmp[i] = (acc[0] / span) as u8;
            tmp[i + 1] = (acc[1] / span) as u8;
            tmp[i + 2] = (acc[2] / span) as u8;
            tmp[i + 3] = 255;
        }
    }
    for x in 0..w {
        for y in 0..h {
            let mut acc = [0u32; 3];
            for k in -r..=r {
                let sy = (y + k).clamp(0, h - 1);
                let i = ((sy * w + x) * 4) as usize;
                acc[0] += tmp[i] as u32;
                acc[1] += tmp[i + 1] as u32;
                acc[2] += tmp[i + 2] as u32;
            }
            let i = ((y * w + x) * 4) as usize;
            out[i] = (acc[0] / span) as u8;
            out[i + 1] = (acc[1] / span) as u8;
            out[i + 2] = (acc[2] / span) as u8;
            out[i + 3] = 255;
        }
    }
    out
}

fn sd_round(x: f32, y: f32, w: f32, h: f32, r: f32) -> f32 {
    let r = r.min(w * 0.5).min(h * 0.5);
    let qx = (x - w * 0.5).abs() - w * 0.5 + r;
    let qy = (y - h * 0.5).abs() - h * 0.5 + r;
    (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt() + qx.max(qy).min(0.0) - r
}

fn saturate(c: &mut [u8; 3], amount: f32) {
    let gray = c[0] as f32 * 0.30 + c[1] as f32 * 0.59 + c[2] as f32 * 0.11;
    for k in 0..3 {
        c[k] = (gray + (c[k] as f32 - gray) * amount).clamp(0.0, 255.0) as u8;
    }
}

fn lerp3(a: [u8; 3], b: [u8; 3], t: f32) -> [u8; 3] {
    let t = t.clamp(0.0, 1.0);
    [
        (a[0] as f32 + (b[0] as f32 - a[0] as f32) * t) as u8,
        (a[1] as f32 + (b[1] as f32 - a[1] as f32) * t) as u8,
        (a[2] as f32 + (b[2] as f32 - a[2] as f32) * t) as u8,
    ]
}

fn conic(deg: f32) -> ([u8; 3], f32) {
    const STOPS: &[(f32, [u8; 3], f32)] = &[
        (0.0, [255, 176, 90], 0.05),
        (36.0, [255, 228, 190], 0.95),
        (78.0, [255, 255, 255], 0.15),
        (132.0, [150, 210, 255], 0.80),
        (188.0, [206, 160, 255], 0.65),
        (246.0, [255, 112, 48], 0.85),
        (310.0, [255, 220, 170], 0.20),
        (360.0, [255, 176, 90], 0.05),
    ];
    let mut i = 0;
    while i + 1 < STOPS.len() && STOPS[i + 1].0 < deg { i += 1; }
    let (a, b) = (STOPS[i], STOPS[(i + 1).min(STOPS.len() - 1)]);
    let t = if b.0 == a.0 { 0.0 } else { (deg - a.0) / (b.0 - a.0) };
    (lerp3(a.1, b.1, t), a.2 + (b.2 - a.2) * t)
}

pub fn iris_at(x: f32, time: f32) -> [u8; 3] {
    let t = (x * 0.015 + time * 0.08).rem_euclid(1.0);
    const STOPS: [[u8; 3]; 6] = [
        [255, 122, 24],
        [255, 214, 140],
        [255, 250, 245],
        [150, 206, 255],
        [214, 160, 255],
        [255, 96, 48],
    ];
    let scaled = t * STOPS.len() as f32;
    let i = scaled.floor() as usize % STOPS.len();
    let j = (i + 1) % STOPS.len();
    lerp3(STOPS[i], STOPS[j], scaled.fract())
}

fn hash(x: i32, y: i32, seed: u32) -> u32 {
    let mut n = (x as u32).wrapping_mul(374761393) ^ (y as u32).wrapping_mul(668265263) ^ seed;
    n = (n ^ (n >> 13)).wrapping_mul(1274126177);
    n ^ (n >> 16)
}

pub fn wrap(font: &mut Font, text: &str, size: i32, weight: i32, max_width: i32) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        let trial = if line.is_empty() { word.to_string() } else { format!("{line} {word}") };
        if font.measure(&trial, size, weight) > max_width && !line.is_empty() {
            lines.push(line);
            line = word.to_string();
        } else {
            line = trial;
        }
    }
    if !line.is_empty() { lines.push(line); }
    lines
}
