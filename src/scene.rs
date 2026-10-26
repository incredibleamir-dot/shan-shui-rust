use std::collections::HashMap;

use crate::architecture as arch;
use crate::color::{get_palette, py_round, Palette};
use crate::context::Ctx;
use crate::mountains::{dist_mount, flat_mount, mountain};
use crate::noise::Noise;
use crate::planner::plan_range;
use crate::primitives::{norm_rand, stroke};
use crate::rng::{Rng, Seed};
use crate::shapes::{Circle, GradientRect, Point, Seal, Shape, Text};
use crate::water as water_mod;

#[derive(Clone, Debug, PartialEq)]
pub struct SceneOptions {
    pub seed: Option<Seed>,
    pub theme: String,
    pub width: u32,
    pub height: u32,
    pub camera: f64,
    pub chunk_width: u32,
    pub vegetation: bool,
    pub architecture: f64,
    pub people: f64,
    pub boats: f64,
    pub dust: f64,
    pub water_style: String,
    pub sky: bool,
    pub sun: bool,
    pub mist: bool,
    pub birds: i32,
    pub title: String,
    pub seal: String,
    pub title_size: f64,
}

impl Default for SceneOptions {
    fn default() -> Self {
        Self {
            seed: None,
            theme: "ink".into(),
            width: 1200,
            height: 800,
            camera: 0.0,
            chunk_width: 512,
            vegetation: true,
            architecture: 1.0,
            people: 1.0,
            boats: 1.0,
            dust: 1.0,
            water_style: "river".into(),
            sky: true,
            sun: true,
            mist: true,
            birds: 5,
            title: String::new(),
            seal: String::new(),
            title_size: 22.0,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SceneItem {
    pub tag: &'static str,
    pub x: f64,
    pub y: f64,
    pub shapes: Vec<Shape>,
    pub z: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Chunk {
    pub index: i32,
    pub xmin: f64,
    pub xmax: f64,
    pub items: Vec<SceneItem>,
}

fn clamp(value: f64, lo: f64, hi: f64) -> f64 {
    if value < lo {
        lo
    } else if value > hi {
        hi
    } else {
        value
    }
}

#[allow(dead_code)]
fn depth_for(y: f64) -> f64 {
    clamp((y - 260.0) / 520.0, 0.3, 1.0)
}

/// Python-compatible float formatting for RNG child labels.
fn py_float_str(value: f64) -> String {
    if value == 0.0 {
        return if value.is_sign_negative() {
            "-0.0".into()
        } else {
            "0.0".into()
        };
    }
    if value.fract() == 0.0 && value.abs() < 1e16 {
        return format!("{:.1}", value);
    }
    format!("{}", value)
}

fn round2(value: f64) -> f64 {
    py_round(value * 100.0) / 100.0
}

pub struct Scene {
    pub options: SceneOptions,
    pub palette: Palette,
    pub base_rng: Rng,
    base_ctx: Ctx,
    chunks: HashMap<i32, Chunk>,
}

impl Scene {
    pub fn new(options: SceneOptions) -> Result<Self, String> {
        let palette = get_palette(&options.theme)?;
        let base_rng = Rng::new(options.seed.clone());
        let noise_rng = base_rng.child("noise");
        let noise = Noise::new(noise_rng);
        let base_ctx = Ctx {
            rng: base_rng.child("brush"),
            noise,
            palette: std::sync::Arc::new(palette.clone()),
            vegetation: options.vegetation,
            architecture: options.architecture,
            people: options.people,
            boats: options.boats,
            dust: options.dust,
        };
        Ok(Self {
            options,
            palette,
            base_rng,
            base_ctx,
            chunks: HashMap::new(),
        })
    }

    pub fn seed(&self) -> &Seed {
        &self.base_rng.seed_value
    }

    pub fn chunk_width(&self) -> f64 {
        self.options.chunk_width as f64
    }

    fn build_chunk(&mut self, index: i32) -> Chunk {
        let xmin = index as f64 * self.chunk_width();
        let xmax = xmin + self.chunk_width();
        let mut chunk = Chunk {
            index,
            xmin,
            xmax,
            items: Vec::new(),
        };
        let label = format!("('chunk', {})", index);
        let mut ctx = self.base_ctx.child(&label);
        let plans = plan_range(xmin, xmax, &mut ctx);
        let mut item_seed = 0.0f64;

        for (k, plan) in plans.iter().enumerate() {
            let item_label = format!("('item', {}, '{}')", k, plan.tag);
            let mut item_ctx = ctx.child(&item_label);
            item_seed = item_ctx.rng.random() * 1000.0;
            let depth = if plan.tag == "distmount" {
                0.15
            } else {
                clamp(plan.y / 800.0, 0.3, 1.0)
            };
            match plan.tag {
                "mount" => {
                    let shapes = mountain(
                        &mut item_ctx,
                        plan.x,
                        plan.y,
                        item_seed,
                        None,
                        None,
                        200,
                        true,
                        depth,
                    );
                    chunk.items.push(SceneItem {
                        tag: "mount",
                        x: plan.x,
                        y: plan.y,
                        shapes,
                        z: 0.0,
                    });
                    let waves = water_mod::water(
                        &mut item_ctx.child("waves"),
                        plan.x,
                        plan.y,
                        item_seed,
                        2.0,
                        800.0,
                        10,
                        depth,
                    );
                    chunk.items.push(SceneItem {
                        tag: "water",
                        x: plan.x,
                        y: plan.y,
                        shapes: waves,
                        z: -1.0,
                    });
                }
                "flatmount" => {
                    let wid = 600.0 + item_ctx.rng.random() * 400.0;
                    let cho = 0.5 + item_ctx.rng.random() * 0.2;
                    let shapes = flat_mount(
                        &mut item_ctx,
                        plan.x,
                        plan.y,
                        item_seed,
                        Some(100.0),
                        Some(wid),
                        80,
                        cho,
                        depth,
                    );
                    chunk.items.push(SceneItem {
                        tag: "flatmount",
                        x: plan.x,
                        y: plan.y,
                        shapes,
                        z: 0.0,
                    });
                }
                "distmount" => {
                    let seed = item_ctx.rng.random() * 100.0;
                    let len_ = *item_ctx.rng.choice(&[500.0, 1000.0, 1500.0]);
                    let shapes = dist_mount(
                        &mut item_ctx,
                        plan.x,
                        plan.y,
                        seed,
                        Some(150.0),
                        len_,
                        5,
                        depth,
                    );
                    chunk.items.push(SceneItem {
                        tag: "distmount",
                        x: plan.x,
                        y: plan.y,
                        shapes,
                        z: 0.0,
                    });
                }
                "boat" => {
                    if item_ctx.rng.random() >= self.options.boats {
                        continue;
                    }
                    let fli = *item_ctx.rng.choice(&[true, false]);
                    let shapes = arch::boat01(
                        &mut item_ctx,
                        plan.x,
                        plan.y,
                        item_seed,
                        120.0,
                        plan.y / 800.0,
                        fli,
                        depth,
                    );
                    chunk.items.push(SceneItem {
                        tag: "boat",
                        x: plan.x,
                        y: plan.y,
                        shapes,
                        z: 0.0,
                    });
                }
                _ => {}
            }
        }

        if self.options.mist {
            let label = format!("('mist', {})", index);
            let mut mist_ctx = ctx.child(&label);
            let seed = if !plans.is_empty() {
                item_seed
            } else {
                index as f64
            };
            let shapes = water_mod::mist(
                &mut mist_ctx,
                xmin,
                560.0,
                seed,
                self.chunk_width() * 1.3,
                220.0,
                24,
                0.35,
            );
            chunk.items.push(SceneItem {
                tag: "mist",
                x: xmin,
                y: 560.0,
                shapes,
                z: -0.4,
            });
        }

        chunk
    }

    pub fn chunk(&mut self, index: i32) -> &Chunk {
        if !self.chunks.contains_key(&index) {
            let chunk = self.build_chunk(index);
            self.chunks.insert(index, chunk);
        }
        self.chunks.get(&index).unwrap()
    }

    pub fn preload(&mut self, xmin: f64, xmax: f64) {
        let _ = self.chunks_for(xmin, xmax);
    }

    pub fn chunks_for(&mut self, xmin: f64, xmax: f64) -> Vec<&Chunk> {
        let cw = self.chunk_width();
        let i0 = ((xmin - cw) / cw).floor() as i32;
        let i1 = ((xmax + cw) / cw).floor() as i32;
        let mut out = Vec::new();
        for i in i0..=i1 {
            // Build first, then borrow immutably.
            let _ = self.chunk(i);
        }
        for i in i0..=i1 {
            out.push(self.chunks.get(&i).unwrap());
        }
        out
    }

    /// Paint the view from `xmin..xmax`, but keep the sun and birds tied to a
    /// fixed pair of world ranges. The studio uses this to make a camera crop
    /// of the full panorama match the sharpened viewport exactly: the sky is
    /// fixed in the painting instead of sliding with the camera.
    fn background(
        &mut self,
        xmin: f64,
        xmax: f64,
        height: f64,
        sun_anchor: (f64, f64),
        birds_anchor: (f64, f64),
    ) -> Vec<Shape> {
        let mut shapes: Vec<Shape> = Vec::new();
        let (top, bottom) = if self.options.sky {
            (self.palette.sky_top, self.palette.sky_bottom)
        } else {
            (self.palette.fill, self.palette.fill)
        };
        shapes.push(Shape::GradientRect(GradientRect {
            x: xmin,
            y: 0.0,
            w: xmax - xmin,
            h: height,
            top,
            bottom,
            vertical: true,
        }));

        if self.options.sun {
            let label = format!("('sun', {})", py_float_str(round2(sun_anchor.0)));
            let mut ctx = self.base_ctx.child(&label);
            let cx = sun_anchor.0 + (sun_anchor.1 - sun_anchor.0) * (0.68 + ctx.rng.random() * 0.1);
            let cy = height * (0.2 + ctx.rng.random() * 0.08);
            let r = height * (0.045 + ctx.rng.random() * 0.02);
            for k in (1..=12).rev() {
                let t = k as f64 / 12.0;
                shapes.push(Shape::Circle(Circle::filled(
                    cx,
                    cy,
                    r * (1.0 + t * 1.8),
                    self.palette.sun_at(0.012 + 0.012 * (1.0 - t)),
                )));
            }
            shapes.push(Shape::Circle(Circle::filled(
                cx,
                cy,
                r,
                self.palette.sun_at(0.5),
            )));
        }

        if self.options.birds > 0 {
            let label = format!("('birds', {})", py_float_str(round2(birds_anchor.0)));
            let mut ctx = self.base_ctx.child(&label);
            let ink = self.palette.ink_at(0.5, 0.6);
            for _ in 0..self.options.birds {
                let bx = norm_rand(
                    &mut ctx,
                    birds_anchor.0 + (birds_anchor.1 - birds_anchor.0) * 0.15,
                    birds_anchor.0 + (birds_anchor.1 - birds_anchor.0) * 0.85,
                );
                let by = norm_rand(&mut ctx, height * 0.08, height * 0.3);
                let s = 3.0 + ctx.rng.random() * 4.0;
                for d in [-1.0, 1.0] {
                    let row = [(bx, by), (bx + d * s, by - s * 0.6)];
                    shapes.extend(stroke(
                        &mut ctx,
                        &row,
                        0.0,
                        0.0,
                        1.0,
                        Some(ink),
                        0.0,
                        1.0,
                        None,
                    ));
                }
            }
        }
        shapes
    }

    pub fn render(
        &mut self,
        xmin: Option<f64>,
        xmax: Option<f64>,
        width: Option<f64>,
        height: Option<f64>,
    ) -> Vec<Shape> {
        let width = width.unwrap_or(self.options.width as f64);
        let height = height.unwrap_or(self.options.height as f64);
        let xmin = xmin.unwrap_or(self.options.camera - width / 2.0);
        let xmax = xmax.unwrap_or(xmin + width);
        self.render_window(xmin, xmax, height, (xmin, xmax), (xmin, xmax))
    }

    /// Render a viewport whose sky is anchored to a fixed part of the world.
    /// This keeps a cropped panorama and its sharpened viewport identical while
    /// still letting the study/export paths use the classic framing rules.
    pub fn render_window(
        &mut self,
        xmin: f64,
        xmax: f64,
        height: f64,
        sun_anchor: (f64, f64),
        birds_anchor: (f64, f64),
    ) -> Vec<Shape> {
        let mut shapes = self.background(xmin, xmax, height, sun_anchor, birds_anchor);

        let cw = self.chunk_width();
        let i0 = ((xmin - cw) / cw).floor() as i32;
        let i1 = ((xmax + cw) / cw).floor() as i32;
        let mut entries: Vec<(f64, f64, usize, i32, usize)> = Vec::new();
        let mut order = 0usize;
        for index in i0..=i1 {
            let _ = self.chunk(index);
            let chunk = self.chunks.get(&index).unwrap();
            for (item_idx, item) in chunk.items.iter().enumerate() {
                if xmin - cw - 400.0 < item.x && item.x < xmax + cw + 400.0 {
                    entries.push((item.z, item.y, order, index, item_idx));
                    order += 1;
                }
            }
        }
        entries.sort_by(|a, b| {
            a.0.partial_cmp(&b.0)
                .unwrap()
                .then(a.1.partial_cmp(&b.1).unwrap())
                .then(a.2.cmp(&b.2))
        });
        for (_, _, _, index, item_idx) in entries {
            let item = &self.chunks.get(&index).unwrap().items[item_idx];
            if !item.shapes.is_empty() {
                shapes.extend(item.shapes.iter().cloned());
            }
        }

        if self.options.mist {
            shapes.push(Shape::GradientRect(GradientRect {
                x: xmin,
                y: height * 0.62,
                w: xmax - xmin,
                h: height * 0.38,
                top: self.palette.mist_at(0.0),
                bottom: self.palette.mist_at(0.16),
                vertical: true,
            }));
        }

        if !self.options.title.is_empty() {
            shapes.push(Shape::Text(Text {
                x: xmin + 46.0,
                y: 64.0,
                text: self.options.title.clone(),
                size: self.options.title_size,
                color: self.palette.ink_at(0.78, 0.9),
                angle: 0.0,
                family: "serif".into(),
                anchor: "start".into(),
            }));
        }
        if !self.options.seal.is_empty() {
            shapes.push(Shape::Seal(Seal::new(
                xmin + 46.0,
                64.0 + self.options.title_size + 22.0,
                30.0,
                self.options.seal.clone(),
                self.palette.accent_at(0.9, 1.0),
            )));
        }
        shapes
    }

    pub fn render_at(
        &mut self,
        camera: f64,
        width: Option<f64>,
        height: Option<f64>,
    ) -> Vec<Shape> {
        let width = width.unwrap_or(self.options.width as f64);
        let height = height.unwrap_or(self.options.height as f64);
        self.render(
            Some(camera - width / 2.0),
            Some(camera + width / 2.0),
            Some(width),
            Some(height),
        )
    }

    pub fn shape_count(&mut self, xmin: f64, xmax: f64) -> usize {
        self.chunks_for(xmin, xmax)
            .iter()
            .map(|c| c.items.iter().map(|i| i.shapes.len()).sum::<usize>())
            .sum()
    }

    /// Debug helper: reproduce the planned items for a chunk without building
    /// its shapes. Used by the parity dump tools.
    #[doc(hidden)]
    pub fn debug_plans(&self, index: i32) -> Vec<crate::planner::PlanItem> {
        let xmin = index as f64 * self.chunk_width();
        let label = format!("('chunk', {})", index);
        let mut ctx = self.base_ctx.child(&label);
        crate::planner::plan_range(xmin, xmin + self.chunk_width(), &mut ctx)
    }
}

pub fn shape_values(shapes: &[Shape]) -> Vec<f64> {
    let mut values = Vec::new();
    for shape in shapes {
        match shape {
            Shape::Poly(poly) => {
                for (x, y) in &poly.pts {
                    values.push(*x);
                    values.push(*y);
                }
            }
            Shape::Circle(c) => {
                values.push(c.cx);
                values.push(c.cy);
                values.push(c.r);
            }
            Shape::GradientRect(g) => {
                values.push(g.x);
                values.push(g.y);
                values.push(g.w);
                values.push(g.h);
            }
            Shape::Text(t) => {
                values.push(t.x);
                values.push(t.y);
                values.push(t.size);
            }
            Shape::Seal(s) => {
                values.push(s.x);
                values.push(s.y);
                values.push(s.size);
            }
        }
    }
    values
}

pub fn point_at(x: f64, y: f64) -> Point {
    (x, y)
}
