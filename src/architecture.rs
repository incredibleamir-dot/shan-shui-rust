//! Huts, pavilions, pagodas, boats and transmission towers.
//!
//! Ported closely from the original Python module. In particular the order in
//! which random numbers are consumed is identical, so seeded paintings match.

use std::f64::consts::PI;

use crate::color::Color;
use crate::context::Ctx;
use crate::geometry::mid_pt2;
use crate::people::{hat02, man, stick01};
use crate::primitives::{div, norm_rand, rand_choice, stroke, texture, wtrand};
use crate::shapes::{Point, Poly, Shape, Text};

/// Anchors handed to the decoration callback of a box.
pub struct DecoAnchors {
    pub pul: Point,
    pub pur: Point,
    pub pdl: Point,
    pub pdr: Point,
}

fn flip_points(pts: &[Point]) -> Vec<Point> {
    pts.iter().map(|p| (-p.0, p.1)).collect()
}

/// Python's `mid_pt` with two arguments.
fn mid2(a: Point, b: Point) -> Point {
    mid_pt2(a, b)
}

fn filled(pts: Vec<Point>, fill: Color) -> Shape {
    Shape::Poly(Poly::new(pts, Some(fill)))
}

fn stroked(pts: Vec<Point>, col: Color, width: f64) -> Shape {
    Shape::Poly(Poly {
        pts,
        fill: None,
        stroke: Some(col),
        width,
    })
}

pub fn hut(
    ctx: &mut Ctx,
    xoff: f64,
    yoff: f64,
    hei: f64,
    wid: f64,
    tex: usize,
    depth: f64,
) -> Vec<Shape> {
    let reso = [10usize, 10usize];
    let mut ptlist: Vec<Vec<Point>> = Vec::with_capacity(reso[0]);
    for i in 0..reso[0] {
        let heir = hei + hei * 0.2 * ctx.rng.random();
        let mut row: Vec<Point> = Vec::with_capacity(reso[1]);
        for j in 0..reso[1] {
            let nx = wid
                * (i as f64 / (reso[0] - 1) as f64 - 0.5)
                * (j as f64 / (reso[1] - 1) as f64).powf(0.7);
            let ny = heir * (j as f64 / (reso[1] - 1) as f64);
            row.push((nx, ny));
        }
        ptlist.push(row);
    }

    let wall = ctx.palette.wall_at(0.85, depth);
    let ink = ctx.palette.ink_at(0.3, depth);

    let mut shapes: Vec<Shape> = Vec::new();
    let mut body: Vec<Point> = ptlist[0][..ptlist[0].len() - 1].to_vec();
    body.extend(
        ptlist[ptlist.len() - 1][..ptlist[ptlist.len() - 1].len() - 1]
            .iter()
            .rev()
            .copied(),
    );
    shapes.push(filled(
        body.iter().map(|p| (p.0 + xoff, p.1 + yoff)).collect(),
        wall,
    ));
    for row in [&ptlist[0], &ptlist[ptlist.len() - 1]] {
        shapes.push(stroked(
            row.iter().map(|p| (p.0 + xoff, p.1 + yoff)).collect(),
            ink,
            2.0,
        ));
    }

    let shade = ctx.palette.ink_at(0.1, depth);
    shapes.extend(texture(
        ctx,
        &ptlist,
        xoff,
        yoff,
        tex,
        1.0,
        0.25,
        0.0,
        Some(&|_x: f64| 5.0),
        Some(&mut |ctx: &mut Ctx, _t: f64| {
            ctx.palette.wash_at(0.3 + ctx.rng.random() * 0.3, depth)
        }),
        Some(&mut |ctx: &mut Ctx| wtrand(ctx, |a| a * a)),
        Some(shade),
    ));
    shapes
}

