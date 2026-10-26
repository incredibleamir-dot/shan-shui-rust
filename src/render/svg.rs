//! SVG export, mirroring the original `render_svg.py` output.

use std::fmt::Write as _;
use std::path::Path;

use crate::color::Color;
use crate::shapes::Shape;

fn num(value: f64) -> String {
    let mut s = format!("{:.2}", value);
    while s.ends_with('0') {
        s.pop();
    }
    if s.ends_with('.') {
        s.pop();
    }
    if s.is_empty() || s == "-" {
        s.push('0');
    }
    s
}

fn color(value: Option<Color>) -> String {
    match value {
        None => "none".to_string(),
        Some(c) if c.a >= 1.0 => format!("rgb({},{},{})", c.r, c.g, c.b),
        Some(c) => format!("rgba({},{},{},{:.3})", c.r, c.g, c.b, c.a),
    }
}

fn points(pts: &[(f64, f64)]) -> String {
    let mut out = String::new();
    for (i, (x, y)) in pts.iter().enumerate() {
        if i > 0 {
            out.push(' ');
        }
        let _ = write!(out, "{},{}", num(*x), num(*y));
    }
    out
}

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn text_anchor(anchor: &str) -> &str {
    match anchor {
        "middle" => "middle",
        "start" => "start",
        "end" => "end",
        _ => "middle",
    }
}

