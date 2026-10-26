//! Command line front end matching the original Python `shan-shui` CLI, with
//! an extra `--finish` preset and a few studio-oriented options.

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

use shan_shui::finish::Finish;
use shan_shui::render::{render_shapes, save_png, save_svg, RenderOptions};
use shan_shui::rng::Seed;
use shan_shui::scene::{Scene, SceneOptions};
use shan_shui::{get_palette, theme_names, VERSION};

const HELP: &str = "\
Generate procedural Chinese landscape paintings (port of {Shan, Shui}*).

Usage: shan-shui [OPTIONS]

Options:
      --version                  print version and exit
      --theme <THEME>            colour theme [default: ink]
      --list-themes              list themes and exit
      --seed <SEED>              integer or text seed
      --camera <CAMERA>          horizontal centre of the view [default: 0]
      --width <WIDTH>            image width in pixels [default: 1200]
      --height <HEIGHT>          image height in pixels [default: 800]
  -f, --format <FORMAT>          png, svg or both [default: both]
  -o, --out <OUT>                output file or base path [default: out/shan-shui]
      --supersample <FACTOR>     PNG supersampling factor [default: 2]
      --title <TITLE>            optional inscription
      --seal <SEAL>              optional red seal text
      --title-size <SIZE>        inscription font size [default: 22]
      --finish <PRESET>          wash, crisp, vivid, vintage or noir [default: wash]
      --paper, --no-paper        paint the paper background [default: paper]
      --grain, --no-grain        paper grain [default: grain]
      --sky, --no-sky            sky gradient [default: sky]
      --sun, --no-sun            sun [default: sun]
      --mist, --no-mist          mist [default: mist]
      --vegetation, --no-vegetation
                                 trees and greenery [default: vegetation]
      --architecture <DENSITY>   0..1 building density [default: 1]
      --people <DENSITY>         0..1 people density [default: 1]
      --boats <DENSITY>          0..1 boat density [default: 1]
      --dust <DENSITY>           0..1 dust density [default: 1]
      --birds <COUNT>            number of birds [default: 5]
      --chunk-width <PIXELS>     generation chunk width [default: 512]
  -h, --help                     print help
";

struct Args {
    theme: String,
    seed: Option<Seed>,
    camera: f64,
    width: u32,
    height: u32,
    formats: Vec<&'static str>,
    out: String,
    supersample: u32,
    title: String,
    seal: String,
    title_size: f64,
    paper: bool,
    grain: bool,
    mist: bool,
    sun: bool,
    vegetation: bool,
    architecture: f64,
    people: f64,
    boats: f64,
    dust: f64,
    birds: i32,
    chunk_width: u32,
    sky: bool,
    finish: Finish,
    list_themes: bool,
}

impl Default for Args {
    fn default() -> Self {
        Self {
            theme: "ink".into(),
            seed: None,
            camera: 0.0,
            width: 1200,
            height: 800,
            formats: vec!["png", "svg"],
            out: "out/shan-shui".into(),
            supersample: 2,
            title: String::new(),
            seal: String::new(),
            title_size: 22.0,
            paper: true,
            grain: true,
            mist: true,
            sun: true,
            vegetation: true,
            architecture: 1.0,
            people: 1.0,
            boats: 1.0,
            dust: 1.0,
            birds: 5,
            chunk_width: 512,
            sky: true,
            finish: Finish::Wash,
            list_themes: false,
        }
    }
}

fn parse_value(
    args: &[String],
    index: &mut usize,
    inline: Option<String>,
    name: &str,
) -> Result<String, String> {
    if let Some(value) = inline {
        return Ok(value);
    }
    *index += 1;
    args.get(*index)
        .cloned()
        .ok_or_else(|| format!("argument {name}: expected one argument"))
}

fn parse_f64(value: &str, name: &str) -> Result<f64, String> {
    value
        .parse::<f64>()
        .map_err(|_| format!("argument {name}: invalid float value: '{value}'"))
}

fn parse_i64(value: &str, name: &str) -> Result<i64, String> {
    value
        .parse::<i64>()
        .map_err(|_| format!("argument {name}: invalid integer value: '{value}'"))
}