#[allow(clippy::too_many_arguments)]
pub fn box_(
    ctx: &mut Ctx,
    xoff: f64,
    yoff: f64,
    hei: f64,
    wid: f64,
    rot: f64,
    per: f64,
    tra: bool,
    bot: bool,
    wei: f64,
    dec: Option<&dyn Fn(&DecoAnchors) -> Vec<Vec<Point>>>,
    depth: f64,
) -> Vec<Shape> {
    let mid = -wid * 0.5 + wid * rot;
    let bmid = -wid * 0.5 + wid * (1.0 - rot);
    let mut ptlist: Vec<Vec<Point>> = vec![
        div(&[(-wid * 0.5, -hei), (-wid * 0.5, 0.0)], 5.0),
        div(&[(wid * 0.5, -hei), (wid * 0.5, 0.0)], 5.0),
    ];
    if bot {
        ptlist.push(div(&[(-wid * 0.5, 0.0), (mid, per)], 5.0));
        ptlist.push(div(&[(wid * 0.5, 0.0), (mid, per)], 5.0));
    }
    ptlist.push(div(&[(mid, -hei), (mid, per)], 5.0));
    if tra {
        if bot {
            ptlist.push(div(&[(-wid * 0.5, 0.0), (bmid, -per)], 5.0));
            ptlist.push(div(&[(wid * 0.5, 0.0), (bmid, -per)], 5.0));
        }
        ptlist.push(div(&[(bmid, -hei), (bmid, -per)], 5.0));
    }

    let surf = if rot < 0.5 { 1.0 } else { -1.0 };
    if let Some(dec) = dec {
        ptlist.extend(dec(&DecoAnchors {
            pul: (surf * wid * 0.5, -hei),
            pur: (mid, -hei + per),
            pdl: (surf * wid * 0.5, 0.0),
            pdr: (mid, per),
        }));
    }

    let polist = [
        (-wid * 0.5, -hei),
        (wid * 0.5, -hei),
        (wid * 0.5, 0.0),
        (mid, per),
        (-wid * 0.5, 0.0),
    ];
    let mut shapes: Vec<Shape> = Vec::new();
    let fill = ctx.palette.fill;
    if !tra {
        shapes.push(filled(
            polist.iter().map(|p| (p.0 + xoff, p.1 + yoff)).collect(),
            fill,
        ));
    }
    let ink = ctx.palette.ink_at(0.4, depth);
    for line in &ptlist {
        let shifted: Vec<Point> = line.iter().map(|p| (p.0 + xoff, p.1 + yoff)).collect();
        shapes.extend(stroke(
            ctx,
            &shifted,
            0.0,
            0.0,
            wei,
            Some(ink),
            1.0,
            1.0,
            Some(&|_x: f64| 1.0),
        ));
    }
    shapes
}

