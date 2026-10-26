use crate::architecture as arch;
use crate::color::Color;
use crate::context::Ctx;
use crate::geometry::{mid_pt, triangulate};
use crate::primitives::{div, loop_noise, norm_rand, rand_choice, stroke, texture};
use crate::shapes::{Point, Poly, Shape};
use crate::trees;

fn count(limit: f64) -> usize {
    limit.ceil().max(0.0) as usize
}

fn py_mod2(value: usize) -> bool {
    value % 2 != 0
}

pub fn foot(ctx: &mut Ctx, ptlist: &[Vec<Point>], xof: f64, yof: f64, depth: f64) -> Vec<Shape> {
    let mut ftlist: Vec<Vec<Point>> = Vec::new();
    let span = 10usize;
    let mut ni = 0usize;
    for i in 0..ptlist.len().saturating_sub(2) {
        if i == ni {
            let step = *rand_choice(ctx, &[1, 2]);
            ni = (ni + step).min(ptlist.len() - 1);
            ftlist.push(Vec::new());
            ftlist.push(Vec::new());
            for j in 0..count((ptlist[i].len() as f64 / 8.0).min(10.0)) {
                let a = ftlist.len() - 2;
                ftlist[a].push((
                    ptlist[i][j].0 + ctx.noise.noise(j as f64 * 0.1, i as f64, 0.0) * 10.0,
                    ptlist[i][j].1,
                ));
                let b = ftlist.len() - 1;
                ftlist[b].push((
                    ptlist[i][ptlist[i].len() - 1 - j].0
                        - ctx.noise.noise(j as f64 * 0.1, i as f64, 0.0) * 10.0,
                    ptlist[i][ptlist[i].len() - 1 - j].1,
                ));
            }
            let idx = ftlist.len() - 2;
            ftlist[idx].reverse();
            let idx = ftlist.len() - 1;
            ftlist[idx].reverse();
            for j in 0..span {
                let p = j as f64 / span as f64;
                let x1 = ptlist[i][0].0 * (1.0 - p) + ptlist[ni][0].0 * p;
                let mut y1 = ptlist[i][0].1 * (1.0 - p) + ptlist[ni][0].1 * p;
                let x2 = ptlist[i][ptlist[i].len() - 1].0 * (1.0 - p)
                    + ptlist[ni][ptlist[ni].len() - 1].0 * p;
                let mut y2 = ptlist[i][ptlist[i].len() - 1].1 * (1.0 - p)
                    + ptlist[ni][ptlist[ni].len() - 1].1 * p;
                let vib = -1.7 * (p - 1.0) * p.powf(1.0 / 5.0);
                let n = ctx.noise.noise(xof * 0.05, i as f64, 0.0) * 5.0;
                y1 += vib * 5.0 + n;
                y2 += vib * 5.0 + n;
                let a = ftlist.len() - 2;
                ftlist[a].push((x1, y1));
                let b = ftlist.len() - 1;
                ftlist[b].push((x2, y2));
            }
        }
    }

    let mut shapes: Vec<Shape> = Vec::new();
    for line in &ftlist {
        if !line.is_empty() {
            shapes.push(Shape::Poly(Poly::new(
                line.iter().map(|p| (p.0 + xof, p.1 + yof)).collect(),
                Some(ctx.palette.fill),
            )));
        }
    }
    for line in &ftlist {
        if !line.is_empty() {
            let col = ctx.palette.ink_at(0.1 + ctx.rng.random() * 0.1, depth);
            let shifted: Vec<Point> = line.iter().map(|p| (p.0 + xof, p.1 + yof)).collect();
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
    }
    shapes
}

pub fn mountain(
    ctx: &mut Ctx,
    xoff: f64,
    yoff: f64,
    seed: f64,
    hei: Option<f64>,
    wid: Option<f64>,
    tex: usize,
    veg: bool,
    depth: f64,
) -> Vec<Shape> {
    let hei = hei.unwrap_or_else(|| 100.0 + ctx.rng.random() * 400.0);
    let wid = wid.unwrap_or_else(|| 400.0 + ctx.rng.random() * 200.0);
    let h = hei;
    let w = wid;
    let reso = (10usize, 50usize);
    let mut shapes: Vec<Shape> = Vec::new();

    let mut ptlist: Vec<Vec<Point>> = Vec::new();
    let mut hoff = 0.0;
    for j in 0..reso.0 {
        hoff += (ctx.rng.random() * yoff) / 100.0;
        let mut row: Vec<Point> = Vec::new();
        for i in 0..reso.1 {
            let x = (i as f64 / reso.1 as f64 - 0.5) * std::f64::consts::PI;
            let mut y = x.cos();
            y *= ctx.noise.noise(x + 10.0, j as f64 * 0.15, seed);
            let p = 1.0 - j as f64 / reso.0 as f64;
            row.push(((x / std::f64::consts::PI) * w * p, -y * h * p + hoff));
        }
        ptlist.push(row);
    }

    if ctx.vegetation {
        // Top fringe trees.
        let mut veglist: Vec<Point> = Vec::new();
        for i in 0..ptlist.len() {
            for j in 0..ptlist[i].len() {
                let y = ptlist[i][j].1;
                if i == 0
                    && ctx.noise.noise(j as f64 * 0.1, seed, 0.0).powi(3) < 0.1
                    && y.abs() / h > 0.2
                {
                    veglist.push(ptlist[i][j]);
                }
            }
        }
        for (vx, vy) in veglist {
            shapes.extend(trees::tree02(
                ctx,
                vx + xoff,
                vy + yoff - 5.0,
                16.0,
                8.0,
                2,
                depth,
                None,
            ));
        }
    }

    let mut outline = ptlist[0].clone();
    outline.push((0.0, reso.0 as f64 * 4.0));
    shapes.push(Shape::Poly(Poly::new(
        outline
            .into_iter()
            .map(|p| (p.0 + xoff, p.1 + yoff))
            .collect(),
        Some(ctx.palette.fill),
    )));

    let outline_col = ctx.palette.ink_at(0.3, depth);
    let shifted: Vec<Point> = ptlist[0].iter().map(|p| (p.0 + xoff, p.1 + yoff)).collect();
    shapes.extend(stroke(
        ctx,
        &shifted,
        0.0,
        0.0,
        3.0,
        Some(outline_col),
        1.0,
        1.0,
        None,
    ));

    shapes.extend(foot(ctx, &ptlist, xoff, yoff, depth));

    let sha = *rand_choice(ctx, &[0.0, 0.0, 0.0, 0.0, 5.0]);
    let shade = ctx.palette.ink_at(0.1, depth);
    shapes.extend(texture(
        ctx,
        &ptlist,
        xoff,
        yoff,
        tex,
        1.5,
        0.2,
        sha,
        None,
        Some(&mut |ctx: &mut Ctx, _t: f64| {
            ctx.palette.wash_at(0.12 + ctx.rng.random() * 0.22, depth)
        }),
        None,
        Some(shade),
    ));

    if ctx.vegetation {
        let mut veglist: Vec<Point> = Vec::new();
        for i in 0..ptlist.len() {
            for j in 0..ptlist[i].len() {
                let y = ptlist[i][j].1;
                if ctx
                    .noise
                    .noise(i as f64 * 0.1, j as f64 * 0.1, seed + 2.0)
                    .powi(3)
                    < 0.1
                    && y.abs() / h > 0.5
                {
                    veglist.push(ptlist[i][j]);
                }
            }
        }
        for (vx, vy) in veglist {
            shapes.extend(trees::tree02(
                ctx,
                vx + xoff,
                vy + yoff,
                16.0,
                8.0,
                5,
                depth,
                None,
            ));
        }
    }

    if veg && ctx.vegetation {
        // Middle trees with a cluster proof.
        let mut veglist: Vec<Point> = Vec::new();
        for i in 0..ptlist.len() {
            for j in 0..ptlist[i].len() {
                let ns = ctx.noise.noise(i as f64 * 0.2, j as f64 * 0.05, seed);
                if py_mod2(j) && ns.powi(4) < 0.012 && ptlist[i][j].1.abs() / h < 0.3 {
                    veglist.push(ptlist[i][j]);
                }
            }
        }
        for (i, (vx, vy)) in veglist.iter().enumerate() {
            let mut counter = 0;
            let mut proof = false;
            for (j, v) in veglist.iter().enumerate() {
                if i != j && (vx - v.0).powi(2) + (vy - v.1).powi(2) < 900.0 {
                    counter += 1;
                }
                if counter > 2 {
                    proof = true;
                    break;
                }
            }
            if proof {
                let ratio = (h + vy) / h;
                let hei = ratio * 70.0 * 0.3 + ctx.rng.random() * (ratio * 70.0) * 0.7;
                let wid = ctx.rng.random() * 3.0 + 1.0;
                shapes.extend(trees::tree01(
                    ctx,
                    vx + xoff,
                    vy + yoff,
                    hei,
                    wid,
                    depth,
                    None,
                    None,
                ));
            }
        }

        // Bottom-fringe trees.
        let mut veglist: Vec<Point> = Vec::new();
        for i in 0..ptlist.len() {
            for j in 0..ptlist[i].len() {
                let ns = ctx.noise.noise(i as f64 * 0.2, j as f64 * 0.05, seed);
                if (j == 0 || j == ptlist[i].len() - 1) && ns.powi(4) < 0.012 {
                    veglist.push(ptlist[i][j]);
                }
            }
        }
        for (vx, vy) in veglist {
            let ratio = (h + vy) / h;
            let mut ht = ratio * 120.0;
            ht = ht * 0.5 + ctx.rng.random() * ht * 0.5;
            let bc = ctx.rng.random() * 0.1;
            let ben = move |t: f64| t * bc;
            shapes.extend(trees::tree03(
                ctx,
                vx + xoff,
                vy + yoff,
                ht,
                5.0,
                Some(&ben),
                depth,
                None,
            ));
        }
    }

    // Small buildings at the foot.
    let mut veglist: Vec<Point> = Vec::new();
    for i in 0..ptlist.len() {
        for j in 0..ptlist[i].len() {
            let ns = ctx
                .noise
                .noise(i as f64 * 0.2, j as f64 * 0.05, seed + 10.0);
            if i != 0 && (j == 1 || j == ptlist[i].len() - 2) && ns.powi(4) < 0.008 {
                veglist.push(ptlist[i][j]);
            }
        }
    }
    for (vx, vy) in veglist {
        if ctx.rng.random() >= ctx.architecture {
            continue;
        }
        let tt = *rand_choice(ctx, &[0, 0, 1, 1, 1, 2]);
        if tt == 1 {
            let wid = norm_rand(ctx, 40.0, 70.0);
            let sto = *rand_choice(ctx, &[1, 2, 2, 3]);
            let rot = ctx.rng.random();
            let sty = *rand_choice(ctx, &[1, 2, 3]);
            shapes.extend(arch::arch02(
                ctx,
                vx + xoff,
                vy + yoff,
                seed,
                10.0,
                wid,
                rot,
                5.0,
                sto,
                sty,
                false,
                None,
                depth,
            ));
        } else if tt == 2 {
            let sto = *rand_choice(ctx, &[1, 1, 1, 2, 2]);
            shapes.extend(arch::arch04(
                ctx,
                vx + xoff,
                vy + yoff,
                seed,
                15.0,
                30.0,
                0.7,
                5.0,
                sto,
                depth,
            ));
        }
    }

    // Pagoda at the first interior row.
    let mut veglist: Vec<Point> = Vec::new();
    for i in 0..ptlist.len() {
        for j in 0..ptlist[i].len() {
            if i == 1
                && (j as f64 - ptlist[i].len() as f64 / 2.0).abs() < 1.0
                && ctx.rng.random() < 0.02
            {
                veglist.push(ptlist[i][j]);
            }
        }
    }
    for (vx, vy) in veglist {
        if ctx.rng.random() < ctx.architecture {
            let sto = *rand_choice(ctx, &[5, 7]);
            let wid = 40.0 + ctx.rng.random() * 20.0;
            shapes.extend(arch::arch03(
                ctx,
                vx + xoff,
                vy + yoff,
                seed,
                10.0,
                wid,
                0.7,
                5.0,
                sto,
                depth,
            ));
        }
    }

    // Transmission towers.
    let mut veglist: Vec<Point> = Vec::new();
    for i in 0..ptlist.len() {
        for j in 0..ptlist[i].len() {
            if i % 2 == 0
                && (j == 1 || j == ptlist[i].len() - 2)
                && ctx
                    .noise
                    .noise(
                        i as f64 * 0.2,
                        j as f64 * 0.05,
                        seed + 20.0 * std::f64::consts::PI,
                    )
                    .powi(4)
                    < 0.002
            {
                veglist.push(ptlist[i][j]);
            }
        }
    }
    for (vx, vy) in veglist {
        if ctx.rng.random() < ctx.architecture {
            shapes.extend(arch::transmission_tower01(
                ctx,
                vx + xoff,
                vy + yoff,
                seed,
                100.0,
                20.0,
                depth,
            ));
        }
    }

    // Rocks along the base.
    let mut veglist: Vec<Point> = Vec::new();
    for i in 0..ptlist.len() {
        for j in 0..ptlist[i].len() {
            if (j == 0 || j == ptlist[i].len() - 1) && ctx.rng.random() < 0.1 {
                veglist.push(ptlist[i][j]);
            }
        }
    }
    for (vx, vy) in veglist {
        let wid = 20.0 + ctx.rng.random() * 20.0;
        let hei = 20.0 + ctx.rng.random() * 20.0;
        shapes.extend(rock(
            ctx,
            vx + xoff,
            vy + yoff,
            seed,
            hei,
            wid,
            40,
            2.0,
            depth,
        ));
    }

    shapes
}

pub fn flat_mount(
    ctx: &mut Ctx,
    xoff: f64,
    yoff: f64,
    seed: f64,
    hei: Option<f64>,
    wid: Option<f64>,
    tex: usize,
    cho: f64,
    depth: f64,
) -> Vec<Shape> {
    let hei = hei.unwrap_or_else(|| 40.0 + ctx.rng.random() * 400.0);
    let wid = wid.unwrap_or_else(|| 400.0 + ctx.rng.random() * 200.0);
    let mut shapes: Vec<Shape> = Vec::new();
    let mut ptlist: Vec<Vec<Point>> = Vec::new();
    let reso = (5usize, 50usize);
    let mut hoff = 0.0;
    let mut flat: Vec<Vec<Point>> = Vec::new();

    for j in 0..reso.0 {
        hoff += (ctx.rng.random() * yoff) / 100.0;
        ptlist.push(Vec::new());
        flat.push(Vec::new());
        for i in 0..reso.1 {
            let x = (i as f64 / reso.1 as f64 - 0.5) * std::f64::consts::PI;
            let mut y = (x * 2.0).cos() + 1.0;
            y *= ctx.noise.noise(x + 10.0, j as f64 * 0.1, seed);
            let p = 1.0 - (j as f64 / reso.0 as f64) * 0.6;
            let nx = (x / std::f64::consts::PI) * wid * p;
            let mut ny = -y * hei * p + hoff;
            let h = 100.0;
            if ny < -h * cho + hoff {
                ny = -h * cho + hoff;
                if flat[flat.len() - 1].len() % 2 == 0 {
                    let last = flat.len() - 1;
                    flat[last].push((nx, ny));
                }
            } else if flat[flat.len() - 1].len() % 2 == 1 {
                let last = flat.len() - 1;
                let p = ptlist[last][ptlist[last].len() - 1];
                flat[last].push(p);
            }
            let last = ptlist.len() - 1;
            ptlist[last].push((nx, ny));
        }
    }

    let mut outline = ptlist[0].clone();
    outline.push((0.0, reso.0 as f64 * 4.0));
    shapes.push(Shape::Poly(Poly::new(
        outline
            .into_iter()
            .map(|p| (p.0 + xoff, p.1 + yoff))
            .collect(),
        Some(ctx.palette.fill),
    )));

    let col = ctx.palette.ink_at(0.3, depth);
    let shifted: Vec<Point> = ptlist[0].iter().map(|p| (p.0 + xoff, p.1 + yoff)).collect();
    shapes.extend(stroke(
        ctx,
        &shifted,
        0.0,
        0.0,
        3.0,
        Some(col),
        1.0,
        1.0,
        None,
    ));

    let shade = ctx.palette.ink_at(0.1, depth);
    shapes.extend(texture(
        ctx,
        &ptlist,
        xoff,
        yoff,
        tex,
        2.0,
        0.2,
        0.0,
        None,
        Some(&mut |ctx: &mut Ctx, _t: f64| {
            ctx.palette.wash_at(0.12 + ctx.rng.random() * 0.25, depth)
        }),
        Some(&mut |ctx: &mut Ctx| {
            if ctx.rng.random() > 0.5 {
                0.1 + 0.4 * ctx.rng.random()
            } else {
                0.9 - 0.4 * ctx.rng.random()
            }
        }),
        Some(shade),
    ));

    let mut grlist1: Vec<Point> = Vec::new();
    let mut grlist2: Vec<Point> = Vec::new();
    let mut i = 0;
    while i < flat.len() {
        if flat[i].len() >= 2 {
            grlist1.push(flat[i][0]);
            grlist2.push(flat[i][flat[i].len() - 1]);
        }
        i += 2;
    }

    if grlist1.is_empty() {
        return shapes;
    }

    let wb = (grlist1[0].0, grlist2[0].0);
    for i in 0..3 {
        let p = 0.8 - i as f64 * 0.2;
        let y1 = grlist1[0].1 - 5.0;
        let y2 = grlist2[0].1 - 5.0;
        grlist1.insert(0, (wb.0 * p, y1));
        grlist2.insert(0, (wb.1 * p, y2));
    }
    let wb = (grlist1[grlist1.len() - 1].0, grlist2[grlist2.len() - 1].0);
    for i in 0..3 {
        let p = 0.6 - (i * i) as f64 * 0.1;
        let y1 = grlist1[grlist1.len() - 1].1 + 1.0;
        let y2 = grlist2[grlist2.len() - 1].1 + 1.0;
        grlist1.push((wb.0 * p, y1));
        grlist2.push((wb.1 * p, y2));
    }

    let d = 5;
    grlist1 = div(&grlist1, d as f64);
    grlist2 = div(&grlist2, d as f64);
    let rev1: Vec<Point> = grlist1.iter().rev().copied().collect();
    let mut grlist: Vec<Point> = rev1.clone();
    grlist.extend(grlist2);
    grlist.push(rev1[0]);

    for i in 0..grlist.len() {
        let v = (1.0 - ((i % d as usize) as f64 - d as f64 / 2.0).abs() / (d as f64 / 2.0)) * 0.12;
        grlist[i] = (
            grlist[i].0 * (1.0 - v + ctx.noise.noise(grlist[i].1 * 0.5, 0.0, 0.0) * v),
            grlist[i].1,
        );
    }

    shapes.push(Shape::Poly(Poly::new(
        grlist.iter().map(|p| (p.0 + xoff, p.1 + yoff)).collect(),
        Some(ctx.palette.fill),
    )));
    let col = ctx.palette.ink_at(0.2, depth);
    let shifted: Vec<Point> = grlist.iter().map(|p| (p.0 + xoff, p.1 + yoff)).collect();
    shapes.extend(stroke(
        ctx,
        &shifted,
        0.0,
        0.0,
        3.0,
        Some(col),
        0.5,
        1.0,
        None,
    ));

    let xmin = grlist.iter().map(|p| p.0).fold(f64::INFINITY, f64::min);
    let xmax = grlist.iter().map(|p| p.0).fold(f64::NEG_INFINITY, f64::max);
    let ymin = grlist.iter().map(|p| p.1).fold(f64::INFINITY, f64::min);
    let ymax = grlist.iter().map(|p| p.1).fold(f64::NEG_INFINITY, f64::max);
    shapes.extend(flat_dec(ctx, xoff, yoff, (xmin, xmax, ymin, ymax), depth));
    shapes
}

pub fn flat_dec(
    ctx: &mut Ctx,
    xoff: f64,
    yoff: f64,
    bounds: (f64, f64, f64, f64),
    depth: f64,
) -> Vec<Shape> {
    let (gxmin, gxmax, gymin, gymax) = bounds;
    let mut shapes: Vec<Shape> = Vec::new();
    let tt = *rand_choice(ctx, &[0, 0, 1, 2, 3, 4]);
    let mid_y = (gymin + gymax) / 2.0;

    for _ in 0..count(ctx.rng.random() * 5.0) {
        let x = xoff + norm_rand(ctx, gxmin, gxmax);
        let y = yoff + mid_y + norm_rand(ctx, -10.0, 10.0) + 10.0;
        let seed = ctx.rng.random() * 100.0;
        let wid = 10.0 + ctx.rng.random() * 20.0;
        let hei = 10.0 + ctx.rng.random() * 20.0;
        shapes.extend(rock(ctx, x, y, seed, hei, wid, 40, 2.0, depth));
    }
    for _ in 0..(*rand_choice(ctx, &[0, 0, 1, 2]) as usize) {
        let xr = xoff + norm_rand(ctx, gxmin, gxmax);
        let yr = yoff + mid_y + norm_rand(ctx, -5.0, 5.0) + 20.0;
        for _ in 0..count(2.0 + ctx.rng.random() * 3.0) {
            let nx = norm_rand(ctx, -30.0, 30.0).clamp(gxmin, gxmax);
            let hei = 60.0 + ctx.rng.random() * 40.0;
            shapes.extend(trees::tree08(ctx, xr + nx, yr, hei, 1.0, depth));
        }
    }

    match tt {
        0 => {
            for _ in 0..count(ctx.rng.random() * 3.0) {
                let x = xoff + norm_rand(ctx, gxmin, gxmax);
                let y = yoff + mid_y + norm_rand(ctx, -5.0, 5.0) + 20.0;
                let seed = ctx.rng.random() * 100.0;
                let wid = 50.0 + ctx.rng.random() * 20.0;
                let hei = 40.0 + ctx.rng.random() * 20.0;
                shapes.extend(rock(ctx, x, y, seed, hei, wid, 40, 5.0, depth));
            }
        }
        1 => {
            let pmin = ctx.rng.random() * 0.5;
            let pmax = ctx.rng.random() * 0.5 + 0.5;
            let xmin = gxmin * (1.0 - pmin) + gxmax * pmin;
            let xmax = gxmin * (1.0 - pmax) + gxmax * pmax;
            let mut i = xmin;
            while i < xmax {
                let x = xoff + i + 20.0 * norm_rand(ctx, -1.0, 1.0);
                let y = yoff + mid_y + 20.0;
                let hei = 100.0 + ctx.rng.random() * 200.0;
                shapes.extend(trees::tree05(ctx, x, y, hei, 5.0, depth));
                i += 30.0;
            }
            for _ in 0..count(ctx.rng.random() * 4.0) {
                let x = xoff + norm_rand(ctx, gxmin, gxmax);
                let y = yoff + mid_y + norm_rand(ctx, -5.0, 5.0) + 20.0;
                let seed = ctx.rng.random() * 100.0;
                let wid = 50.0 + ctx.rng.random() * 20.0;
                let hei = 40.0 + ctx.rng.random() * 20.0;
                shapes.extend(rock(ctx, x, y, seed, hei, wid, 40, 5.0, depth));
            }
        }
        2 => {
            for i in 0..(*rand_choice(ctx, &[1, 1, 1, 1, 2, 2, 3]) as usize) {
                let xr = norm_rand(ctx, gxmin, gxmax);
                let yr = mid_y;
                shapes.extend(trees::tree04(
                    ctx,
                    xoff + xr,
                    yoff + yr + 20.0,
                    300.0,
                    6.0,
                    depth,
                ));
                for j in 0..count(ctx.rng.random() * 2.0) {
                    let cx = (xr + norm_rand(ctx, -50.0, 50.0)).clamp(gxmin, gxmax);
                    let ry = yoff + yr + norm_rand(ctx, -5.0, 5.0) + 20.0;
                    let seed = (j * i) as f64 * ctx.rng.random() * 100.0;
                    let wid = 50.0 + ctx.rng.random() * 20.0;
                    let hei = 40.0 + ctx.rng.random() * 20.0;
                    shapes.extend(rock(ctx, xoff + cx, ry, seed, hei, wid, 40, 5.0, depth));
                }
            }
        }
        3 => {
            for _ in 0..(*rand_choice(ctx, &[1, 1, 1, 1, 2, 2, 3]) as usize) {
                let x = xoff + norm_rand(ctx, gxmin, gxmax);
                let y = yoff + mid_y;
                let hei = 60.0 + ctx.rng.random() * 60.0;
                shapes.extend(trees::tree06(ctx, x, y, hei, 6.0, depth));
            }
        }
        4 => {
            let pmin = ctx.rng.random() * 0.5;
            let pmax = ctx.rng.random() * 0.5 + 0.5;
            let xmin = gxmin * (1.0 - pmin) + gxmax * pmin;
            let xmax = gxmin * (1.0 - pmax) + gxmax * pmax;
            let mut i = xmin;
            while i < xmax {
                let x = xoff + i + 20.0 * norm_rand(ctx, -1.0, 1.0);
                let y = yoff + mid_y + norm_rand(ctx, -1.0, 1.0);
                let hei = norm_rand(ctx, 40.0, 80.0);
                shapes.extend(trees::tree07(ctx, x, y, hei, 4.0, None, depth));
                i += 20.0;
            }
        }
        _ => {}
    }

    for _ in 0..count(50.0 * ctx.rng.random()) {
        let x = xoff + norm_rand(ctx, gxmin, gxmax);
        let y = yoff + norm_rand(ctx, gymin, gymax);
        shapes.extend(trees::tree02(ctx, x, y, 16.0, 8.0, 5, depth, None));
    }

    let ts = *rand_choice(ctx, &[0, 0, 0, 0, 1]);
    if ts == 1 && tt != 4 {
        let x = xoff + norm_rand(ctx, gxmin, gxmax);
        let y = yoff + mid_y + 20.0;
        let seed = ctx.rng.random();
        let wid = norm_rand(ctx, 160.0, 200.0);
        let hei = norm_rand(ctx, 80.0, 100.0);
        let per = ctx.rng.random();
        shapes.extend(arch::arch01(ctx, x, y, seed, hei, wid, 0.7, per, depth));
    }
    shapes
}

pub fn dist_mount(
    ctx: &mut Ctx,
    xoff: f64,
    yoff: f64,
    seed: f64,
    hei: Option<f64>,
    len_: f64,
    seg: i32,
    depth: f64,
) -> Vec<Shape> {
    let hei = hei.unwrap_or(300.0);
    let mut shapes: Vec<Shape> = Vec::new();
    let span = 10.0;
    let mut ptlist: Vec<Vec<Point>> = Vec::new();

    for i in 0..count(len_ / span / seg as f64) {
        let mut row: Vec<Point> = Vec::new();
        for j in 0..=seg {
            let k = i as i32 * seg + j;
            row.push((
                xoff + k as f64 * span,
                yoff - hei
                    * ctx.noise.noise(k as f64 * 0.05, seed, 0.0)
                    * ((std::f64::consts::PI * k as f64) / (len_ / span))
                        .sin()
                        .powf(0.5),
            ));
        }
        for j in 0..count(seg as f64 / 2.0 + 1.0) {
            let k = i as i32 * seg + j as i32 * 2;
            row.insert(
                0,
                (
                    xoff + k as f64 * span,
                    yoff + 24.0
                        * ctx.noise.noise(k as f64 * 0.05, 2.0, seed)
                        * ((std::f64::consts::PI * k as f64) / (len_ / span)).sin(),
                ),
            );
        }
        ptlist.push(row);
    }

    for row in &ptlist {
        if row.is_empty() {
            continue;
        }
        let last = row[row.len() - 1];
        let base = ctx
            .palette
            .distant_at(ctx.noise.noise(last.0 * 0.02, last.1 * 0.02, yoff), depth);
        shapes.push(Shape::Poly(Poly::new(row.clone(), Some(base))));
        let tris = triangulate(row, 100.0, true, false);
        for tri in tris {
            let m = mid_pt(&tri);
            let co = ctx
                .palette
                .distant_at(ctx.noise.noise(m.0 * 0.02, m.1 * 0.02, yoff), depth);
            shapes.push(Shape::Poly(Poly::stroked(tri, Some(co), co, 1.0)));
        }
    }
    shapes
}

pub fn rock(
    ctx: &mut Ctx,
    xoff: f64,
    yoff: f64,
    seed: f64,
    hei: f64,
    wid: f64,
    tex: usize,
    sha: f64,
    depth: f64,
) -> Vec<Shape> {
    let mut shapes: Vec<Shape> = Vec::new();
    let reso = (10usize, 50usize);
    let mut ptlist: Vec<Vec<Point>> = Vec::new();

    for i in 0..reso.0 {
        let mut row: Vec<Point> = Vec::new();
        let mut nslist: Vec<f64> = Vec::new();
        for j in 0..reso.1 {
            nslist.push(ctx.noise.noise(i as f64, j as f64 * 0.2, seed));
        }
        loop_noise(&mut nslist);
        for j in 0..reso.1 {
            let a = (j as f64 / reso.1 as f64) * std::f64::consts::PI * 2.0
                - std::f64::consts::PI / 2.0;
            let mut l = (wid * hei) / ((hei * a.cos()).powi(2) + (wid * a.sin()).powi(2)).sqrt();
            l *= 0.7 + 0.3 * nslist[j];
            let p = 1.0 - i as f64 / reso.0 as f64;
            let nx = a.cos() * l * p;
            let mut ny = -a.sin() * l * p;
            if std::f64::consts::PI < a || a < 0.0 {
                ny *= 0.2;
            }
            ny += hei * (i as f64 / reso.0 as f64) * 0.2;
            row.push((nx, ny));
        }
        ptlist.push(row);
    }

    let mut outline = ptlist[0].clone();
    outline.push((0.0, 0.0));
    shapes.push(Shape::Poly(Poly::new(
        outline
            .into_iter()
            .map(|p| (p.0 + xoff, p.1 + yoff))
            .collect(),
        Some(ctx.palette.fill),
    )));

    let outline_col = ctx.palette.ink_at(0.3, depth);
    let shifted: Vec<Point> = ptlist[0].iter().map(|p| (p.0 + xoff, p.1 + yoff)).collect();
    shapes.extend(stroke(
        ctx,
        &shifted,
        0.0,
        0.0,
        3.0,
        Some(outline_col),
        1.0,
        1.0,
        None,
    ));

    let shade = ctx.palette.ink_at(0.1, depth);
    shapes.extend(texture(
        ctx,
        &ptlist,
        xoff,
        yoff,
        tex,
        3.0,
        0.2,
        sha,
        None,
        Some(&mut |ctx: &mut Ctx, _t: f64| {
            ctx.palette.rock_at(0.3 + ctx.rng.random() * 0.3, depth)
        }),
        Some(&mut |ctx: &mut Ctx| {
            if ctx.rng.random() > 0.5 {
                0.15 + 0.15 * ctx.rng.random()
            } else {
                0.85 - 0.15 * ctx.rng.random()
            }
        }),
        Some(shade),
    ));
    shapes
}

#[allow(dead_code)]
fn _color_helper(c: Color) -> Color {
    c
}
