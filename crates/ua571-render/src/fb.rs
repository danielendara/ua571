//! Fixed-resolution monochrome framebuffer (GRiD / CGA-style canvas).

/// Logical display width matching the GRiD Pascal recreation coordinates.
pub const WIDTH: usize = 640;
/// Tall enough for options panel baseline at y≈212 plus margin.
pub const HEIGHT: usize = 240;

/// 1-bit framebuffer with GRiD-like draw primitives.
#[derive(Clone)]
pub struct Framebuffer {
    /// Packed pixels: true = lit (phosphor on).
    pub pixels: Vec<bool>,
}

impl Default for Framebuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl Framebuffer {
    pub fn new() -> Self {
        Self {
            pixels: vec![false; WIDTH * HEIGHT],
        }
    }

    pub fn clear(&mut self) {
        self.pixels.fill(false);
    }

    #[inline]
    pub fn in_bounds(x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && (x as usize) < WIDTH && (y as usize) < HEIGHT
    }

    #[inline]
    pub fn set(&mut self, x: i32, y: i32, on: bool) {
        if Self::in_bounds(x, y) {
            self.pixels[y as usize * WIDTH + x as usize] = on;
        }
    }

    #[inline]
    #[cfg(test)]
    pub fn get(&self, x: i32, y: i32) -> bool {
        if Self::in_bounds(x, y) {
            self.pixels[y as usize * WIDTH + x as usize]
        } else {
            false
        }
    }

    pub fn pixel(&mut self, x: i32, y: i32) {
        self.set(x, y, true);
    }