/// Intersecting beams on the face of a box.
///
/// `style` mirrors the original: 1 (vertical sides), 2 (horizontal bands) and
/// 3 (a lattice). `hsp` and `vsp` are the horizontal/vertical spacing pairs.
pub fn deco(
    style: i32,
    pul: Point,
    pur: Point,
    pdl: Point,
    pdr: Point,
    hsp: (f64, f64),
    vsp: (f64, f64),
) -> Vec<Vec<Point>> {
    let mut plist: Vec<Vec<Point>> = Vec::new();
    if !(1..=3).contains(&style) {
        return plist;
    }
    let dl = div(&[pul, pdl], vsp.1);
    let dr = div(&[pur, pdr], vsp.1);
    let du = div(&[pul, pur], hsp.1);
    let dd = div(&[pdl, pdr], hsp.1);

    let hi = (hsp.0 as usize).min(du.len().saturating_sub(1));
    let mi = (vsp.0 as usize).max(1);

    if style == 1 {
        if du.is_empty() || dd.is_empty() || dl.is_empty() || dr.is_empty() {
            return plist;
        }
        let mlu = du[hi];
        let mru = du[du.len() - 1 - hi];
        let mld = dd[hi];
        let mrd = dd[du.len() - 1 - hi];
        let mut i = vsp.0 as usize;
        while (i as f64) < dl.len() as f64 - vsp.0 {
            let left = div(&[mlu, mld], vsp.1);
            let right = div(&[mru, mrd], vsp.1);
            if let (Some(mml), Some(mmr)) = (left.get(i), right.get(i)) {
                if let Some(p) = dl.get(i) {
                    plist.push(div(&[*mml, *p], 5.0));
                }
                if let Some(p) = dr.get(i) {
                    plist.push(div(&[*mmr, *p], 5.0));
                }
            }
            i += mi;
        }
        plist.push(div(&[mlu, mld], 5.0));
        plist.push(div(&[mru, mrd], 5.0));
    } else if style == 2 {
        let mut i = hsp.0 as usize;
        while (i as f64) < du.len() as f64 - hsp.0 {
            if let (Some(u), Some(d)) = (du.get(i), dd.get(i)) {
                plist.push(div(&[*u, *d], 5.0));
            }
            i += mi;
        }
    } else if style == 3 {
        if du.is_empty() || dd.is_empty() || dl.is_empty() || dr.is_empty() {
            return plist;
        }
        let mlu = du[hi];
        let mru = du[du.len() - 1 - hi];
        let mld = dd[hi];
        let mrd = dd[du.len() - 1 - hi];
        let mut i = vsp.0 as usize;
        while (i as f64) < dl.len() as f64 - vsp.0 {
            let lr = div(&[mlu, mld], vsp.1);
            let rr = div(&[mru, mrd], vsp.1);
            let ur = div(&[mlu, mru], vsp.1);
            let drr = div(&[mld, mrd], vsp.1);
            if let (Some(mml), Some(mmr), Some(mmu), Some(mmd)) =
                (lr.get(i), rr.get(i), ur.get(i), drr.get(i))
            {
                plist.push(div(&[*mml, *mmr], 5.0));
                plist.push(div(&[*mmu, *mmd], 5.0));
            }
            i += mi;
        }
        plist.push(div(&[mlu, mld], 5.0));
        plist.push(div(&[mru, mrd], 5.0));
    }
    plist
}

#[allow(clippy::too_many_arguments)]
pub fn rail(
    ctx: &mut Ctx,
    xoff: f64,
    yoff: f64,
    seed: f64,
    hei: f64,
    wid: f64,
    rot: f64,
    per: f64,
    seg: i32,
    wei: f64,
    tra: bool,
    fro: bool,
    depth: f64,
) -> Vec<Shape> {
    let mid = -wid * 0.5 + wid * rot;
    let bmid = -wid * 0.5 + wid * (1.0 - rot);
    let steps = seg.max(1) as f64;
    let mut ptlist: Vec<Vec<Point>> = Vec::new();
    if fro {
        ptlist.push(div(&[(-wid * 0.5, 0.0), (mid, per)], steps));
        ptlist.push(div(&[(mid, per), (wid * 0.5, 0.0)], steps));
    }
    if tra {
        ptlist.push(div(&[(-wid * 0.5, 0.0), (bmid, -per)], steps));
        ptlist.push(div(&[(bmid, -per), (wid * 0.5, 0.0)], steps));
    }
    if fro {
        ptlist.push(div(&[(-wid * 0.5, -hei), (mid, -hei + per)], steps));
        ptlist.push(div(&[(mid, -hei + per), (wid * 0.5, -hei)], steps));
    }
    if tra {
        ptlist.push(div(&[(-wid * 0.5, -hei), (bmid, -hei - per)], steps));
        ptlist.push(div(&[(bmid, -hei - per), (wid * 0.5, -hei)], steps));
    }

    if tra && !ptlist.is_empty() {
        let open_idx = (ctx.rng.random() * ptlist.len() as f64) as usize;
        let idx = open_idx.min(ptlist.len() - 1);
        if let Some(row) = ptlist.get_mut(idx) {
            row.pop();
        }
    }

    let ink = ctx.palette.ink_at(0.5, depth);
    let mut shapes: Vec<Shape> = Vec::new();
    let half = ptlist.len() / 2;
    for i in 0..half {
        let mut j = 0usize;
        while j < ptlist[i].len() {
            let other = (half + i) % ptlist.len();
            if ptlist[other].is_empty() {
                j += 1;
                continue;
            }
            let oj = j % ptlist[other].len();
            let (px, py) = ptlist[i][j];
            let (ox, oy) = ptlist[other][oj];
            let py = py + (ctx.noise.noise(i as f64, j as f64 * 0.5, seed) - 0.5) * hei;
            let oy = oy + (ctx.noise.noise(i as f64 + 0.5, j as f64 * 0.5, seed) - 0.5) * hei;
            ptlist[i][j] = (px, py);
            ptlist[other][oj] = (ox, oy);
            let mut ln = div(&[(px, py), (ox, oy)], 2.0);
            if !ln.is_empty() {
                ln[0].0 += (ctx.rng.random() - 0.5) * hei * 0.5;
                shapes.push(stroked(
                    ln.into_iter().map(|p| (p.0 + xoff, p.1 + yoff)).collect(),
                    ink,
                    2.0,
                ));
            }
            j += 1;
        }
    }

    for line in &ptlist {
        let shifted: Vec<Point> = line.iter().map(|p| (p.0 + xoff, p.1 + yoff)).collect();
        shapes.extend(stroke(
            ctx,
            &shifted,
            0.0,
            0.0,
            wei,
            Some(ink),
            0.5,
            1.0,
            Some(&|_x: f64| 1.0),
        ));
    }
    shapes
}

