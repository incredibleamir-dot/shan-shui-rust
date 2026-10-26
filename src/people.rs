//! Small human figures, hats, walking sticks and cloth.

use std::f64::consts::PI;

use crate::color::Color;
use crate::context::Ctx;
use crate::primitives::bez_mh;
use crate::shapes::{Point, Poly, Shape};

type HatFn = fn(&mut Ctx, Point, Point, bool) -> Vec<Shape>;

pub fn expand(ptlist: &[Point], wfun: impl Fn(f64) -> f64) -> (Vec<Point>, Vec<Point>) {
    let mut vtxlist0: Vec<Point> = Vec::new();
    let mut vtxlist1: Vec<Point> = Vec::new();
    if ptlist.len() < 2 {
        return (ptlist.to_vec(), ptlist.to_vec());
    }
    for i in 1..ptlist.len().saturating_sub(1) {
        let w = wfun(i as f64 / ptlist.len() as f64);
        let a1 = (ptlist[i].1 - ptlist[i - 1].1).atan2(ptlist[i].0 - ptlist[i - 1].0);
        let a2 = (ptlist[i].1 - ptlist[i + 1].1).atan2(ptlist[i].0 - ptlist[i + 1].0);
        let mut a = (a1 + a2) / 2.0;
        if a < a2 {
            a += PI;
        }
        vtxlist0.push((ptlist[i].0 + w * a.cos(), ptlist[i].1 + w * a.sin()));
        vtxlist1.push((ptlist[i].0 - w * a.cos(), ptlist[i].1 - w * a.sin()));
    }

    let l = ptlist.len() - 1;
    let a0 = (ptlist[1].1 - ptlist[0].1).atan2(ptlist[1].0 - ptlist[0].0) - PI / 2.0;
    let a1 = (ptlist[l].1 - ptlist[l - 1].1).atan2(ptlist[l].0 - ptlist[l - 1].0) - PI / 2.0;
    let w0 = wfun(0.0);
    let w1 = wfun(1.0);
    vtxlist0.insert(
        0,
        (ptlist[0].0 + w0 * a0.cos(), ptlist[0].1 + w0 * a0.sin()),
    );
    vtxlist1.insert(
        0,
        (ptlist[0].0 - w0 * a0.cos(), ptlist[0].1 - w0 * a0.sin()),
    );
    vtxlist0.push((ptlist[l].0 + w1 * a1.cos(), ptlist[l].1 + w1 * a1.sin()));
    vtxlist1.push((ptlist[l].0 - w1 * a1.cos(), ptlist[l].1 - w1 * a1.sin()));
    (vtxlist0, vtxlist1)
}

pub fn tranpoly(p0: Point, p1: Point, ptlist: &[Point]) -> Vec<Point> {
    let plist: Vec<Point> = ptlist.iter().map(|v| (-v.0, v.1)).collect();
    let ang = (p1.1 - p0.1).atan2(p1.0 - p0.0) - PI / 2.0;
    let scl = (p1.0 - p0.0).hypot(p1.1 - p0.1);
    plist
        .iter()
        .map(|v| {
            let d = v.0.hypot(v.1);
            let a = v.1.atan2(v.0);
            (
                p0.0 + d * scl * (ang + a).cos(),
                p0.1 + d * scl * (ang + a).sin(),
            )
        })
        .collect()
}

pub fn flipper(plist: &[Point]) -> Vec<Point> {
    plist.iter().map(|v| (-v.0, v.1)).collect()
}

fn poly_fill(pts: Vec<Point>, fill: Color) -> Shape {
    Shape::Poly(Poly::new(pts, Some(fill)))
}

fn poly_stroke(pts: Vec<Point>, stroke: Color, width: f64) -> Shape {
    Shape::Poly(Poly {
        pts,
        fill: None,
        stroke: Some(stroke),
        width,
    })
}