    /// Bresenham line (WinDrawLine).
    pub fn line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32) {
        let mut x0 = x0;
        let mut y0 = y0;
        let dx = (x1 - x0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let dy = -(y1 - y0).abs();
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;

        loop {
            self.pixel(x0, y0);
            if x0 == x1 && y0 == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x0 += sx;
            }
            if e2 <= dx {
                err += dx;
                y0 += sy;
            }
        }
    }

    /// Draw a rectangle outline of exact pixel size `w` × `h` (edges at `x` and `x+w-1`).
    pub fn rect_outline(&mut self, x: i32, y: i32, w: i32, h: i32) {
        if w <= 0 || h <= 0 {
            return;
        }
        let x1 = x + w - 1;
        let y1 = y + h - 1;
        self.line(x, y, x1, y);
        self.line(x, y1, x1, y1);
        self.line(x, y, x, y1);
        self.line(x1, y, x1, y1);
    }

    /// XOR invert rectangle (WinInvertRectangle) — selection / CRITICAL flash.
    pub fn invert_rect(&mut self, x: i32, y: i32, w: i32, h: i32) {
        if w <= 0 || h <= 0 {
            return;
        }
        let x1 = (x + w).min(WIDTH as i32);
        let y1 = (y + h).min(HEIGHT as i32);
        let x0 = x.max(0);
        let y0 = y.max(0);
        for py in y0..y1 {
            for px in x0..x1 {
                let i = py as usize * WIDTH + px as usize;
                self.pixels[i] = !self.pixels[i];
            }
        }
    }

    pub fn fill_rect(&mut self, x: i32, y: i32, w: i32, h: i32, on: bool) {
        if w <= 0 || h <= 0 {
            return;
        }
        let x1 = (x + w).min(WIDTH as i32);
        let y1 = (y + h).min(HEIGHT as i32);
        let x0 = x.max(0);
        let y0 = y.max(0);
        for py in y0..y1 {
            for px in x0..x1 {
                self.set(px, py, on);
            }
        }
    }

    /// Midpoint circle (from original HEADER.PAS Circle).
    pub fn circle(&mut self, xcent: i32, ycent: i32, rad: i32) {
        if rad <= 0 {
            return;
        }
        let mut x = 0;
        let mut y = rad;
        let mut d = 1 - rad;
        let mut d_e = 3;
        let mut d_se = -(rad * 2) + 5;

        let plot = |fb: &mut Framebuffer, cx: i32, cy: i32, px: i32, py: i32| {
            fb.pixel(cx + px, cy - py);
            fb.pixel(cx - px, cy + py);
            fb.pixel(cx - px, cy - py);
            fb.pixel(cx + px, cy + py);
            fb.pixel(cx + py, cy - px);
            fb.pixel(cx - py, cy + px);
            fb.pixel(cx - py, cy - px);
            fb.pixel(cx + py, cy + px);
        };

        plot(self, xcent, ycent, x, y);
        while y > x {
            if d < 0 {
                d += d_e;
                d_e += 2;
                d_se += 2;
                x += 1;
            } else {
                d += d_se;
                d_e += 2;
                d_se += 4;
                x += 1;
                y -= 1;
            }
            plot(self, xcent, ycent, x, y);
        }
    }

    /// Draw 8×8 bitmap glyph scaled by `scale` (1 = section, 2 = header-ish).
    pub fn draw_char(&mut self, ch: char, x: i32, y: i32, scale: i32) {
        let glyph = crate::font::glyph(ch);
        let scale = scale.max(1);
        for row in 0..8i32 {
            let bits = glyph[row as usize];
            for col in 0..8i32 {
                if bits & (0x80 >> col) != 0 {
                    if scale == 1 {
                        self.pixel(x + col, y + row);
                    } else {
                        self.fill_rect(x + col * scale, y + row * scale, scale, scale, true);
                    }
                }
            }
        }
    }

    pub fn draw_text(&mut self, text: &str, x: i32, y: i32, scale: i32) {
        let scale = scale.max(1);
        let advance = 8 * scale;
        let mut cx = x;
        for ch in text.chars() {
            if ch == '\n' {
                continue;
            }
            self.draw_char(ch, cx, y, scale);
            cx += advance;
        }
    }

    pub fn text_width(text: &str, scale: i32) -> i32 {
        text.chars().count() as i32 * 8 * scale.max(1)
    }

    /// Present monochrome buffer as 0x00RRGGBB for minifb (nearest-neighbor scale).
    pub fn present_scaled(
        &self,
        out: &mut [u32],
        window_w: usize,
        window_h: usize,
        on_color: u32,
        off_color: u32,
    ) {
        if window_w == 0 || window_h == 0 || out.len() < window_w * window_h {
            return;
        }
        for wy in 0..window_h {
            let sy = wy * HEIGHT / window_h;
            for wx in 0..window_w {
                let sx = wx * WIDTH / window_w;
                let on = self.pixels[sy * WIDTH + sx];
                out[wy * window_w + wx] = if on { on_color } else { off_color };
            }
        }
    }

    /// Present as RGBA8 bytes for HTML Canvas `ImageData` (nearest-neighbor scale).
    ///
    /// `on_rgba` / `off_rgba` are `[r, g, b, a]`.
    pub fn present_rgba(
        &self,
        out: &mut [u8],
        window_w: usize,
        window_h: usize,
        on_rgba: [u8; 4],
        off_rgba: [u8; 4],
    ) {
        if window_w == 0 || window_h == 0 || out.len() < window_w * window_h * 4 {
            return;
        }
        for wy in 0..window_h {
            let sy = wy * HEIGHT / window_h;
            for wx in 0..window_w {
                let sx = wx * WIDTH / window_w;
                let on = self.pixels[sy * WIDTH + sx];
                let c = if on { on_rgba } else { off_rgba };
                let i = (wy * window_w + wx) * 4;
                out[i] = c[0];
                out[i + 1] = c[1];
                out[i + 2] = c[2];
                out[i + 3] = c[3];
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invert_toggles() {
        let mut fb = Framebuffer::new();
        fb.pixel(10, 10);
        assert!(fb.get(10, 10));
        fb.invert_rect(10, 10, 1, 1);
        assert!(!fb.get(10, 10));
    }

    #[test]
    fn line_sets_endpoints() {
        let mut fb = Framebuffer::new();
        fb.line(0, 0, 10, 0);
        assert!(fb.get(0, 0));
        assert!(fb.get(10, 0));
        assert!(fb.get(5, 0));
    }
}