#[allow(clippy::too_many_arguments)]
pub fn roof(
    ctx: &mut Ctx,
    xoff: f64,
    yoff: f64,
    hei: f64,
    wid: f64,
    rot: f64,
    per: f64,
    cor: f64,
    wei: f64,
    pla: Option<(i32, String)>,
    depth: f64,
) -> Vec<Shape> {
    let apply = |pts: &[Point]| -> Vec<Point> {
        if rot < 0.5 {
            flip_points(pts)
        } else {
            pts.to_vec()
        }
    };
    let rrot = if rot < 0.5 { 1.0 - rot } else { rot };
    let mid = -wid * 0.5 + wid * rrot;
    let quat = (mid + wid * 0.5) * 0.5 - mid;

    let ptlist: Vec<Vec<Point>> = vec![
        div(
            &apply(&[
                (-wid * 0.5 + quat, -hei - per / 2.0),
                (-wid * 0.5 + quat * 0.5, -hei / 2.0 - per / 4.0),
                (-wid * 0.5 - cor, 0.0),
            ]),
            5.0,
        ),
        div(
            &apply(&[
                (mid + quat, -hei),
                ((mid + quat + wid * 0.5) / 2.0, -hei / 2.0),
                (wid * 0.5 + cor, 0.0),
            ]),
            5.0,
        ),
        div(
            &apply(&[
                (mid + quat, -hei),
                (mid + quat / 2.0, -hei / 2.0 + per / 2.0),
                (mid + cor, per),
            ]),
            5.0,
        ),
        div(&apply(&[(-wid * 0.5 - cor, 0.0), (mid + cor, per)]), 5.0),
        div(&apply(&[(wid * 0.5 + cor, 0.0), (mid + cor, per)]), 5.0),
        div(
            &apply(&[(-wid * 0.5 + quat, -hei - per / 2.0), (mid + quat, -hei)]),
            5.0,
        ),
    ];

    let polist = apply(&[
        (-wid * 0.5, 0.0),
        (-wid * 0.5 + quat, -hei - per / 2.0),
        (mid + quat, -hei),
        (wid * 0.5, 0.0),
        (mid, per),
    ]);
    let fill = ctx.palette.fill;
    let mut shapes: Vec<Shape> = vec![filled(
        polist.iter().map(|p| (p.0 + xoff, p.1 + yoff)).collect(),
        fill,
    )];
    let ink = ctx.palette.ink_at(0.4, depth);
    for line in &ptlist {
        let shifted: Vec<Point> = line.iter().map(|p| (p.0 + xoff, p.1 + yoff)).collect();
        shapes.extend(stroke(
            ctx,
            &shifted,
            0.0,
            0.0,
            wei,
            Some(ink),
            1.0,
            1.0,
            Some(&|_x: f64| 1.0),
        ));
    }

    if let Some((kind, text)) = pla {
        if kind == 1 {
            let mut pp = apply(&[
                (mid + quat / 2.0, -hei / 2.0 + per / 2.0),
                (-wid * 0.5 + quat * 0.5, -hei / 2.0 - per / 4.0),
            ]);
            if pp[0].0 > pp[1].0 {
                pp.swap(0, 1);
            }
            let mp = mid2(pp[0], pp[1]);
            let a = (pp[1].1 - pp[0].1).atan2(pp[1].0 - pp[0].0);
            let mut t = Text::new(
                mp.0 + xoff,
                mp.1 + yoff,
                text,
                hei * 0.6,
                ctx.palette.ink_at(0.9, depth),
            );
            t.angle = a * 180.0 / PI;
            t.family = "Verdana".into();
            shapes.push(Shape::Text(t));
        }
    }
    shapes
}

