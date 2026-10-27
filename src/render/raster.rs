//! CPU rasteriser producing a bitmap that closely follows the reference
//! renderer's behaviour: aliased primitives drawn at a supersampled resolution
//! and then reduced with a Lanczos filter.

use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

use crate::color::{round_half_even, Color};
use crate::finish::Finish;
use crate::scene::Scene;
use crate::shapes::Shape;

/// A simple RGBA8 image.
#[derive(Clone, Debug, PartialEq)]
pub struct Bitmap {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

impl Bitmap {
    pub fn new(width: u32, height: u32, fill: Color) -> Self {
        let mut pixels = Vec::with_capacity((width as usize) * (height as usize) * 4);
        for _ in 0..(width as usize) * (height as usize) {
            pixels.extend_from_slice(&[fill.r, fill.g, fill.b, 255]);
        }
        Self {
            width,
            height,
            pixels,
        }
    }

    #[inline]
    fn index(&self, x: i32, y: i32) -> Option<usize> {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            return None;
        }
        Some(((y as usize) * (self.width as usize) + x as usize) * 4)
    }

    #[inline]
    pub fn blend_pixel(&mut self, x: i32, y: i32, color: Color, coverage: f32) {
        let Some(idx) = self.index(x, y) else {
            return;
        };
        let alpha = color.a * coverage as f64;
        if alpha <= 0.0 {
            return;
        }
        let inv = 1.0 - alpha;
        let p = &mut self.pixels[idx..idx + 4];
        p[0] = ((color.r as f64 * alpha) + (p[0] as f64 * inv)).round() as u8;
        p[1] = ((color.g as f64 * alpha) + (p[1] as f64 * inv)).round() as u8;
        p[2] = ((color.b as f64 * alpha) + (p[2] as f64 * inv)).round() as u8;
        p[3] = 255;
    }

    #[inline]
    fn blend_span(&mut self, y: i32, x0: i32, x1: i32, color: Color) {
        if y < 0 || y >= self.height as i32 {
            return;
        }
        let lo = x0.max(0);
        let hi = x1.min(self.width as i32 - 1);
        for x in lo..=hi {
            self.blend_pixel(x, y, color, 1.0);
        }
    }

    /// Fill a polygon using the even-odd rule, matching the reference polygon fill.
    pub fn fill_polygon(&mut self, pts: &[(f64, f64)], color: Color) {
        if pts.len() < 3 {
            return;
        }
        let mut ymin = f64::INFINITY;
        let mut ymax = f64::NEG_INFINITY;
        for p in pts {
            ymin = ymin.min(p.1);
            ymax = ymax.max(p.1);
        }
        let y0 = (((ymin - 0.5).ceil()) as i32).max(0);
        let y1 = (((ymax - 0.5).ceil()) as i32 - 1).min(self.height as i32 - 1);
        if y0 > y1 {
            return;
        }

        let edges: Vec<(f64, f64, f64, f64)> = pts
            .iter()
            .zip(pts.iter().cycle().skip(1))
            .take(pts.len())
            .map(|(a, b)| (a.0, a.1, b.0, b.1))
            .collect();

        let mut xs: Vec<f64> = Vec::with_capacity(edges.len());
        for y in y0..=y1 {
            let yc = y as f64 + 0.5;
            xs.clear();
            for &(x0, ya, x1, yb) in &edges {
                if (ya <= yc && yb > yc) || (yb <= yc && ya > yc) {
                    let t = (yc - ya) / (yb - ya);
                    xs.push(x0 + t * (x1 - x0));
                }
            }
            if xs.len() < 2 {
                continue;
            }
            xs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let mut k = 0;
            while k + 1 < xs.len() {
                let start = (xs[k] - 0.5).ceil() as i32;
                let end = (xs[k + 1] - 0.5).ceil() as i32 - 1;
                if end >= start {
                    self.blend_span(y, start, end, color);
                }
                k += 2;
            }
        }
    }

    fn fill_ellipse(&mut self, cx: f64, cy: f64, rx: f64, ry: f64, color: Color) {
        if rx <= 0.0 || ry <= 0.0 {
            return;
        }
        let y0 = ((cy - ry - 0.5).ceil() as i32).max(0);
        let y1 = ((cy + ry - 0.5).ceil() as i32 - 1).min(self.height as i32 - 1);
        for y in y0..=y1 {
            let ny = (y as f64 + 0.5 - cy) / ry;
            let inside = 1.0 - ny * ny;
            if inside <= 0.0 {
                continue;
            }
            let dx = rx * inside.sqrt();
            let x0 = ((cx - dx - 0.5).ceil() as i32).max(0);
            let x1 = ((cx + dx - 0.5).ceil() as i32 - 1).min(self.width as i32 - 1);
            if x1 >= x0 {
                self.blend_span(y, x0, x1, color);
            }
        }
    }

