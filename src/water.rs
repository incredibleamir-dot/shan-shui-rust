use crate::architecture as arch;
use crate::context::Ctx;
use crate::primitives::{blob, norm_rand, stroke, wtrand};
use crate::shapes::{Circle, Point, Poly, Shape};

pub fn water(
    ctx: &mut Ctx,
    xoff: f64,
    yoff: f64,
    _seed: f64,
    hei: f64,
    len_: f64,
    clu: i32,
    depth: f64,
) -> Vec<Shape> {
    let mut shapes = Vec::new();
    let mut ptlist: Vec<Vec<Point>> = Vec::new();
    let mut yk = 0.0;
    for _ in 0..clu {
        let mut row: Vec<Point> = Vec::new();
        let xk = (ctx.rng.random() - 0.5) * (len_ / 8.0);
        yk += ctx.rng.random() * 5.0;
        let lk = len_ / 4.0 + ctx.rng.random() * (len_ / 4.0);
        let mut j = -lk;
        while j < lk {
            row.push((
                j + xk,
                (j * 0.2).sin() * hei * ctx.noise.noise(j * 0.1, 0.0, 0.0) - 20.0 + yk,
            ));
            j += 5.0;
        }
        ptlist.push(row);
    }

    for row in ptlist.iter().skip(1) {
        if row.len() < 2 {
            continue;
        }
        let shifted: Vec<Point> = row.iter().map(|p| (p.0 + xoff, p.1 + yoff)).collect();
        let col = ctx.palette.water_at(0.22 + ctx.rng.random() * 0.3, depth);
        shapes.extend(stroke(
            ctx,
            &shifted,
            0.0,
            0.0,
            1.0,
            Some(col),
            0.5,
            1.0,
            None,
        ));
    }
    shapes
}

pub fn sea(
    ctx: &mut Ctx,
    xoff: f64,
    yoff: f64,
    seed: f64,
    wid: f64,
    hei: f64,
    depth: f64,
) -> Vec<Shape> {
    let mut shapes = Vec::new();
    for _ in 0..42 {
        let y = yoff + ctx.rng.random() * hei;
        let lk = 60.0 + ctx.rng.random() * wid * 0.35;
        let xk = xoff + ctx.rng.random() * wid;
        let amp = 0.5 + ctx.rng.random() * 2.5;
        let mut row: Vec<Point> = Vec::new();
        let mut j = -lk;
        while j < lk {
            row.push((
                xk + j,
                y + (j * 0.05 + seed).sin() * amp * ctx.noise.noise(j * 0.02, seed, 0.0),
            ));
            j += 12.0;
        }
        let mut col = ctx.palette.water_at(0.08 + ctx.rng.random() * 0.22, depth);
        if ctx.rng.random() < 0.12 {
            col = ctx.palette.sun_at(0.06 + ctx.rng.random() * 0.1);
        }
        shapes.extend(stroke(ctx, &row, 0.0, 0.0, 1.0, Some(col), 0.2, 1.0, None));
    }

    let sx = xoff + wid * (0.58 + ctx.rng.random() * 0.25);
    let sy = yoff - hei * 0.4;
    shapes.push(Shape::Circle(Circle::filled(
        sx,
        sy,
        26.0 + ctx.rng.random() * 18.0,
        ctx.palette.sun_at(0.4),
    )));
    for _ in 0..9 {
        let wy = sy + 40.0 + ctx.rng.random() * hei * 0.9;
        let wl = 20.0 + ctx.rng.random() * 90.0;
        let row = [
            (sx - wl + ctx.rng.random() * wl, wy),
            (sx - wl + ctx.rng.random() * wl, wy),
        ];
        let col = ctx.palette.sun_at(0.15 + ctx.rng.random() * 0.2);
        shapes.extend(stroke(ctx, &row, 0.0, 0.0, 1.0, Some(col), 0.5, 1.0, None));
    }
    shapes
}