#[allow(clippy::too_many_arguments)]
pub fn pagroof(
    ctx: &mut Ctx,
    xoff: f64,
    yoff: f64,
    hei: f64,
    wid: f64,
    _rot: f64,
    per: f64,
    cor: f64,
    sid: i32,
    wei: f64,
    depth: f64,
) -> Vec<Shape> {
    let mut ptlist: Vec<Vec<Point>> = Vec::new();
    let mut polist: Vec<Point> = vec![(0.0, -hei)];
    let denom = (sid.max(2) - 1) as f64;
    for i in 0..sid {
        let t = i as f64 / denom;
        let fx = wid * (t - 0.5);
        let fy = per * (1.0 - (t - 0.5).abs() * 2.0);
        let fxx = (wid + cor) * (t - 0.5);
        if i > 0 && !ptlist.is_empty() {
            if let Some(last) = ptlist[ptlist.len() - 1].last().copied() {
                ptlist.push(vec![last, (fxx, fy)]);
            }
        }
        ptlist.push(vec![(0.0, -hei), (fx * 0.5, (-hei + fy) * 0.5), (fxx, fy)]);
        polist.push((fxx, fy));
    }

    let fill = ctx.palette.fill;
    let mut shapes: Vec<Shape> = vec![filled(
        polist.iter().map(|p| (p.0 + xoff, p.1 + yoff)).collect(),
        fill,
    )];
    let ink = ctx.palette.ink_at(0.4, depth);
    for line in &ptlist {
        let points = div(line, 5.0);
        let shifted: Vec<Point> = points.iter().map(|p| (p.0 + xoff, p.1 + yoff)).collect();
        shapes.extend(stroke(
            ctx,
            &shifted,
            0.0,
            0.0,
            wei,
            Some(ink),
            1.0,
            1.0,
            Some(&|_x: f64| 1.0),
        ));
    }
    shapes
}