    /// Fill a thick polyline. Joints are rounded, matching the look of the
    /// reference renderer's wide outlines well enough for brush strokes.
    pub fn stroke_polyline(&mut self, pts: &[(f64, f64)], color: Color, width: f64, closed: bool) {
        if pts.is_empty() || width <= 0.0 {
            return;
        }
        let hw = (width * 0.5).max(0.35);
        let count = if closed {
            pts.len()
        } else {
            pts.len().saturating_sub(1)
        };
        for i in 0..count {
            let a = pts[i];
            let b = pts[(i + 1) % pts.len()];
            let dx = b.0 - a.0;
            let dy = b.1 - a.1;
            let len = dx.hypot(dy);
            if len <= 1e-9 {
                continue;
            }
            let nx = -dy / len * hw;
            let ny = dx / len * hw;
            let quad = [
                (a.0 + nx, a.1 + ny),
                (b.0 + nx, b.1 + ny),
                (b.0 - nx, b.1 - ny),
                (a.0 - nx, a.1 - ny),
            ];
            self.fill_polygon(&quad, color);
        }
        if closed || pts.len() > 2 {
            let join_end = if closed {
                pts.len()
            } else {
                pts.len().saturating_sub(1)
            };
            for p in pts.iter().take(join_end).skip(1) {
                self.fill_ellipse(p.0, p.1, hw, hw, color);
            }
        }
    }

    pub fn fill_rounded_rect(&mut self, x: f64, y: f64, w: f64, h: f64, radius: f64, color: Color) {
        let r = radius.max(0.0).min(w * 0.5).min(h * 0.5);
        if r <= 0.0 {
            self.fill_polygon(&[(x, y), (x + w, y), (x + w, y + h), (x, y + h)], color);
            return;
        }
        let steps = 8;
        let mut pts: Vec<(f64, f64)> = Vec::with_capacity(steps * 4 + 4);
        let corners = [
            (x + w - r, y + r, -std::f64::consts::FRAC_PI_2),
            (x + w - r, y + h - r, 0.0),
            (x + r, y + h - r, std::f64::consts::FRAC_PI_2),
            (x + r, y + r, std::f64::consts::PI),
        ];
        for (cx, cy, start) in corners {
            for k in 0..=steps {
                let a = start + (k as f64 / steps as f64) * std::f64::consts::FRAC_PI_2;
                pts.push((cx + r * a.cos(), cy + r * a.sin()));
            }
        }
        self.fill_polygon(&pts, color);
    }

    fn gradient_rect(
        &mut self,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        top: Color,
        bottom: Color,
        vertical: bool,
        xmin: f64,
        scale: f64,
    ) {
        if vertical {
            let x0 = (x - xmin) * scale;
            let x1 = (x + w - xmin) * scale;
            let y0 = y * scale;
            let y1 = (y + h) * scale;
            let rows = round_half_even(y1 - y0).max(1.0) as i32;
            for i in 0..rows {
                let t = i as f64 / (rows - 1).max(1) as f64;
                let color = top.mix(bottom, t);
                let yy = (y0 + i as f64) as i32;
                let sx = round_half_even(x0) as i32;
                let ex = round_half_even(x1) as i32;
                if yy < 0 || yy >= self.height as i32 {
                    continue;
                }
                for px in sx.max(0)..=ex.min(self.width as i32 - 1) {
                    self.blend_pixel(px, yy, color, 1.0);
                }
            }
        } else {
            let x0 = (x - xmin) * scale;
            let x1 = (x + w - xmin) * scale;
            let y0 = y * scale;
            let y1 = (y + h) * scale;
            let cols = round_half_even(x1 - x0).max(1.0) as i32;
            for i in 0..cols {
                let t = i as f64 / (cols - 1).max(1) as f64;
                let color = top.mix(bottom, t);
                let xx = (x0 + i as f64) as i32;
                if xx < 0 || xx >= self.width as i32 {
                    continue;
                }
                let sy = round_half_even(y0) as i32;
                let ey = round_half_even(y1) as i32;
                for py in sy.max(0)..=ey.min(self.height as i32 - 1) {
                    self.blend_pixel(xx, py, color, 1.0);
                }
            }
        }
    }
}

