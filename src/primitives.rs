use crate::color::Color;
use crate::context::Ctx;
use crate::geometry::mid_pt2;
use crate::shapes::{Point, Poly, Shape};

pub fn distance(p0: Point, p1: Point) -> f64 {
    (p0.0 - p1.0).hypot(p0.1 - p1.1)
}

pub fn mapval(value: f64, istart: f64, istop: f64, ostart: f64, ostop: f64) -> f64 {
    ostart + (ostop - ostart) * ((value - istart) / (istop - istart))
}

/// Matches the original's in-place normalisation, including its divide-by-zero
/// guard for single samples.
pub fn loop_noise(nslist: &mut [f64]) {
    if nslist.is_empty() {
        return;
    }
    let dif = nslist[nslist.len() - 1] - nslist[0];
    let last = nslist.len() - 1;
    let mut bds = [100.0f64, -100.0f64];
    for i in 0..nslist.len() {
        if last != 0 {
            nslist[i] += (dif * (last - i) as f64) / last as f64;
        }
        bds[0] = bds[0].min(nslist[i]);
        bds[1] = bds[1].max(nslist[i]);
    }
    if bds[0] == bds[1] {
        return;
    }
    for value in nslist.iter_mut() {
        *value = mapval(*value, bds[0], bds[1], 0.0, 1.0);
    }
}

pub fn rand_choice<'a, T>(ctx: &mut Ctx, arr: &'a [T]) -> &'a T {
    &arr[(arr.len() as f64 * ctx.rng.random()) as usize]
}

pub fn rand_choice_i32(ctx: &mut Ctx, arr: &[i32]) -> i32 {
    *rand_choice(ctx, arr)
}

pub fn rand_choice_f64(ctx: &mut Ctx, arr: &[f64]) -> f64 {
    *rand_choice(ctx, arr)
}

pub fn rand_choice_bool(ctx: &mut Ctx, arr: &[bool]) -> bool {
    *rand_choice(ctx, arr)
}

pub fn norm_rand(ctx: &mut Ctx, m: f64, big_m: f64) -> f64 {
    mapval(ctx.rng.random(), 0.0, 1.0, m, big_m)
}

pub fn wtrand(ctx: &mut Ctx, func: impl Fn(f64) -> f64) -> f64 {
    loop {
        let x = ctx.rng.random();
        let y = ctx.rng.random();
        if y < func(x) {
            return x;
        }
    }
}

pub fn rand_gaussian(ctx: &mut Ctx) -> f64 {
    wtrand(ctx, |x| (-24.0 * (x - 0.5).powi(2)).exp()) * 2.0 - 1.0
}

pub fn bez_mh(points: &[Point], w: f64) -> Vec<Point> {
    let owned;
    let points: &[Point] = if points.len() == 2 {
        owned = vec![points[0], mid_pt2(points[0], points[1]), points[1]];
        &owned
    } else {
        points
    };
    let mut plist = Vec::new();
    for j in 0..points.len().saturating_sub(2) {
        let p0 = if j == 0 {
            points[j]
        } else {
            mid_pt2(points[j], points[j + 1])
        };
        let p1 = points[j + 1];
        let p2 = if j == points.len() - 3 {
            points[j + 2]
        } else {
            mid_pt2(points[j + 1], points[j + 2])
        };
        let pl = 20usize;
        let extra = if j == points.len() - 3 { 1 } else { 0 };
        for i in 0..pl + extra {
            let t = i as f64 / pl as f64;
            let u = (1.0 - t).powi(2) + 2.0 * t * (1.0 - t) * w + t * t;
            plist.push((
                ((1.0 - t).powi(2) * p0.0 + 2.0 * t * (1.0 - t) * p1.0 * w + t * t * p2.0) / u,
                ((1.0 - t).powi(2) * p0.1 + 2.0 * t * (1.0 - t) * p1.1 * w + t * t * p2.1) / u,
            ));
        }
    }
    plist
}

pub fn div(plist: &[Point], reso: f64) -> Vec<Point> {
    if plist.is_empty() {
        return Vec::new();
    }
    let steps = reso as usize;
    if plist.len() < 2 || steps == 0 {
        return plist.to_vec();
    }
    let tl = (plist.len() - 1) * steps;
    let mut rlist = Vec::with_capacity(tl + 1);
    for i in 0..tl {
        let lastp = plist[i / steps];
        let nextp = plist[(i / steps + if i % steps != 0 { 1 } else { 0 }).min(plist.len() - 1)];
        let p = (i % steps) as f64 / steps as f64;
        let nx = lastp.0 * (1.0 - p) + nextp.0 * p;
        let ny = lastp.1 * (1.0 - p) + nextp.1 * p;
        rlist.push((nx, ny));
    }
    rlist.push(plist[plist.len() - 1]);
    rlist
}