#[allow(clippy::too_many_arguments)]
pub fn arch01(
    ctx: &mut Ctx,
    xoff: f64,
    yoff: f64,
    seed: f64,
    hei: f64,
    wid: f64,
    _rot: f64,
    per: f64,
    depth: f64,
) -> Vec<Shape> {
    let p = 0.4 + ctx.rng.random() * 0.2;
    let h0 = hei * p;
    let h1 = hei * (1.0 - p);
    let mut shapes: Vec<Shape> = Vec::new();
    shapes.extend(hut(ctx, xoff, yoff - hei, h0, wid, 300, depth));
    shapes.extend(box_(
        ctx,
        xoff,
        yoff,
        h1,
        (wid * 2.0) / 3.0,
        0.7,
        per,
        true,
        false,
        3.0,
        None,
        depth,
    ));
    let seg1 = (3.0 + ctx.rng.random() * 3.0) as i32;
    shapes.extend(rail(
        ctx,
        xoff,
        yoff,
        seed,
        10.0,
        wid,
        0.7,
        per * 2.0,
        seg1,
        1.0,
        true,
        false,
        depth,
    ));

    let people = ctx.people;
    let mut mcnt = 0;
    if ctx.rng.random() < people {
        mcnt = *rand_choice(ctx, &[0, 1, 1, 2]);
    }
    if mcnt == 1 {
        let x = xoff + norm_rand(ctx, -wid / 3.0, wid / 3.0);
        let fli = *rand_choice(ctx, &[true, false]);
        shapes.extend(man(ctx, x, yoff, 0.42, None, None, fli, None, None));
    } else if mcnt == 2 {
        let x0 = xoff + norm_rand(ctx, -wid / 4.0, -wid / 5.0);
        shapes.extend(man(ctx, x0, yoff, 0.42, None, None, false, None, None));
        let x1 = xoff + norm_rand(ctx, wid / 5.0, wid / 4.0);
        shapes.extend(man(ctx, x1, yoff, 0.42, None, None, true, None, None));
    }
    let seg2 = (3.0 + ctx.rng.random() * 3.0) as i32;
    shapes.extend(rail(
        ctx,
        xoff,
        yoff,
        seed,
        10.0,
        wid,
        0.7,
        per * 2.0,
        seg2,
        1.0,
        false,
        true,
        depth,
    ));
    shapes
}

#[allow(clippy::too_many_arguments)]
pub fn arch02(
    ctx: &mut Ctx,
    xoff: f64,
    yoff: f64,
    _seed: f64,
    hei: f64,
    wid: f64,
    rot: f64,
    per: f64,
    sto: i32,
    sty: i32,
    rai: bool,
    plaque: Option<String>,
    depth: f64,
) -> Vec<Shape> {
    let mut shapes: Vec<Shape> = Vec::new();
    let mut hoff = 0.0;
    for i in 0..sto {
        let w = wid * 0.85f64.powi(i);
        let dec = |a: &DecoAnchors| {
            let (hsp, vsp) = match sty {
                1 | 2 => ((1.0, 5.0), (1.0, 2.0)),
                3 => ((1.0, 4.0), (1.0, 3.0)),
                _ => ((1.0, 5.0), (1.0, 2.0)),
            };
            deco(sty, a.pul, a.pur, a.pdl, a.pdr, hsp, vsp)
        };
        shapes.extend(box_(
            ctx,
            xoff,
            yoff - hoff,
            hei,
            w,
            rot,
            per,
            false,
            true,
            1.5,
            Some(&dec),
            depth,
        ));
        if rai {
            shapes.extend(rail(
                ctx,
                xoff,
                yoff - hoff,
                i as f64 * 0.2,
                hei / 2.0,
                w * 1.1,
                rot,
                per,
                4,
                0.5,
                false,
                true,
                depth,
            ));
        }
        let mut pla: Option<(i32, String)> = None;
        if sto == 1
            && plaque.as_ref().is_some_and(|p| !p.is_empty())
            && ctx.rng.random() < 1.0 / 3.0
        {
            pla = Some((1, plaque.clone().unwrap_or_default()));
        }
        shapes.extend(roof(
            ctx,
            xoff,
            yoff - hoff - hei,
            hei,
            wid * 0.9f64.powi(i),
            rot,
            per,
            5.0,
            1.5,
            pla,
            depth,
        ));
        hoff += hei * 1.5;
    }
    shapes
}