/// Rendering controls shared by the CLI and the studio UI.
#[derive(Clone, Debug, PartialEq)]
pub struct RenderOptions {
    pub xmin: f64,
    pub width: u32,
    pub height: u32,
    /// Pixels per scene unit. The CLI keeps this at 1 so output keeps the
    /// reference scale; the studio uses it to preview and export the same
    /// composition at different resolutions.
    pub scale: f64,
    pub paper_color: Option<Color>,
    pub supersample: u32,
    pub paper: bool,
    pub grain: bool,
    pub finish: Finish,
    pub grain_seed: u64,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            xmin: -600.0,
            width: 1200,
            height: 800,
            scale: 1.0,
            paper_color: None,
            supersample: 2,
            paper: true,
            grain: true,
            finish: Finish::Wash,
            grain_seed: 7,
        }
    }
}

pub fn render_scene(scene: &mut Scene, options: &RenderOptions) -> Bitmap {
    let scale = if options.scale > 1e-9 {
        options.scale
    } else {
        1.0
    };
    let world_width = options.width as f64 / scale;
    let world_height = options.height as f64 / scale;
    let shapes = scene.render(
        Some(options.xmin),
        Some(options.xmin + world_width),
        Some(world_width),
        Some(world_height),
    );
    render_shapes(&shapes, options)
}

pub fn render_shapes(shapes: &[Shape], options: &RenderOptions) -> Bitmap {
    render_shapes_when(shapes, options, || true)
        .expect("an unconditional render cannot be cancelled")
}

/// Rasterise a shape list, polling `keep_going` regularly so an interactive
/// preview can abandon a stale high-quality frame before it is finished.
pub fn render_shapes_when(
    shapes: &[Shape],
    options: &RenderOptions,
    mut keep_going: impl FnMut() -> bool,
) -> Option<Bitmap> {
    let ss = options.supersample.clamp(1, 8);
    let ss_f = ss as f64;
    let scale = if options.scale > 1e-9 {
        options.scale
    } else {
        1.0
    };
    let draw_scale = ss_f * scale;
    let width = options.width.max(1) * ss;
    let height = options.height.max(1) * ss;
    let paper = options
        .paper_color
        .unwrap_or_else(|| Color::new(247, 244, 236, 1.0));
    let mut bitmap = Bitmap::new(width, height, paper);

    for (index, shape) in shapes.iter().enumerate() {
        if index % 64 == 0 && !keep_going() {
            return None;
        }
        match shape {
            Shape::Poly(poly) => {
                if poly.pts.is_empty() {
                    continue;
                }
                let pts: Vec<(f64, f64)> = poly
                    .pts
                    .iter()
                    .map(|p| ((p.0 - options.xmin) * draw_scale, p.1 * draw_scale))
                    .collect();
                if let Some(fill) = poly.fill {
                    bitmap.fill_polygon(&pts, fill);
                }
                if let Some(stroke) = poly.stroke {
                    if poly.width > 0.0 {
                        let w = (round_half_even(poly.width * draw_scale)).max(1.0);
                        bitmap.stroke_polyline(&pts, stroke, w, true);
                    }
                }
            }
            Shape::Circle(circle) => {
                let cx = (circle.cx - options.xmin) * draw_scale;
                let cy = circle.cy * draw_scale;
                let r = circle.r * draw_scale;
                if let Some(fill) = circle.fill {
                    bitmap.fill_ellipse(cx, cy, r, r, fill);
                }
                if let Some(stroke) = circle.stroke {
                    if circle.width > 0.0 {
                        let w = (round_half_even(circle.width * draw_scale)).max(1.0);
                        let mut ring = Vec::with_capacity(65);
                        for k in 0..=64 {
                            let a = k as f64 / 64.0 * std::f64::consts::TAU;
                            ring.push((cx + r * a.cos(), cy + r * a.sin()));
                        }
                        bitmap.stroke_polyline(&ring, stroke, w, true);
                    }
                }
            }
            Shape::GradientRect(g) => {
                bitmap.gradient_rect(
                    g.x,
                    g.y,
                    g.w,
                    g.h,
                    g.top,
                    g.bottom,
                    g.vertical,
                    options.xmin,
                    draw_scale,
                );
            }
            Shape::Text(text) => {
                crate::render::text::draw_text(&mut bitmap, text, options.xmin, draw_scale);
            }
            Shape::Seal(seal) => {
                crate::render::text::draw_seal(&mut bitmap, seal, options.xmin, draw_scale);
            }
        }
    }

    if ss > 1 {
        bitmap = resize_lanczos(&bitmap, options.width.max(1), options.height.max(1));
        if !keep_going() {
            return None;
        }
    }

    if options.paper && options.grain {
        apply_grain(&mut bitmap, options.grain_seed);
    }
    apply_finish(&mut bitmap, options.finish);
    Some(bitmap)
}

