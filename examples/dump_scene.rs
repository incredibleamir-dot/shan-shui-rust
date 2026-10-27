//! Dump a scene in the same text format as `tools/dump_scene.py`.
//!
//! cargo run --example dump_scene -- --seed 42 --width 480 --height 320

use shan_shui::rng::Seed;
use shan_shui::scene::{Scene, SceneOptions};
use shan_shui::shapes::Shape;

fn f(value: f64) -> String {
    format!("{value:.6}")
}

fn color(value: Option<shan_shui::Color>) -> String {
    match value {
        None => "none".to_string(),
        Some(c) => format!("{},{},{},{}", c.r, c.g, c.b, f(c.a)),
    }
}

/// Stable string quoting for the strings used by the generator.
fn stable_repr(text: &str) -> String {
    let has_single = text.contains('\'');
    let has_double = text.contains('"');
    let quote = if has_single && !has_double { '"' } else { '\'' };
    let mut out = String::new();
    out.push(quote);
    for ch in text.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c == quote => {
                out.push('\\');
                out.push(c);
            }
            c => out.push(c),
        }
    }
    out.push(quote);
    out
}

fn shape_text(shape: &Shape) -> String {
    match shape {
        Shape::Poly(poly) => {
            let pts: Vec<String> = poly
                .pts
                .iter()
                .take(6)
                .map(|(x, y)| format!("{},{}", f(*x), f(*y)))
                .collect();
            let more = if poly.pts.len() > 6 {
                format!(" +{}", poly.pts.len() - 6)
            } else {
                String::new()
            };
            format!(
                "poly n={} pts={}{} fill={} stroke={} width={}",
                poly.pts.len(),
                pts.join(" "),
                more,
                color(poly.fill),
                color(poly.stroke),
                f(poly.width)
            )
        }
        Shape::Circle(circle) => format!(
            "circle {},{} r={} fill={} stroke={} width={}",
            f(circle.cx),
            f(circle.cy),
            f(circle.r),
            color(circle.fill),
            color(circle.stroke),
            f(circle.width)
        ),
        Shape::GradientRect(g) => format!(
            "gradrect {},{} {}x{} top={} bottom={} vertical={}",
            f(g.x),
            f(g.y),
            f(g.w),
            f(g.h),
            color(Some(g.top)),
            color(Some(g.bottom)),
            g.vertical
        ),
        Shape::Text(t) => format!(
            "text {},{} size={} anchor={} angle={} text={}",
            f(t.x),
            f(t.y),
            f(t.size),
            t.anchor,
            f(t.angle),
            stable_repr(&t.text)
        ),
        Shape::Seal(s) => format!(
            "seal {},{} size={} text={}",
            f(s.x),
            f(s.y),
            f(s.size),
            stable_repr(&s.text)
        ),
    }
}

fn main() {
    let raw: Vec<String> = std::env::args().collect();
    let mut seed = "42".to_string();
    let mut theme = "ink".to_string();
    let mut camera = 0.0;
    let mut width = 480u32;
    let mut height = 320u32;
    let mut chunk_width = 512u32;
    let mut chunks = "-2,1".to_string();
    let mut items = false;
    let mut plans = false;
    let mut all = false;
    let mut vegetation = true;
    let mut architecture = 1.0;
    let mut people = 1.0;
    let mut boats = 1.0;
    let mut dust = 1.0;
    let mut water_style = "river".to_string();
    let mut sky = true;
    let mut sun = true;
    let mut mist = true;
    let mut birds = 5;
    let mut title = String::new();
    let mut seal = String::new();
    let mut title_size = 22.0;
    let mut i = 1;
    while i < raw.len() {
        match raw[i].as_str() {
            "--seed" => {
                i += 1;
                seed = raw[i].clone();
            }
            "--theme" => {
                i += 1;
                theme = raw[i].clone();
            }
            "--camera" => {
                i += 1;
                camera = raw[i].parse().unwrap();
            }
            "--width" => {
                i += 1;
                width = raw[i].parse().unwrap();
            }
            "--height" => {
                i += 1;
                height = raw[i].parse().unwrap();
            }
            "--chunk-width" => {
                i += 1;
                chunk_width = raw[i].parse().unwrap();
            }
            "--chunks" => {
                i += 1;
                chunks = raw[i].clone();
            }
            "--items" => items = true,
            "--plans" => plans = true,
            "--all" => all = true,
            "--vegetation" => vegetation = true,
            "--no-vegetation" => vegetation = false,
            "--architecture" => {
                i += 1;
                architecture = raw[i].parse().unwrap();
            }
            "--people" => {
                i += 1;
                people = raw[i].parse().unwrap();
            }
            "--boats" => {
                i += 1;
                boats = raw[i].parse().unwrap();
            }
            "--dust" => {
                i += 1;
                dust = raw[i].parse().unwrap();
            }
            "--water-style" => {
                i += 1;
                water_style = raw[i].clone();
            }
            "--sky" => sky = true,
            "--no-sky" => sky = false,
            "--sun" => sun = true,
            "--no-sun" => sun = false,
            "--mist" => mist = true,
            "--no-mist" => mist = false,
            "--birds" => {
                i += 1;
                birds = raw[i].parse().unwrap();
            }
            "--title" => {
                i += 1;
                title = raw[i].clone();
            }
            "--seal" => {
                i += 1;
                seal = raw[i].clone();
            }
            "--title-size" => {
                i += 1;
                title_size = raw[i].parse().unwrap();
            }
            other => panic!("unknown argument {other}"),
        }
        i += 1;
    }

    let seed = Seed::from_cli(&seed);
    let mut scene = Scene::new(SceneOptions {
        seed: Some(seed),
        theme,
        width,
        height,
        camera,
        chunk_width,
        vegetation,
        architecture,
        people,
        boats,
        dust,
        water_style,
        sky,
        sun,
        mist,
        birds,
        title,
        seal,
        title_size,
    })
    .unwrap();
    let shapes = scene.render_at(camera, None, None);
    println!("theme={}", scene.options.theme);
    println!("seed={}", scene.seed());
    println!("render_shapes={}", shapes.len());
    let parts: Vec<i32> = chunks.split(',').map(|p| p.parse().unwrap()).collect();
    for index in parts[0]..=parts[1] {
        let plan_list = if plans {
            Some(scene.debug_plans(index))
        } else {
            None
        };
        let chunk = scene.chunk(index);
        let total: usize = chunk.items.iter().map(|item| item.shapes.len()).sum();
        println!(
            "chunk {} xmin={} xmax={} items={} shapes={}",
            index,
            f(chunk.xmin),
            f(chunk.xmax),
            chunk.items.len(),
            total
        );
        if let Some(plan_list) = plan_list {
            for plan in plan_list {
                println!(
                    "  plan {} x={} y={} h={}",
                    plan.tag,
                    f(plan.x),
                    f(plan.y),
                    f(plan.h)
                );
            }
        }
        if !items {
            continue;
        }
        for item in &chunk.items {
            println!(
                "  item {} x={} y={} z={} count={}",
                item.tag,
                f(item.x),
                f(item.y),
                f(item.z),
                item.shapes.len()
            );
            if all {
                for shape in &item.shapes {
                    println!("    {}", shape_text(shape));
                }
            } else if let Some(shape) = item.shapes.first() {
                println!("    {}", shape_text(shape));
            }
        }
    }
}