#[allow(clippy::too_many_arguments)]
pub fn arch03(
    ctx: &mut Ctx,
    xoff: f64,
    yoff: f64,
    _seed: f64,
    hei: f64,
    wid: f64,
    rot: f64,
    per: f64,
    sto: i32,
    depth: f64,
) -> Vec<Shape> {
    let mut shapes: Vec<Shape> = Vec::new();
    let mut hoff = 0.0;
    for i in 0..sto {
        let w = wid * 0.85f64.powi(i);
        let dec = |a: &DecoAnchors| deco(1, a.pul, a.pur, a.pdl, a.pdr, (1.0, 4.0), (1.0, 2.0));
        shapes.extend(box_(
            ctx,
            xoff,
            yoff - hoff,
            hei,
            w,
            rot,
            per / 2.0,
            false,
            true,
            1.5,
            Some(&dec),
            depth,
        ));
        shapes.extend(rail(
            ctx,
            xoff,
            yoff - hoff,
            i as f64 * 0.2,
            hei / 2.0,
            w * 1.1,
            rot,
            per / 2.0,
            5,
            0.5,
            false,
            true,
            depth,
        ));
        shapes.extend(pagroof(
            ctx,
            xoff,
            yoff - hoff - hei,
            hei * 1.5,
            wid * 0.9f64.powi(i),
            rot,
            per,
            10.0,
            4,
            1.5,
            depth,
        ));
        hoff += hei * 1.5;
    }
    shapes
}

#[allow(clippy::too_many_arguments)]
pub fn arch04(
    ctx: &mut Ctx,
    xoff: f64,
    yoff: f64,
    _seed: f64,
    hei: f64,
    wid: f64,
    rot: f64,
    per: f64,
    sto: i32,
    depth: f64,
) -> Vec<Shape> {
    let mut shapes: Vec<Shape> = Vec::new();
    let mut hoff = 0.0;
    for i in 0..sto {
        let w = wid * 0.85f64.powi(i);
        shapes.extend(box_(
            ctx,
            xoff,
            yoff - hoff,
            hei,
            w,
            rot,
            per / 2.0,
            true,
            true,
            1.5,
            None,
            depth,
        ));
        shapes.extend(rail(
            ctx,
            xoff,
            yoff - hoff,
            i as f64 * 0.2,
            hei / 3.0,
            w * 1.2,
            rot,
            per / 2.0,
            3,
            0.5,
            true,
            true,
            depth,
        ));
        shapes.extend(pagroof(
            ctx,
            xoff,
            yoff - hoff - hei,
            hei,
            wid * 0.9f64.powi(i),
            rot,
            per,
            10.0,
            4,
            1.5,
            depth,
        ));
        hoff += hei * 1.2;
    }
    shapes
}

#[allow(clippy::too_many_arguments)]
pub fn boat01(
    ctx: &mut Ctx,
    xoff: f64,
    yoff: f64,
    _seed: f64,
    len_: f64,
    sca: f64,
    fli: bool,
    depth: f64,
) -> Vec<Shape> {
    let dir = if fli { -1.0 } else { 1.0 };
    let mut shapes: Vec<Shape> = Vec::new();
    if ctx.people > 0.0 {
        let limb_len = [0.0, 30.0, 20.0, 30.0, 10.0, 30.0, 30.0, 30.0, 30.0];
        shapes.extend(man(
            ctx,
            xoff + 20.0 * sca * dir,
            yoff,
            0.5 * sca,
            Some(hat02),
            Some(stick01),
            !fli,
            None,
            Some(&limb_len),
        ));
    }

    let fun1 = |x: f64| (x * PI).sin().max(0.0).powf(0.5) * 7.0 * sca;
    let fun2 = |x: f64| (x * PI).sin().max(0.0).powf(0.5) * 10.0 * sca;

    let mut plist1: Vec<Point> = Vec::new();
    let mut plist2: Vec<Point> = Vec::new();
    let mut i = 0.0;
    let limit = len_ * sca;
    let step = 5.0 * sca;
    if step > 0.0 {
        while i < limit {
            plist1.push((i * dir, fun1(i / len_)));
            plist2.push((i * dir, fun2(i / len_)));
            i += step;
        }
    }
    let mut plist = plist1;
    plist.extend(plist2.iter().rev().copied());

    let fill = ctx.palette.fill;
    shapes.push(filled(
        plist.iter().map(|p| (p.0 + xoff, p.1 + yoff)).collect(),
        fill,
    ));
    let ink = ctx.palette.ink_at(0.4, depth);
    let shifted: Vec<Point> = plist.iter().map(|p| (p.0 + xoff, p.1 + yoff)).collect();
    shapes.extend(stroke(
        ctx,
        &shifted,
        0.0,
        0.0,
        1.0,
        Some(ink),
        0.5,
        1.0,
        Some(&|x: f64| (x * PI * 2.0).sin()),
    ));
    shapes
}

