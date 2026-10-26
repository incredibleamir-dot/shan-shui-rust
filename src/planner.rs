use std::collections::HashMap;

use crate::context::Ctx;
use crate::primitives::rand_choice;

#[derive(Clone, Debug, PartialEq)]
pub struct PlanItem {
    pub tag: &'static str,
    pub x: f64,
    pub y: f64,
    pub h: f64,
}

pub fn plan_range(xmin: f64, xmax: f64, ctx: &mut Ctx) -> Vec<PlanItem> {
    let mut reg: Vec<PlanItem> = Vec::new();
    let samp = 0.03;

    let ns = |ctx: &mut Ctx, x: f64, _y: f64| -> f64 {
        (ctx.noise.noise(x * samp, 0.0, 0.0) - 0.55).max(0.0) * 2.0
    };
    let yr =
        |ctx: &mut Ctx, x: f64| -> f64 { ctx.noise.noise(x * 0.01, std::f64::consts::PI, 0.0) };

    let locmax = |ctx: &mut Ctx, x: f64, y: f64, r: i32| -> bool {
        let z0 = ns(ctx, x, y);
        if z0 <= 0.3 {
            return false;
        }
        for i in (x - r as f64) as i64..(x + r as f64) as i64 {
            for j in (y - r as f64) as i64..(y + r as f64) as i64 {
                if ns(ctx, i as f64, j as f64) > z0 {
                    return false;
                }
            }
        }
        true
    };

    let chadd = |reg: &mut Vec<PlanItem>, item: PlanItem, mind: f64| -> bool {
        for other in reg.iter() {
            if (other.x - item.x).abs() < mind {
                return false;
            }
        }
        reg.push(item);
        true
    };

    let xstep = 5i64;
    let mwid = 200.0;
    let mut planmtx: HashMap<i64, i32> = HashMap::new();
    let mut i = xmin as i64;
    while i < xmax as i64 {
        planmtx.insert((i as f64 / xstep as f64).floor() as i64, 0);
        i += xstep;
    }

    i = xmin as i64;
    while i < xmax as i64 {
        let i_f = i as f64;
        let ymax = (yr(ctx, i_f) * 480.0) as i32;
        let mut j = 0i32;
        while j < ymax {
            if locmax(ctx, i_f, j as f64, 2) {
                let xof = i_f + 2.0 * (ctx.rng.random() - 0.5) * 500.0;
                let yof = j as f64 + 300.0;
                let item = PlanItem {
                    tag: "mount",
                    x: xof,
                    y: yof,
                    h: ns(ctx, i_f, j as f64),
                };
                if chadd(&mut reg, item, 10.0) {
                    let mut k = ((xof - mwid) / xstep as f64).floor() as i64;
                    let kend = ((xof + mwid) / xstep as f64).ceil() as i64;
                    while k < kend {
                        *planmtx.entry(k).or_insert(0) += 1;
                        k += 1;
                    }
                }
            }
            j += 30;
        }
        if (i.abs() % 1000) < (xstep - 1).max(1) {
            let item = PlanItem {
                tag: "distmount",
                x: i_f,
                y: 280.0 - ctx.rng.random() * 50.0,
                h: ns(ctx, i_f, 0.0),
            };
            chadd(&mut reg, item, 10.0);
        }
        i += xstep;
    }

    i = xmin as i64;
    while i < xmax as i64 {
        if planmtx
            .get(&((i as f64 / xstep as f64).floor() as i64))
            .copied()
            .unwrap_or(0)
            == 0
            && ctx.rng.random() < 0.01
        {
            let count = (4.0 * ctx.rng.random()).ceil().max(0.0) as i32;
            for j in 0..count {
                let item = PlanItem {
                    tag: "flatmount",
                    x: i as f64 + 2.0 * (ctx.rng.random() - 0.5) * 700.0,
                    y: 700.0 - j as f64 * 50.0,
                    h: ns(ctx, i as f64, j as f64),
                };
                chadd(&mut reg, item, 10.0);
            }
        }
        i += xstep;
    }

    i = xmin as i64;
    while i < xmax as i64 {
        if ctx.rng.random() < 0.2 {
            let item = PlanItem {
                tag: "boat",
                x: i as f64,
                y: 300.0 + ctx.rng.random() * 390.0,
                h: 0.0,
            };
            chadd(&mut reg, item, 400.0);
        }
        i += xstep;
    }

    reg
}

pub fn _unused_choice_helper(ctx: &mut Ctx) -> i32 {
    *rand_choice(ctx, &[1, 2])
}
