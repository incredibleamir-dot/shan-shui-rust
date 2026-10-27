//! Text, seal and font handling.
//!
//! The renderer looks for a CJK serif face in the system font directories
//! (SimSun/Microsoft YaHei on Windows, Songti/PingFang on macOS, Noto CJK or
//! WenQuanYi on Linux) and loads it through `ab_glyph`.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use ab_glyph::{Font, FontVec, PxScale, ScaleFont};

use crate::color::Color;
use crate::render::raster::Bitmap;
use crate::shapes::{Seal, Text};

fn font_cache() -> &'static Mutex<HashMap<String, Option<Arc<FontVec>>>> {
    static CACHE: OnceLock<Mutex<HashMap<String, Option<Arc<FontVec>>>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// System font directories searched for inscription and seal faces.
pub fn font_dirs() -> Vec<std::path::PathBuf> {
    let mut dirs = Vec::new();
    if let Some(windir) = std::env::var_os("WINDIR") {
        dirs.push(std::path::PathBuf::from(windir).join("Fonts"));
    }
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        dirs.push(
            std::path::PathBuf::from(local)
                .join("Microsoft")
                .join("Windows")
                .join("Fonts"),
        );
    }
    for dir in [
        "/System/Library/Fonts",
        "/System/Library/Fonts/Supplemental",
        "/Library/Fonts",
        "/usr/share/fonts",
        "/usr/share/fonts/opentype/noto",
        "/usr/share/fonts/truetype/noto",
        "/usr/share/fonts/truetype/wqy",
        "/usr/share/fonts/truetype/arphic",
        "/usr/share/fonts/truetype/dejavu",
        "/usr/share/fonts/TTF",
        "/usr/share/fonts/OTF",
        "/usr/local/share/fonts",
    ] {
        dirs.push(std::path::PathBuf::from(dir));
    }
    if let Some(home) = std::env::var_os("HOME") {
        let home = std::path::PathBuf::from(home);
        dirs.push(home.join("Library").join("Fonts"));
        dirs.push(home.join(".local").join("share").join("fonts"));
        dirs.push(home.join(".fonts"));
    }
    if let Some(data) = std::env::var_os("XDG_DATA_HOME") {
        dirs.push(std::path::PathBuf::from(data).join("fonts"));
    }
    dirs
}

fn candidate_names(family: &str) -> &'static [&'static str] {
    if family.to_lowercase().contains("verdana") {
        &[
            "verdana.ttf",
            "Verdana.ttf",
            "arial.ttf",
            "Arial.ttf",
            "segoeui.ttf",
            "Helvetica.ttc",
            "DejaVuSans.ttf",
        ]
    } else {
        &[
            "simsun.ttc",
            "msyh.ttc",
            "simhei.ttf",
            "NotoSerifCJK-Regular.ttc",
            "NotoSerifCJK.ttc",
            "NotoSansCJK-Regular.ttc",
            "NotoSansCJK.ttc",
            "Songti.ttc",
            "STSong.ttf",
            "STHeiti Light.ttc",
            "PingFang.ttc",
            "wqy-zenhei.ttc",
            "wqy-microhei.ttc",
            "uming.ttc",
            "ukai.ttc",
            "arialuni.ttf",
            "Arial Unicode.ttf",
            "DejaVuSans.ttf",
        ]
    }
}

fn load_font_file(path: &std::path::Path) -> Option<FontVec> {
    let bytes = std::fs::read(path).ok()?;
    let is_collection = path
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("ttc"));
    if is_collection {
        FontVec::try_from_vec_and_index(bytes, 0).ok()
    } else {
        FontVec::try_from_vec(bytes).ok()
    }
}

fn load_font(family: &str) -> Option<Arc<FontVec>> {
    let key = if family.to_lowercase().contains("verdana") {
        "verdana".to_string()
    } else {
        "serif".to_string()
    };
    let mut cache = font_cache().lock().ok()?;
    if let Some(entry) = cache.get(&key) {
        return entry.clone();
    }
    let dirs = font_dirs();
    let mut found: Option<Arc<FontVec>> = None;
    'search: for name in candidate_names(family) {
        for dir in &dirs {
            if let Some(font) = load_font_file(&dir.join(name)) {
                found = Some(Arc::new(font));
                break 'search;
            }
        }
    }
    cache.insert(key, found.clone());
    found
}