#[allow(clippy::too_many_arguments)]
pub fn transmission_tower01(
    ctx: &mut Ctx,
    xoff: f64,
    yoff: f64,
    _seed: f64,
    hei: f64,
    wid: f64,
    depth: f64,
) -> Vec<Shape> {
    let ink = ctx.palette.ink_at(0.4, depth);
    let quickstroke = |ctx: &mut Ctx, pl: &[Point]| -> Vec<Shape> {
        let points = div(pl, 5.0);
        let shifted: Vec<Point> = points.iter().map(|p| (p.0 + xoff, p.1 + yoff)).collect();
        stroke(
            ctx,
            &shifted,
            0.0,
            0.0,
            1.0,
            Some(ink),
            0.5,
            1.0,
            Some(&|_x: f64| 0.5),
        )
    };

    let mut shapes: Vec<Shape> = Vec::new();
    let p00 = (-wid * 0.05, -hei);
    let p01 = (wid * 0.05, -hei);
    let p10 = (-wid * 0.1, -hei * 0.9);
    let p11 = (wid * 0.1, -hei * 0.9);
    let p20 = (-wid * 0.2, -hei * 0.5);
    let p21 = (wid * 0.2, -hei * 0.5);
    let p30 = (-wid * 0.5, 0.0);
    let p31 = (wid * 0.5, 0.0);
    let bch = [(0.7, -0.85), (1.0, -0.675), (0.7, -0.5)];

    for (bx, by) in bch {
        shapes.extend(quickstroke(
            ctx,
            &[(-bx * wid, by * hei), (bx * wid, by * hei)],
        ));
        shapes.extend(quickstroke(
            ctx,
            &[(-bx * wid, by * hei), (0.0, (by - 0.05) * hei)],
        ));
        shapes.extend(quickstroke(
            ctx,
            &[(bx * wid, by * hei), (0.0, (by - 0.05) * hei)],
        ));
        shapes.extend(quickstroke(
            ctx,
            &[(-bx * wid, by * hei), (-bx * wid, (by + 0.1) * hei)],
        ));
        shapes.extend(quickstroke(
            ctx,
            &[(bx * wid, by * hei), (bx * wid, (by + 0.1) * hei)],
        ));
    }

    let l10 = div(&[p00, p10, p20, p30], 5.0);
    let l11 = div(&[p01, p11, p21, p31], 5.0);
    for i in 0..l10.len().saturating_sub(1) {
        shapes.extend(quickstroke(ctx, &[l10[i], l11[i + 1]]));
        shapes.extend(quickstroke(ctx, &[l11[i], l10[i + 1]]));
    }
    shapes.extend(quickstroke(ctx, &[p00, p01]));
    shapes.extend(quickstroke(ctx, &[p10, p11]));
    shapes.extend(quickstroke(ctx, &[p20, p21]));
    shapes.extend(quickstroke(ctx, &[p00, p10, p20, p30]));
    shapes.extend(quickstroke(ctx, &[p01, p11, p21, p31]));
    shapes
}