pub fn lake(
    ctx: &mut Ctx,
    xoff: f64,
    yoff: f64,
    seed: f64,
    wid: f64,
    hei: f64,
    depth: f64,
) -> Vec<Shape> {
    let mut shapes = Vec::new();
    let mut rim: Vec<Point> = Vec::new();
    let steps = 72;
    for i in 0..steps {
        let a = (i as f64 / steps as f64) * std::f64::consts::PI * 2.0;
        let wob = 1.0 + 0.05 * ctx.noise.noise(a.cos() * 2.0, a.sin() * 2.0, seed);
        rim.push((
            xoff + a.cos() * wid * 0.5 * wob,
            yoff + a.sin() * hei * 0.5 * wob,
        ));
    }
    shapes.push(Shape::Poly(Poly::new(
        rim.clone(),
        Some(ctx.palette.water_at(0.16, depth)),
    )));
    let mut rim_closed = rim.clone();
    rim_closed.push(rim[0]);
    let rim_col = ctx.palette.ink_at(0.22, depth);
    shapes.extend(stroke(
        ctx,
        &rim_closed,
        0.0,
        0.0,
        1.0,
        Some(rim_col),
        0.6,
        1.0,
        None,
    ));

    let waves = 10.max((wid * hei / 14000.0) as i32);
    for _ in 0..waves {
        let a = ctx.rng.random() * std::f64::consts::PI * 2.0;
        let rr = ctx.rng.random() * 0.62;
        let cx = xoff + a.cos() * wid * 0.5 * rr;
        let cy = yoff + a.sin() * hei * 0.5 * rr;
        let hl = 18.0 + ctx.rng.random() * 70.0;
        let mut row: Vec<Point> = Vec::new();
        let mut j = -hl;
        while j < hl {
            row.push((cx + j, cy + (j * 0.2).sin() * 1.5));
            j += 7.0;
        }
        if row.len() >= 2 {
            let col = ctx.palette.water_at(0.18 + ctx.rng.random() * 0.2, depth);
            shapes.extend(stroke(ctx, &row, 0.0, 0.0, 1.0, Some(col), 0.5, 1.0, None));
        }
    }

    if ctx.boats > 0.0 && ctx.rng.random() < 0.7 {
        let bx = xoff + norm_rand(ctx, -wid * 0.25, wid * 0.25);
        let by = yoff + norm_rand(ctx, -hei * 0.1, hei * 0.25);
        let blen = 70.0 + ctx.rng.random() * 50.0;
        let fli = ctx.rng.random() > 0.5;
        shapes.extend(arch::boat01(ctx, bx, by, seed, blen, 1.0, fli, depth));
    }

    if ctx.vegetation {
        for _ in 0..(2 + (ctx.rng.random() * 3.0) as i32) {
            let a = ctx.rng.random() * std::f64::consts::PI * 2.0;
            let rx = xoff + a.cos() * wid * 0.58;
            let ry = yoff + a.sin() * hei * 0.58;
            if ry < yoff {
                continue;
            }
            shapes.push(Shape::Circle(Circle::filled(
                rx,
                ry,
                2.0 + ctx.rng.random() * 2.0,
                ctx.palette.foliage_at(&mut ctx.rng, depth, 0.5),
            )));
        }
    }
    shapes
}

pub fn mist(
    ctx: &mut Ctx,
    xoff: f64,
    yoff: f64,
    _seed: f64,
    wid: f64,
    hei: f64,
    clu: i32,
    _depth: f64,
) -> Vec<Shape> {
    let mut shapes = Vec::new();
    for _ in 0..clu {
        let x = xoff + norm_rand(ctx, -wid * 0.2, wid * 1.2);
        let y = yoff + norm_rand(ctx, -hei * 0.2, hei);
        let l = 40.0 + wtrand(ctx, |t| t) * wid * 0.25;
        let w = 6.0 + ctx.rng.random() * 22.0;
        let alpha = 0.03 + ctx.rng.random() * 0.06;
        let ang = norm_rand(ctx, -0.15, 0.15);
        let col = ctx.palette.mist_at(alpha);
        shapes.extend(blob(ctx, x, y, l, w, ang, Some(col), 0.7, None));
    }
    shapes
}

pub fn birds(ctx: &mut Ctx, x: f64, y: f64, count: i32, depth: f64) -> Vec<Shape> {
    let mut shapes = Vec::new();
    for _ in 0..count {
        let bx = x + norm_rand(ctx, -40.0, 40.0);
        let by = y + norm_rand(ctx, -20.0, 20.0);
        let s = 3.0 + ctx.rng.random() * 4.0;
        let col = ctx.palette.ink_at(0.4, depth);
        for d in [-1.0, 1.0] {
            let row = [(bx, by), (bx + d * s, by - s * 0.6)];
            shapes.extend(stroke(ctx, &row, 0.0, 0.0, 1.0, Some(col), 0.0, 1.0, None));
        }
    }
    shapes
}