pub fn poly(
    pts: &[Point],
    xof: f64,
    yof: f64,
    fill: Option<Color>,
    mut stroke: Option<Color>,
    wid: f64,
) -> Vec<Shape> {
    if pts.is_empty() {
        return Vec::new();
    }
    if wid > 0.0 && stroke.is_none() && fill.is_some() {
        stroke = fill;
    }
    if fill.is_none() && stroke.is_none() {
        return Vec::new();
    }
    let moved = pts.iter().map(|p| (p.0 + xof, p.1 + yof)).collect();
    vec![Shape::Poly(Poly {
        pts: moved,
        fill,
        stroke,
        width: wid,
    })]
}

pub fn stroke(
    ctx: &mut Ctx,
    ptlist: &[Point],
    xof: f64,
    yof: f64,
    wid: f64,
    col: Option<Color>,
    noi: f64,
    out: f64,
    fun: Option<&dyn Fn(f64) -> f64>,
) -> Vec<Shape> {
    if ptlist.is_empty() {
        return Vec::new();
    }
    let col = col.unwrap_or_else(|| ctx.palette.ink_at(0.9, 1.0));
    let default_fun = |x: f64| (x * std::f64::consts::PI).sin();
    let fun: &dyn Fn(f64) -> f64 = fun.unwrap_or(&default_fun);

    let mut vtxlist0: Vec<Point> = Vec::new();
    let mut vtxlist1: Vec<Point> = Vec::new();
    let n0 = ctx.rng.random() * 10.0;
    for i in 1..ptlist.len().saturating_sub(1) {
        let mut w = wid * fun(i as f64 / ptlist.len() as f64);
        w = w * (1.0 - noi) + w * noi * ctx.noise.noise(i as f64 * 0.5, n0, 0.0);
        let a1 = (ptlist[i].1 - ptlist[i - 1].1).atan2(ptlist[i].0 - ptlist[i - 1].0);
        let a2 = (ptlist[i].1 - ptlist[i + 1].1).atan2(ptlist[i].0 - ptlist[i + 1].0);
        let mut a = (a1 + a2) / 2.0;
        if a < a2 {
            a += std::f64::consts::PI;
        }
        vtxlist0.push((ptlist[i].0 + w * a.cos(), ptlist[i].1 + w * a.sin()));
        vtxlist1.push((ptlist[i].0 - w * a.cos(), ptlist[i].1 - w * a.sin()));
    }

    let mut vtxlist = Vec::with_capacity(vtxlist0.len() * 2 + 3);
    vtxlist.push(ptlist[0]);
    vtxlist.extend(vtxlist0);
    let mut tail = vtxlist1;
    tail.push(ptlist[ptlist.len() - 1]);
    vtxlist.extend(tail.into_iter().rev());
    vtxlist.push(ptlist[0]);

    poly(
        &vtxlist,
        xof,
        yof,
        Some(col),
        if out > 0.0 { Some(col) } else { None },
        out,
    )
}

pub fn blob_points(
    ctx: &mut Ctx,
    x: f64,
    y: f64,
    len_: f64,
    wid: f64,
    ang: f64,
    noi: f64,
    fun: Option<&dyn Fn(f64) -> f64>,
) -> Vec<Point> {
    let default_fun = |p: f64| {
        if p <= 1.0 {
            (p * std::f64::consts::PI).sin().max(0.0).powf(0.5)
        } else {
            -(((p + 1.0) * std::f64::consts::PI).sin()).powf(0.5)
        }
    };
    let fun: &dyn Fn(f64) -> f64 = fun.unwrap_or(&default_fun);

    let reso = 20usize;
    let mut lalist: Vec<(f64, f64)> = Vec::with_capacity(reso + 1);
    for i in 0..=reso {
        let p = (i as f64 / reso as f64) * 2.0;
        let xo = len_ / 2.0 - (p - 1.0).abs() * len_;
        let yo = (fun(p) * wid) / 2.0;
        let a = yo.atan2(xo);
        let l = xo.hypot(yo);
        lalist.push((l, a));
    }

    let mut nslist = Vec::with_capacity(reso + 1);
    let n0 = ctx.rng.random() * 10.0;
    for i in 0..=reso {
        nslist.push(ctx.noise.noise(i as f64 * 0.05, n0, 0.0));
    }
    loop_noise(&mut nslist);

    let mut plist = Vec::with_capacity(reso + 1);
    for (i, (l, a)) in lalist.iter().enumerate() {
        let ns = nslist[i] * noi + (1.0 - noi);
        plist.push((x + (a + ang).cos() * l * ns, y + (a + ang).sin() * l * ns));
    }
    plist
}