pub fn hat01(ctx: &mut Ctx, p0: Point, p1: Point, fli: bool) -> Vec<Shape> {
    let ink = ctx.palette.ink_at(0.8, 1.0);
    let base = [
        (-0.3, 0.5),
        (0.3, 0.8),
        (0.2, 1.0),
        (0.0, 1.1),
        (-0.3, 1.15),
        (-0.55, 1.0),
        (-0.65, 0.5),
    ];
    let pts = if fli { flipper(&base) } else { base.to_vec() };
    let mut shapes = vec![poly_fill(tranpoly(p0, p1, &pts), ink)];

    let seed = ctx.rng.random();
    let mut qlist1: Vec<Point> = Vec::with_capacity(10);
    for i in 0..10 {
        qlist1.push((
            -0.3 - ctx.noise.noise(i as f64 * 0.2, seed, 0.0) * i as f64 * 0.1,
            0.5 - i as f64 * 0.3,
        ));
    }
    let pts = if fli { flipper(&qlist1) } else { qlist1 };
    shapes.push(poly_stroke(tranpoly(p0, p1, &pts), ink, 1.0));
    shapes
}

pub fn hat02(ctx: &mut Ctx, p0: Point, p1: Point, fli: bool) -> Vec<Shape> {
    let ink = ctx.palette.ink_at(0.8, 1.0);
    let base = [
        (-0.3, 0.5),
        (-1.1, 0.5),
        (-1.2, 0.6),
        (-1.1, 0.7),
        (-0.3, 0.8),
        (0.3, 0.8),
        (1.0, 0.7),
        (1.3, 0.6),
        (1.2, 0.5),
        (0.3, 0.5),
    ];
    let pts = if fli { flipper(&base) } else { base.to_vec() };
    vec![poly_fill(tranpoly(p0, p1, &pts), ink)]
}

pub fn stick01(ctx: &mut Ctx, p0: Point, p1: Point, fli: bool) -> Vec<Shape> {
    let seed = ctx.rng.random();
    let mut qlist1: Vec<Point> = Vec::with_capacity(12);
    for i in 0..12 {
        qlist1.push((
            -ctx.noise.noise(i as f64 * 0.1, seed, 0.0)
                * 0.1
                * ((i as f64 / 12.0) * PI).sin()
                * 5.0,
            i as f64 * 0.3,
        ));
    }
    let pts = if fli { flipper(&qlist1) } else { qlist1 };
    let ink = ctx.palette.ink_at(0.5, 1.0);
    vec![poly_stroke(tranpoly(p0, p1, &pts), ink, 1.0)]
}

fn parent_of(index: usize) -> Option<usize> {
    match index {
        0 => None,
        1 => Some(0),
        2 => Some(1),
        3 => Some(0),
        4 => Some(3),
        5 => Some(1),
        6 => Some(5),
        7 => Some(1),
        8 => Some(7),
        _ => None,
    }
}

fn path_to_root(index: usize) -> Vec<usize> {
    let mut path = Vec::new();
    let mut cur = Some(index);
    while let Some(v) = cur {
        path.push(v);
        cur = parent_of(v);
    }
    path.reverse();
    path
}

fn global_pos(index: usize, ang: &[f64], lengths: &[f64]) -> Point {
    let mut px = 0.0;
    let mut py = 0.0;
    let mut angle = 0.0;
    for p in path_to_root(index) {
        angle += ang[p];
        px += lengths[p] * angle.cos();
        py += lengths[p] * angle.sin();
    }
    (px, py)
}