/// Width, ascent and descent (both positive magnitudes) at a given pixel size.
pub fn text_metrics(family: &str, text: &str, size: f32) -> Option<(f32, f32, f32)> {
    let font = load_font(family)?;
    let scaled = font.as_scaled(PxScale::from(size.max(1.0)));
    let mut width = 0.0;
    for ch in text.chars() {
        width += scaled.h_advance(font.glyph_id(ch));
    }
    Some((width, scaled.ascent(), scaled.descent().abs()))
}

pub fn font_family_label(family: &str) -> &'static str {
    if family.to_lowercase().contains("verdana") {
        "Verdana"
    } else {
        "SimSun"
    }
}

fn draw_glyphs_into_bitmap(
    bitmap: &mut Bitmap,
    font: &FontVec,
    text: &str,
    left: f64,
    baseline: f64,
    size: f32,
    color: Color,
) {
    let scaled = font.as_scaled(PxScale::from(size));
    let mut pen = left;
    for ch in text.chars() {
        let id = font.glyph_id(ch);
        let glyph = id.with_scale_and_position(
            PxScale::from(size),
            ab_glyph::point(pen as f32, baseline as f32),
        );
        if let Some(outlined) = font.outline_glyph(glyph) {
            let bounds = outlined.px_bounds();
            outlined.draw(|gx, gy, coverage| {
                let x = bounds.min.x.round() as i32 + gx as i32;
                let y = bounds.min.y.round() as i32 + gy as i32;
                if coverage > 0.0 {
                    bitmap.blend_pixel(x, y, color, coverage);
                }
            });
        }
        pen += scaled.h_advance(id) as f64;
    }
}

fn draw_glyphs_into_layer(
    layer: &mut [f32],
    width: usize,
    height: usize,
    font: &FontVec,
    text: &str,
    left: f64,
    baseline: f64,
    size: f32,
) {
    let scaled = font.as_scaled(PxScale::from(size));
    let mut pen = left;
    for ch in text.chars() {
        let id = font.glyph_id(ch);
        let glyph = id.with_scale_and_position(
            PxScale::from(size),
            ab_glyph::point(pen as f32, baseline as f32),
        );
        if let Some(outlined) = font.outline_glyph(glyph) {
            let bounds = outlined.px_bounds();
            outlined.draw(|gx, gy, coverage| {
                let x = bounds.min.x.round() as i32 + gx as i32;
                let y = bounds.min.y.round() as i32 + gy as i32;
                if x < 0 || y < 0 || x >= width as i32 || y >= height as i32 {
                    return;
                }
                let idx = y as usize * width + x as usize;
                layer[idx] = (layer[idx] + coverage).min(1.0);
            });
        }
        pen += scaled.h_advance(id) as f64;
    }
}