fn lanczos(x: f64) -> f64 {
    if x.abs() < 1e-12 {
        return 1.0;
    }
    if x.abs() >= 3.0 {
        return 0.0;
    }
    let px = std::f64::consts::PI * x;
    3.0 * px.sin() * (px / 3.0).sin() / (px * px)
}

fn resize_lanczos(src: &Bitmap, dst_w: u32, dst_h: u32) -> Bitmap {
    if src.width == dst_w && src.height == dst_h {
        return src.clone();
    }
    let sw = src.width as usize;
    let sh = src.height as usize;
    let dw = dst_w as usize;
    let dh = dst_h as usize;

    // Precompute horizontal weights for every destination column.
    let scale_x = sw as f64 / dw as f64;
    let support_x = (3.0 * scale_x).max(3.0);
    let mut x_weights: Vec<Vec<(usize, f64)>> = Vec::with_capacity(dw);
    for i in 0..dw {
        let center = (i as f64 + 0.5) * scale_x - 0.5;
        let left = (center - support_x).ceil() as i64;
        let right = (center + support_x).floor() as i64;
        let mut weights = Vec::new();
        let mut total = 0.0;
        for j in left..=right {
            if j < 0 || j >= sw as i64 {
                continue;
            }
            let w = lanczos((j as f64 - center) / scale_x);
            if w != 0.0 {
                total += w;
                weights.push((j as usize, w));
            }
        }
        if total == 0.0 {
            weights.push((center.clamp(0.0, sw as f64 - 1.0) as usize, 1.0));
        } else {
            for item in &mut weights {
                item.1 /= total;
            }
        }
        x_weights.push(weights);
    }

    let mut tmp = vec![0.0f32; dw * sh * 4];
    for y in 0..sh {
        for (x, weights) in x_weights.iter().enumerate() {
            let mut acc = [0.0f32; 4];
            for &(sx, w) in weights {
                let idx = (y * sw + sx) * 4;
                for c in 0..4 {
                    acc[c] += src.pixels[idx + c] as f32 * w as f32;
                }
            }
            let out = (y * dw + x) * 4;
            for c in 0..4 {
                tmp[out + c] = acc[c];
            }
        }
    }

    // Vertical pass.
    let scale_y = sh as f64 / dh as f64;
    let support_y = (3.0 * scale_y).max(3.0);
    let mut out = Bitmap {
        width: dst_w,
        height: dst_h,
        pixels: vec![0u8; dw * dh * 4],
    };
    for j in 0..dh {
        let center = (j as f64 + 0.5) * scale_y - 0.5;
        let left = (center - support_y).ceil() as i64;
        let right = (center + support_y).floor() as i64;
        let mut weights: Vec<(usize, f64)> = Vec::new();
        let mut total = 0.0;
        for k in left..=right {
            if k < 0 || k >= sh as i64 {
                continue;
            }
            let w = lanczos((k as f64 - center) / scale_y);
            if w != 0.0 {
                total += w;
                weights.push((k as usize, w));
            }
        }
        if total == 0.0 {
            weights.push((center.clamp(0.0, sh as f64 - 1.0) as usize, 1.0));
        } else {
            for item in &mut weights {
                item.1 /= total;
            }
        }
        for x in 0..dw {
            let mut acc = [0.0f32; 4];
            for &(sy, w) in &weights {
                let idx = (sy * dw + x) * 4;
                for c in 0..4 {
                    acc[c] += tmp[idx + c] * w as f32;
                }
            }
            let idx = (j * dw + x) * 4;
            for c in 0..4 {
                out.pixels[idx + c] = acc[c].clamp(0.0, 255.0).round() as u8;
            }
            out.pixels[idx + 3] = 255;
        }
    }
    out
}

fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E3779B97F4A7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
    z ^ (z >> 31)
}

fn uniform(state: &mut u64) -> f64 {
    (splitmix64(state) >> 11) as f64 / (1u64 << 53) as f64
}