fn shape_svg(shape: &Shape, defs: &mut Vec<String>, grad_index: &mut usize) -> String {
    match shape {
        Shape::Poly(poly) => {
            if poly.pts.is_empty() {
                return String::new();
            }
            let mut attrs = vec![
                format!("points=\"{}\"", points(&poly.pts)),
                format!("fill=\"{}\"", color(poly.fill)),
            ];
            if let Some(stroke) = poly.stroke {
                if poly.width > 0.0 {
                    attrs.push(format!("stroke=\"{}\"", color(Some(stroke))));
                    attrs.push(format!("stroke-width=\"{}\"", num(poly.width)));
                }
            }
            format!("<polygon {}/>", attrs.join(" "))
        }
        Shape::Circle(circle) => {
            let mut attrs = vec![
                format!("cx=\"{}\"", num(circle.cx)),
                format!("cy=\"{}\"", num(circle.cy)),
                format!("r=\"{}\"", num(circle.r)),
                format!("fill=\"{}\"", color(circle.fill)),
            ];
            if let Some(stroke) = circle.stroke {
                if circle.width > 0.0 {
                    attrs.push(format!("stroke=\"{}\"", color(Some(stroke))));
                    attrs.push(format!("stroke-width=\"{}\"", num(circle.width)));
                }
            }
            format!("<circle {}/>", attrs.join(" "))
        }
        Shape::GradientRect(g) => {
            let gid = format!("grad{}", *grad_index);
            *grad_index += 1;
            let coords = if g.vertical {
                "x1=\"0\" y1=\"0\" x2=\"0\" y2=\"1\""
            } else {
                "x1=\"0\" y1=\"0\" x2=\"1\" y2=\"0\""
            };
            defs.push(format!(
                "<linearGradient id=\"{}\" {}><stop offset=\"0%\" stop-color=\"{}\"/><stop offset=\"100%\" stop-color=\"{}\"/></linearGradient>",
                gid,
                coords,
                color(Some(g.top)),
                color(Some(g.bottom))
            ));
            format!(
                "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"url(#{})\"/>",
                num(g.x),
                num(g.y),
                num(g.w),
                num(g.h),
                gid
            )
        }
        Shape::Text(t) => {
            let x = num(t.x);
            let y = num(t.y);
            let attrs = [
                format!("x=\"{}\"", x),
                format!("y=\"{}\"", y),
                format!("font-size=\"{}\"", num(t.size)),
                format!("fill=\"{}\"", color(Some(t.color))),
                format!(
                    "font-family=\"{}, Songti SC, STSong, SimSun, serif\"",
                    escape(&t.family)
                ),
                format!("text-anchor=\"{}\"", text_anchor(&t.anchor)),
                "dominant-baseline=\"middle\"".to_string(),
            ];
            let transform = if t.angle != 0.0 {
                format!(" transform=\"rotate({} {} {})\"", num(t.angle), x, y)
            } else {
                String::new()
            };
            format!(
                "<text {}{}>{}</text>",
                attrs.join(" "),
                transform,
                escape(&t.text)
            )
        }
        Shape::Seal(s) => {
            let x = s.x;
            let y = s.y;
            let size = s.size;
            format!(
                "<g><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{}\" fill=\"{}\"/><text x=\"{}\" y=\"{}\" font-size=\"{}\" font-weight=\"bold\" fill=\"{}\" text-anchor=\"middle\" dominant-baseline=\"middle\" font-family=\"Songti SC, STSong, SimSun, serif\">{}</text></g>",
                num(x),
                num(y),
                num(size),
                num(size),
                num(size * 0.12),
                color(Some(s.color)),
                num(x + size / 2.0),
                num(y + size / 2.0 + size * 0.03),
                num(size * 0.48),
                color(Some(s.text_color)),
                escape(&s.text)
            )
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn to_svg(
    shapes: &[Shape],
    xmin: f64,
    width: f64,
    height: f64,
    paper: bool,
    paper_color: Option<Color>,
    grain: bool,
) -> String {
    to_svg_sized(
        shapes,
        xmin,
        width,
        height,
        width as u32,
        height as u32,
        paper,
        paper_color,
        grain,
    )
}

/// Like [`to_svg`], but the SVG viewport keeps scene units while the width and
/// height attributes describe the requested output resolution.
#[allow(clippy::too_many_arguments)]
pub fn to_svg_sized(
    shapes: &[Shape],
    xmin: f64,
    width: f64,
    height: f64,
    output_width: u32,
    output_height: u32,
    paper: bool,
    paper_color: Option<Color>,
    grain: bool,
) -> String {
    let mut defs: Vec<String> = Vec::new();
    let mut grad_index = 0usize;
    let body: Vec<String> = shapes
        .iter()
        .map(|shape| shape_svg(shape, &mut defs, &mut grad_index))
        .collect();

    if grain {
        defs.push(
            "<filter id=\"paperGrain\" x=\"0\" y=\"0\" width=\"100%\" height=\"100%\"><feTurbulence type=\"fractalNoise\" baseFrequency=\"0.9\" numOctaves=\"2\" seed=\"7\" stitchTiles=\"stitch\"/><feColorMatrix type=\"saturate\" values=\"0\"/><feComponentTransfer><feFuncA type=\"linear\" slope=\"0.55\"/></feComponentTransfer></filter>".to_string(),
        );
    }

    let mut out = String::new();
    let _ = write!(
        out,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"{} 0 {} {}\" width=\"{}\" height=\"{}\">",
        num(xmin),
        num(width),
        num(height),
        output_width.max(1),
        output_height.max(1)
    );
    if !defs.is_empty() {
        out.push_str("<defs>");
        for d in &defs {
            out.push_str(d);
        }
        out.push_str("</defs>");
    }
    if paper {
        let fill = match paper_color {
            Some(c) => color(Some(c)),
            None => "#ffffff".to_string(),
        };
        let _ = write!(
            out,
            "<rect x=\"{}\" y=\"0\" width=\"{}\" height=\"{}\" fill=\"{}\"/>",
            num(xmin),
            num(width),
            num(height),
            fill
        );
    }
    for item in body {
        out.push_str(&item);
    }
    if paper && grain {
        let _ = write!(
            out,
            "<rect x=\"{}\" y=\"0\" width=\"{}\" height=\"{}\" filter=\"url(#paperGrain)\" opacity=\"0.16\" style=\"mix-blend-mode:multiply\"/>",
            num(xmin),
            num(width),
            num(height)
        );
    }
    out.push_str("</svg>");
    out
}

pub fn save_svg(
    path: &Path,
    shapes: &[Shape],
    xmin: f64,
    width: f64,
    height: f64,
    paper: bool,
    paper_color: Option<Color>,
    grain: bool,
) -> Result<(), String> {
    save_svg_sized(
        path,
        shapes,
        xmin,
        width,
        height,
        width as u32,
        height as u32,
        paper,
        paper_color,
        grain,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn save_svg_sized(
    path: &Path,
    shapes: &[Shape],
    xmin: f64,
    width: f64,
    height: f64,
    output_width: u32,
    output_height: u32,
    paper: bool,
    paper_color: Option<Color>,
    grain: bool,
) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
    }
    std::fs::write(
        path,
        to_svg_sized(
            shapes,
            xmin,
            width,
            height,
            output_width,
            output_height,
            paper,
            paper_color,
            grain,
        ),
    )
    .map_err(|e| e.to_string())
}