fn parse_u32(value: &str, name: &str) -> Result<u32, String> {
    let parsed = parse_i64(value, name)?;
    if parsed < 0 {
        return Err(format!("argument {name}: expected a positive integer"));
    }
    Ok(parsed as u32)
}

fn parse_density(value: &str, name: &str) -> Result<f64, String> {
    Ok(parse_f64(value, name)?.clamp(0.0, 1.0))
}

fn parse_args(raw: &[String]) -> Result<Args, String> {
    let mut args = Args::default();
    let mut index = 1usize;
    while index < raw.len() {
        let item = &raw[index];
        let (key, inline) = match item.split_once('=') {
            Some((key, value)) => (key, Some(value.to_string())),
            None => (item.as_str(), None),
        };
        match key {
            "-h" | "--help" => {
                print!("{HELP}");
                std::process::exit(0);
            }
            "--version" => {
                println!("shan-shui {VERSION}");
                std::process::exit(0);
            }
            "--list-themes" => args.list_themes = true,
            "--theme" => {
                args.theme = parse_value(raw, &mut index, inline, key)?;
            }
            "--seed" => {
                let value = parse_value(raw, &mut index, inline, key)?;
                args.seed = Some(Seed::from_cli(&value));
            }
            "--camera" => {
                args.camera = parse_f64(&parse_value(raw, &mut index, inline, key)?, key)?;
            }
            "--width" => {
                args.width = parse_u32(&parse_value(raw, &mut index, inline, key)?, key)?;
            }
            "--height" => {
                args.height = parse_u32(&parse_value(raw, &mut index, inline, key)?, key)?;
            }
            "-f" | "--format" => {
                let value = parse_value(raw, &mut index, inline, key)?;
                args.formats = match value.as_str() {
                    "png" => vec!["png"],
                    "svg" => vec!["svg"],
                    "both" => vec!["png", "svg"],
                    other => {
                        return Err(format!(
                            "argument {key}: invalid choice: '{other}' (choose from 'png', 'svg', 'both')"
                        ))
                    }
                };
            }
            "-o" | "--out" => {
                args.out = parse_value(raw, &mut index, inline, key)?;
            }
            "--supersample" => {
                args.supersample = parse_u32(&parse_value(raw, &mut index, inline, key)?, key)?;
                if args.supersample < 1 {
                    return Err("argument --supersample: must be at least 1".into());
                }
            }
            "--title" => args.title = parse_value(raw, &mut index, inline, key)?,
            "--seal" => args.seal = parse_value(raw, &mut index, inline, key)?,
            "--title-size" => {
                args.title_size = parse_f64(&parse_value(raw, &mut index, inline, key)?, key)?;
            }
            "--finish" => {
                let value = parse_value(raw, &mut index, inline, key)?;
                args.finish = Finish::from_key(&value).ok_or_else(|| {
                    format!(
                        "argument --finish: invalid choice: '{value}' (choose from {})",
                        Finish::ALL
                            .iter()
                            .map(|f| format!("'{}'", f.key()))
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                })?;
            }
            "--paper" => args.paper = true,
            "--no-paper" => args.paper = false,
            "--grain" => args.grain = true,
            "--no-grain" => args.grain = false,
            "--sky" => args.sky = true,
            "--no-sky" => args.sky = false,
            "--sun" => args.sun = true,
            "--no-sun" => args.sun = false,
            "--mist" => args.mist = true,
            "--no-mist" => args.mist = false,
            "--vegetation" => args.vegetation = true,
            "--no-vegetation" => args.vegetation = false,
            "--architecture" => {
                args.architecture =
                    parse_density(&parse_value(raw, &mut index, inline, key)?, key)?;
            }
            "--people" => {
                args.people = parse_density(&parse_value(raw, &mut index, inline, key)?, key)?;
            }
            "--boats" => {
                args.boats = parse_density(&parse_value(raw, &mut index, inline, key)?, key)?;
            }
            "--dust" => {
                args.dust = parse_density(&parse_value(raw, &mut index, inline, key)?, key)?;
            }
            "--birds" => {
                args.birds = parse_i64(&parse_value(raw, &mut index, inline, key)?, key)? as i32;
            }
            "--chunk-width" => {
                args.chunk_width = parse_u32(&parse_value(raw, &mut index, inline, key)?, key)?;
                if args.chunk_width == 0 {
                    return Err("argument --chunk-width: must be at least 1".into());
                }
            }
            other => {
                return Err(format!("unrecognized argument: {other}"));
            }
        }
        index += 1;
    }
    Ok(args)
}

/// Match `pathlib.Path.with_suffix` closely enough for the CLI.
fn output_paths(out: &str, formats: &[&str]) -> Vec<(&'static str, PathBuf)> {
    let mut base = PathBuf::from(out);
    let suffix = base
        .extension()
        .map(|ext| ext.to_string_lossy().to_ascii_lowercase());
    if formats.len() == 1 && suffix.as_deref() == Some(formats[0]) {
        let format = if formats[0] == "png" { "png" } else { "svg" };
        return vec![(format, base)];
    }
    if matches!(suffix.as_deref(), Some("png") | Some("svg")) {
        base.set_extension("");
    }
    formats
        .iter()
        .map(|format| {
            let format: &'static str = if *format == "png" { "png" } else { "svg" };
            (format, base.with_extension(format))
        })
        .collect()
}

fn grain_seed(seed: &Seed) -> u64 {
    // Deterministic 64-bit FNV-1a of the resolved seed, so grain varies by seed
    // while exports stay reproducible.
    let mut hash = 0xcbf29ce484222325u64;
    for byte in seed.raw().as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn run(raw: &[String]) -> Result<(), String> {
    let args = parse_args(raw)?;

    if args.list_themes {
        let mut names = theme_names();
        names.sort_unstable();
        for name in names {
            let palette = get_palette(name)?;
            println!(
                "{:<8} paper={:?} ink={:?}",
                name,
                (palette.paper.r, palette.paper.g, palette.paper.b),
                (palette.ink.r, palette.ink.g, palette.ink.b)
            );
        }
        return Ok(());
    }

    if args.width < 64 || args.height < 64 {
        return Err("width and height must be at least 64".into());
    }
    if args.architecture < 0.0
        || args.architecture > 1.0
        || args.people < 0.0
        || args.people > 1.0
        || args.boats < 0.0
        || args.boats > 1.0
    {
        return Err("density arguments must be between 0 and 1".into());
    }

    let options = SceneOptions {
        seed: args.seed.clone(),
        theme: args.theme.clone(),
        width: args.width,
        height: args.height,
        camera: args.camera,
        chunk_width: args.chunk_width,
        vegetation: args.vegetation,
        architecture: args.architecture,
        people: args.people,
        boats: args.boats,
        dust: args.dust,
        water_style: "river".into(),
        sky: args.sky,
        sun: args.sun,
        mist: args.mist,
        birds: args.birds,
        title: args.title.clone(),
        seal: args.seal.clone(),
        title_size: args.title_size,
    };

    let started = Instant::now();
    let mut scene = Scene::new(options)?;
    let resolved = scene.seed().clone();
    let shapes = scene.render_at(args.camera, None, None);
    let rendered = started.elapsed();

    let xmin = args.camera - args.width as f64 / 2.0;
    let paper_color = scene.palette.paper;
    let paths = output_paths(&args.out, &args.formats);
    for (format, path) in paths {
        if format == "png" {
            let render_options = RenderOptions {
                xmin,
                width: args.width,
                height: args.height,
                scale: 1.0,
                paper_color: Some(paper_color),
                supersample: args.supersample,
                paper: args.paper,
                grain: args.grain,
                finish: args.finish,
                grain_seed: grain_seed(&resolved),
            };
            let bitmap = render_shapes(&shapes, &render_options);
            save_png(&path, &bitmap)?;
        } else {
            save_svg(
                &path,
                &shapes,
                xmin,
                args.width as f64,
                args.height as f64,
                args.paper,
                Some(paper_color),
                args.grain,
            )?;
        }
        println!("wrote {}", path.display());
    }

    let total = started.elapsed();
    println!(
        "theme={} seed={} shapes={} render={:.2}s total={:.2}s",
        args.theme,
        resolved,
        shapes.len(),
        rendered.as_secs_f64(),
        total.as_secs_f64()
    );
    Ok(())
}

fn main() -> ExitCode {
    let raw: Vec<String> = std::env::args().collect();
    match run(&raw) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("shan-shui: error: {message}");
            ExitCode::from(2)
        }
    }
}