pub fn man(
    ctx: &mut Ctx,
    xoff: f64,
    yoff: f64,
    sca: f64,
    hat: Option<HatFn>,
    ite: Option<HatFn>,
    fli: bool,
    ang: Option<&[f64]>,
    limb_len: Option<&[f64]>,
) -> Vec<Shape> {
    let ang_storage: Vec<f64>;
    let ang: &[f64] = match ang {
        Some(v) => v,
        None => {
            let r3 = ctx.rng.random();
            let r4 = ctx.rng.random();
            let r7 = ctx.rng.random();
            ang_storage = vec![
                0.0,
                -PI / 2.0,
                0.0,
                (PI / 4.0) * r3,
                ((PI * 3.0) / 4.0) * r4,
                (PI * 3.0) / 4.0,
                -PI / 4.0,
                (-PI * 3.0) / 4.0 - (PI / 4.0) * r7,
                -PI / 4.0,
            ];
            &ang_storage
        }
    };
    let default_limbs = [0.0, 30.0, 20.0, 30.0, 30.0, 30.0, 30.0, 30.0, 30.0];
    let limb_len = limb_len.unwrap_or(&default_limbs);
    let lengths: Vec<f64> = limb_len.iter().map(|v| v * sca).collect();

    let mut pts: Vec<Point> = Vec::with_capacity(9);
    for i in 0..9 {
        pts.push(global_pos(i, ang, &lengths));
    }
    let yoff = yoff - pts[4].1;

    let to_global = |v: Point| -> Point { (if fli { -v.0 } else { v.0 } + xoff, v.1 + yoff) };

    let cloth = |ctx: &mut Ctx, plist: &[Point], fun: &dyn Fn(f64) -> f64| -> Vec<Shape> {
        let tlist = bez_mh(plist, 2.0);
        let (tlist1, tlist2) = expand(&tlist, fun);
        let fill = ctx.palette.fill.with_alpha(1.0);
        let ink1 = ctx.palette.ink_at(0.5, 1.0);
        let ink2 = ctx.palette.ink_at(0.6, 1.0);
        let mut pts: Vec<Point> = tlist1.clone();
        pts.extend(tlist2.iter().rev().copied());
        vec![
            poly_fill(pts.iter().map(|p| to_global(*p)).collect(), fill),
            poly_stroke(tlist1.iter().map(|p| to_global(*p)).collect(), ink1, 1.0),
            poly_stroke(tlist2.iter().map(|p| to_global(*p)).collect(), ink2, 1.0),
        ]
    };

    let fsleeve = |x: f64| {
        sca * 8.0 * ((0.5 * x * PI).sin() * (x * PI).sin().max(0.0).powf(0.1) + (1.0 - x) * 0.4)
    };
    let fbody = |x: f64| {
        sca * 11.0 * ((0.5 * x * PI).sin() * (x * PI).sin().max(0.0).powf(0.1) + (1.0 - x) * 0.5)
    };
    let fhead = |x: f64| sca * 7.0 * (0.0f64.max(0.25 - (x - 0.5).powi(2))).powf(0.3);

    let mut shapes: Vec<Shape> = Vec::new();
    if let Some(ite_fn) = ite {
        shapes.extend(ite_fn(ctx, to_global(pts[8]), to_global(pts[6]), fli));
    }
    shapes.extend(cloth(ctx, &[pts[1], pts[7], pts[8]], &fsleeve));
    shapes.extend(cloth(ctx, &[pts[1], pts[0], pts[3], pts[4]], &fbody));
    shapes.extend(cloth(ctx, &[pts[1], pts[5], pts[6]], &fsleeve));
    shapes.extend(cloth(ctx, &[pts[1], pts[2]], &fhead));

    let hlist = bez_mh(&[pts[1], pts[2]], 2.0);
    let (hlist1, hlist2) = expand(&hlist, fhead);
    let hs = (hlist1.len() as f64 * 0.1) as usize;
    let he = (hlist2.len() as f64 * 0.95) as usize;
    let hlist1 = &hlist1[hs.min(hlist1.len())..];
    let hlist2 = &hlist2[he.min(hlist2.len())..];
    let mut head_pts: Vec<Point> = hlist1.to_vec();
    head_pts.extend(hlist2.iter().rev().copied());
    let ink = ctx.palette.ink_at(0.6, 1.0);
    shapes.push(poly_fill(
        head_pts.iter().map(|p| to_global(*p)).collect(),
        ink,
    ));

    let hat_fn = hat.unwrap_or(hat01);
    shapes.extend(hat_fn(ctx, to_global(pts[1]), to_global(pts[2]), fli));
    shapes
}