pub fn blob(
    ctx: &mut Ctx,
    x: f64,
    y: f64,
    len_: f64,
    wid: f64,
    ang: f64,
    col: Option<Color>,
    noi: f64,
    fun: Option<&dyn Fn(f64) -> f64>,
) -> Vec<Shape> {
    let col = col.unwrap_or_else(|| ctx.palette.ink_at(0.9, 1.0));
    let pts = blob_points(ctx, x, y, len_, wid, ang, noi, fun);
    poly(&pts, 0.0, 0.0, Some(col), None, 0.0)
}

pub fn texture(
    ctx: &mut Ctx,
    ptlist: &[Vec<Point>],
    xof: f64,
    yof: f64,
    tex: usize,
    wid: f64,
    len_: f64,
    sha: f64,
    noi: Option<&dyn Fn(f64) -> f64>,
    col: Option<&mut dyn FnMut(&mut Ctx, f64) -> Color>,
    dis: Option<&mut dyn FnMut(&mut Ctx) -> f64>,
    shade_col: Option<Color>,
) -> Vec<Shape> {
    if ptlist.is_empty() || ptlist[0].is_empty() {
        return Vec::new();
    }

    let default_noi = |x: f64| 30.0 / x;
    let noi: &dyn Fn(f64) -> f64 = noi.unwrap_or(&default_noi);
    let mut default_col =
        |ctx: &mut Ctx, _t: f64| Color::new(100, 100, 100, ctx.rng.random() * 0.3);
    let col_fn: &mut dyn FnMut(&mut Ctx, f64) -> Color = match col {
        Some(f) => f,
        None => &mut default_col,
    };
    let mut default_dis = |ctx: &mut Ctx| -> f64 {
        if ctx.rng.random() > 0.5 {
            (1.0 / 3.0) * ctx.rng.random()
        } else {
            2.0 / 3.0 + (1.0 / 3.0) * ctx.rng.random()
        }
    };
    let dis_fn: &mut dyn FnMut(&mut Ctx) -> f64 = match dis {
        Some(f) => f,
        None => &mut default_dis,
    };
    let shade_col = shade_col.unwrap_or_else(|| ctx.palette.ink_at(0.1, 1.0));

    let reso = [ptlist.len(), ptlist[0].len()];
    let mut texlist: Vec<Vec<Point>> = Vec::with_capacity(tex);
    for i in 0..tex {
        let mid = (dis_fn(ctx) * reso[1] as f64) as i32;
        let hlen = (ctx.rng.random() * (reso[1] as f64 * len_)) as i32;
        let start = 0.max((mid - hlen).min(reso[1] as i32));
        let end = 0.max((mid + hlen).min(reso[1] as i32));
        let layer = (i as f64 / tex as f64) * (reso[0] - 1) as f64;

        let mut row: Vec<Point> = Vec::new();
        for j in start..end {
            let p = layer - layer.floor();
            let r0 = (layer.floor() as usize).min(reso[0] - 1);
            let r1 = (layer.ceil() as usize).min(reso[0] - 1);
            let j = j as usize;
            let x = ptlist[r0][j].0 * p + ptlist[r1][j].0 * (1.0 - p);
            let y = ptlist[r0][j].1 * p + ptlist[r1][j].1 * (1.0 - p);
            let ns = noi(layer + 1.0);
            let nx = x + ns * (ctx.noise.noise(x, j as f64 * 0.5, 0.0) - 0.5);
            let ny = y + ns * (ctx.noise.noise(y, j as f64 * 0.5, 0.0) - 0.5);
            row.push((nx, ny));
        }
        texlist.push(row);
    }

    let mut shapes: Vec<Shape> = Vec::new();
    if sha != 0.0 {
        let step = (1.0 + sha) as usize;
        if step > 0 {
            let mut j = 0usize;
            while j < texlist.len() {
                let shifted: Vec<Point> =
                    texlist[j].iter().map(|p| (p.0 + xof, p.1 + yof)).collect();
                shapes.extend(stroke(
                    ctx,
                    &shifted,
                    0.0,
                    0.0,
                    sha,
                    Some(shade_col),
                    0.5,
                    1.0,
                    None,
                ));
                j += step;
            }
        }
    }
    let step = (1.0 + sha) as usize;
    if step > 0 {
        let start = sha as usize;
        let mut j = start;
        while j < texlist.len() {
            let col = col_fn(ctx, j as f64 / texlist.len() as f64);
            let shifted: Vec<Point> = texlist[j].iter().map(|p| (p.0 + xof, p.1 + yof)).collect();
            shapes.extend(stroke(
                ctx,
                &shifted,
                0.0,
                0.0,
                wid,
                Some(col),
                0.5,
                1.0,
                None,
            ));
            j += step;
        }
    }
    shapes
}