fn apply_grain(bitmap: &mut Bitmap, seed: u64) {
    let mut state = seed ^ 0xC0FFEE_u64;
    for y in 0..bitmap.height {
        for x in 0..bitmap.width {
            // Box-Muller, sigma 7 around 128, matching the reference grain model.
            let u1 = uniform(&mut state).max(1e-9);
            let u2 = uniform(&mut state);
            let gaussian = (-2.0 * u1.ln()).sqrt() * (std::f64::consts::TAU * u2).cos();
            let noise = (128.0 + 7.0 * gaussian).clamp(0.0, 255.0);
            let grain = 232.0 + (noise * 23.0 / 255.0).floor();
            let factor = grain / 255.0;
            let idx = ((y as usize) * bitmap.width as usize + x as usize) * 4;
            for c in 0..3 {
                let v = bitmap.pixels[idx + c] as f64;
                let textured = v * factor;
                bitmap.pixels[idx + c] =
                    (v * 0.35 + textured * 0.65).clamp(0.0, 255.0).round() as u8;
            }
        }
    }
}

fn apply_finish(bitmap: &mut Bitmap, finish: Finish) {
    if finish == Finish::Wash {
        return;
    }
    for px in bitmap.pixels.chunks_exact_mut(4) {
        let r = px[0] as f64;
        let g = px[1] as f64;
        let b = px[2] as f64;
        let (mut nr, mut ng, mut nb) = (r, g, b);
        match finish {
            Finish::Wash => {}
            Finish::Crisp => {
                nr = 128.0 + (r - 128.0) * 1.06;
                ng = 128.0 + (g - 128.0) * 1.06;
                nb = 128.0 + (b - 128.0) * 1.06;
                let luma = 0.299 * nr + 0.587 * ng + 0.114 * nb;
                nr = luma + (nr - luma) * 1.06;
                ng = luma + (ng - luma) * 1.06;
                nb = luma + (nb - luma) * 1.06;
            }
            Finish::Vivid => {
                let luma = 0.299 * r + 0.587 * g + 0.114 * b;
                nr = luma + (r - luma) * 1.32;
                ng = luma + (g - luma) * 1.32;
                nb = luma + (b - luma) * 1.32;
                nr = 128.0 + (nr - 128.0) * 1.08;
                ng = 128.0 + (ng - 128.0) * 1.08;
                nb = 128.0 + (nb - 128.0) * 1.08;
            }
            Finish::Vintage => {
                let sr = 0.393 * r + 0.769 * g + 0.189 * b;
                let sg = 0.349 * r + 0.686 * g + 0.168 * b;
                let sb = 0.272 * r + 0.534 * g + 0.131 * b;
                nr = r * 0.62 + sr * 0.38;
                ng = g * 0.62 + sg * 0.38;
                nb = b * 0.62 + sb * 0.38;
            }
            Finish::Noir => {
                let luma = 0.299 * r + 0.587 * g + 0.114 * b;
                nr = 128.0 + (luma - 128.0) * 1.15;
                ng = nr;
                nb = nr;
            }
        }
        px[0] = nr.clamp(0.0, 255.0).round() as u8;
        px[1] = ng.clamp(0.0, 255.0).round() as u8;
        px[2] = nb.clamp(0.0, 255.0).round() as u8;
    }
}

/// Write an RGB PNG using the pure-Rust encoder.
pub fn save_png(path: &Path, bitmap: &Bitmap) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
    }
    let mut rgb = Vec::with_capacity((bitmap.width as usize) * (bitmap.height as usize) * 3);
    for pixel in bitmap.pixels.chunks_exact(4) {
        rgb.extend_from_slice(&pixel[..3]);
    }
    let file = File::create(path).map_err(|e| e.to_string())?;
    let writer = BufWriter::new(file);
    let mut encoder = png::Encoder::new(writer, bitmap.width, bitmap.height);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    let mut png_writer = encoder.write_header().map_err(|e| e.to_string())?;
    png_writer
        .write_image_data(&rgb)
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shapes::Circle;

    #[test]
    fn a_cancelled_render_stops_between_shapes() {
        let shapes: Vec<Shape> = (0..130)
            .map(|index| {
                Shape::Circle(Circle::filled(
                    8.0 + index as f64 * 0.4,
                    16.0,
                    2.0,
                    Color::new(30, 30, 30, 1.0),
                ))
            })
            .collect();
        let options = RenderOptions {
            xmin: 0.0,
            width: 64,
            height: 32,
            supersample: 1,
            paper: false,
            grain: false,
            ..RenderOptions::default()
        };

        let mut polls = 0;
        let cancelled = render_shapes_when(&shapes, &options, || {
            polls += 1;
            polls < 2
        });
        assert!(cancelled.is_none());
        assert_eq!(polls, 2);

        let rendered = render_shapes_when(&shapes, &options, || true).expect("uncancelled render");
        assert_eq!((rendered.width, rendered.height), (64, 32));
    }
}