fn rotated_composite(
    bitmap: &mut Bitmap,
    layer: &[f32],
    lw: usize,
    lh: usize,
    cx: f64,
    cy: f64,
    angle_deg: f64,
    color: Color,
) {
    let angle = angle_deg.to_radians();
    let (sin, cos) = angle.sin_cos();
    let half_w = lw as f64 / 2.0;
    let half_h = lh as f64 / 2.0;
    let radius = (half_w * half_w + half_h * half_h).sqrt() + 1.0;
    let x0 = (cx - radius).floor() as i32;
    let x1 = (cx + radius).ceil() as i32;
    let y0 = (cy - radius).floor() as i32;
    let y1 = (cy + radius).ceil() as i32;
    for py in y0..=y1 {
        for px in x0..=x1 {
            let dx = px as f64 + 0.5 - cx;
            let dy = py as f64 + 0.5 - cy;
            let lx = cos * dx + sin * dy + half_w;
            let ly = -sin * dx + cos * dy + half_h;
            if lx < 0.0 || ly < 0.0 || lx >= lw as f64 || ly >= lh as f64 {
                continue;
            }
            // Bilinear sample of the coverage layer.
            let sx = lx - 0.5;
            let sy = ly - 0.5;
            let sx0 = sx.floor();
            let sy0 = sy.floor();
            let fx = sx - sx0;
            let fy = sy - sy0;
            let sample = |ix: i32, iy: i32| -> f64 {
                if ix < 0 || iy < 0 || ix >= lw as i32 || iy >= lh as i32 {
                    0.0
                } else {
                    layer[iy as usize * lw + ix as usize] as f64
                }
            };
            let ix = sx0 as i32;
            let iy = sy0 as i32;
            let c00 = sample(ix, iy);
            let c10 = sample(ix + 1, iy);
            let c01 = sample(ix, iy + 1);
            let c11 = sample(ix + 1, iy + 1);
            let coverage =
                (c00 * (1.0 - fx) + c10 * fx) * (1.0 - fy) + (c01 * (1.0 - fx) + c11 * fx) * fy;
            if coverage > 0.001 {
                bitmap.blend_pixel(px, py, color, coverage as f32);
            }
        }
    }
}

pub fn draw_text(bitmap: &mut Bitmap, shape: &Text, xmin: f64, draw_scale: f64) {
    let Some(font) = load_font(&shape.family) else {
        return;
    };
    let size = ((shape.size * draw_scale).max(1.0)) as f32;
    let scaled = font.as_scaled(PxScale::from(size));
    let ascent = scaled.ascent();
    let descent = scaled.descent();
    let mut width = 0.0f32;
    for ch in shape.text.chars() {
        width += scaled.h_advance(font.glyph_id(ch));
    }
    let line_h = ascent - descent;
    let x = (shape.x - xmin) * draw_scale;
    let y = shape.y * draw_scale;

    if shape.angle.abs() < 0.001 {
        let left = match shape.anchor.as_str() {
            "start" => x,
            "end" => x - width as f64,
            _ => x - width as f64 / 2.0,
        };
        let baseline = y + (ascent + descent) as f64 / 2.0;
        draw_glyphs_into_bitmap(
            bitmap,
            &font,
            &shape.text,
            left,
            baseline,
            size,
            shape.color,
        );
        return;
    }

    let pad = 2.0f64;
    let lw = (width as f64 + pad * 2.0).ceil().max(1.0) as usize;
    let lh = (line_h as f64 + pad * 2.0).ceil().max(1.0) as usize;
    let mut layer = vec![0.0f32; lw * lh];
    draw_glyphs_into_layer(
        &mut layer,
        lw,
        lh,
        &font,
        &shape.text,
        pad,
        pad + ascent as f64,
        size,
    );
    rotated_composite(bitmap, &layer, lw, lh, x, y, shape.angle, shape.color);
}

pub fn draw_seal(bitmap: &mut Bitmap, seal: &Seal, xmin: f64, draw_scale: f64) {
    let s = seal.size * draw_scale;
    let x = (seal.x - xmin) * draw_scale;
    let y = seal.y * draw_scale;
    bitmap.fill_rounded_rect(x, y, s, s, s * 0.12, seal.color);

    let Some(font) = load_font("serif") else {
        return;
    };
    let size = ((seal.size * 0.46 * draw_scale).max(1.0)) as f32;
    let scaled = font.as_scaled(PxScale::from(size));
    let ascent = scaled.ascent();
    let descent = scaled.descent();
    let mut width = 0.0f32;
    for ch in seal.text.chars() {
        width += scaled.h_advance(font.glyph_id(ch));
    }
    let cx = x + s / 2.0;
    let cy = y + s / 2.0 + s * 0.03;
    let left = cx - width as f64 / 2.0;
    let baseline = cy + (ascent + descent) as f64 / 2.0;
    draw_glyphs_into_bitmap(
        bitmap,
        &font,
        &seal.text,
        left,
        baseline,
        size,
        seal.text_color,
    );
}
