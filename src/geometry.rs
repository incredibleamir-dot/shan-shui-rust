use crate::shapes::Point;

/// Average of a list of points, preserving Python's accumulation order.
pub fn mid_pt(plist: &[Point]) -> Point {
    let n = plist.len() as f64;
    let mut sx = 0.0;
    let mut sy = 0.0;
    for p in plist {
        sx += p.0 / n;
        sy += p.1 / n;
    }
    (sx, sy)
}

pub fn mid_pt2(a: Point, b: Point) -> Point {
    ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0)
}

fn line_expr(p0: Point, p1: Point) -> (f64, f64) {
    let den = p1.0 - p0.0;
    let m = if den == 0.0 {
        f64::INFINITY
    } else {
        (p1.1 - p0.1) / den
    };
    let k = p0.1 - m * p0.0;
    (m, k)
}

fn on_seg(p: Point, ln: (Point, Point)) -> bool {
    ln.0 .0.min(ln.1 .0) <= p.0
        && p.0 <= ln.0 .0.max(ln.1 .0)
        && ln.0 .1.min(ln.1 .1) <= p.1
        && p.1 <= ln.0 .1.max(ln.1 .1)
}

fn intersect(ln0: (Point, Point), ln1: (Point, Point)) -> Option<Point> {
    let le0 = line_expr(ln0.0, ln0.1);
    let le1 = line_expr(ln1.0, ln1.1);

    if le0.0.is_infinite() && le1.0.is_infinite() {
        return None;
    }
    let (x, y) = if le0.0.is_infinite() {
        let x = ln0.0 .0;
        (x, le1.0 * x + le1.1)
    } else if le1.0.is_infinite() {
        let x = ln1.0 .0;
        (x, le0.0 * x + le0.1)
    } else {
        let den = le0.0 - le1.0;
        if den == 0.0 {
            return None;
        }
        let x = (le1.1 - le0.1) / den;
        (x, le0.0 * x + le0.1)
    };

    if !(x.is_finite() && y.is_finite()) {
        return None;
    }
    let p = (x, y);
    if on_seg(p, ln0) && on_seg(p, ln1) {
        Some(p)
    } else {
        None
    }
}

fn pt_in_poly(pt: Point, plist: &[Point]) -> bool {
    let mut scount = 0i32;
    for i in 0..plist.len() {
        let np = if i != plist.len() - 1 {
            plist[i + 1]
        } else {
            plist[0]
        };
        if intersect((plist[i], np), (pt, (pt.0 + 999.0, pt.1 + 999.0))).is_some() {
            scount += 1;
        }
    }
    scount % 2 == 1
}

fn ln_in_poly(ln: (Point, Point), plist: &[Point]) -> bool {
    let ep = 0.01;
    let lnc = (
        (
            ln.0 .0 * (1.0 - ep) + ln.1 .0 * ep,
            ln.0 .1 * (1.0 - ep) + ln.1 .1 * ep,
        ),
        (
            ln.0 .0 * ep + ln.1 .0 * (1.0 - ep),
            ln.0 .1 * ep + ln.1 .1 * (1.0 - ep),
        ),
    );
    for i in 0..plist.len() {
        let pt = plist[i];
        let np = if i != plist.len() - 1 {
            plist[i + 1]
        } else {
            plist[0]
        };
        if intersect(lnc, (pt, np)).is_some() {
            return false;
        }
    }
    pt_in_poly(mid_pt2(ln.0, ln.1), plist)
}

fn sides_of(plist: &[Point]) -> Vec<f64> {
    let mut slist = Vec::with_capacity(plist.len());
    for i in 0..plist.len() {
        let pt = plist[i];
        let np = if i != plist.len() - 1 {
            plist[i + 1]
        } else {
            plist[0]
        };
        slist.push((np.0 - pt.0).hypot(np.1 - pt.1));
    }
    slist
}

fn area_of(plist: &[Point]) -> f64 {
    let slist = sides_of(plist);
    if slist.len() < 3 {
        return 0.0;
    }
    let (a, b, c) = (slist[0], slist[1], slist[2]);
    let s = (a + b + c) / 2.0;
    let value = s * (s - a) * (s - b) * (s - c);
    if value > 0.0 {
        value.sqrt()
    } else {
        0.0
    }
}

fn sliver_ratio(plist: &[Point]) -> f64 {
    let area = area_of(plist);
    let perim: f64 = sides_of(plist).iter().sum();
    if perim != 0.0 {
        area / perim
    } else {
        0.0
    }
}

fn best_ear(plist: &[Point], convex: bool, optimize: bool) -> (Vec<Point>, Vec<Point>) {
    let mut cuts: Vec<(Vec<Point>, Vec<Point>)> = Vec::new();
    for i in 0..plist.len() {
        let pt = plist[i];
        let lp = if i != 0 {
            plist[i - 1]
        } else {
            plist[plist.len() - 1]
        };
        let np = if i != plist.len() - 1 {
            plist[i + 1]
        } else {
            plist[0]
        };
        if convex || ln_in_poly((lp, np), plist) {
            let mut qlist = Vec::with_capacity(plist.len() - 1);
            qlist.extend_from_slice(&plist[..i]);
            qlist.extend_from_slice(&plist[i + 1..]);
            let cut = (vec![lp, pt, np], qlist);
            if !optimize {
                return cut;
            }
            cuts.push(cut);
        }
    }

    if cuts.is_empty() {
        return (plist.to_vec(), Vec::new());
    }

    let mut best = cuts[0].clone();
    let mut best_ratio = -1.0;
    for cut in &cuts {
        let ratio = sliver_ratio(&cut.0);
        if ratio >= best_ratio {
            best = cut.clone();
            best_ratio = ratio;
        }
    }
    best
}

fn shatter(plist: &[Point], area: f64, depth: u32) -> Vec<Vec<Point>> {
    if plist.is_empty() {
        return Vec::new();
    }
    if plist.len() < 3 || area_of(plist) < area || depth > 24 {
        return vec![plist.to_vec()];
    }

    let slist = sides_of(plist);
    let mut ind = 0usize;
    for i in 1..slist.len() {
        if slist[i] > slist[ind] {
            ind = i;
        }
    }
    let nind = (ind + 1) % plist.len();
    let lind = (ind + 2) % plist.len();
    let mid = mid_pt2(plist[ind], plist[nind]);
    let mut out = shatter(&[plist[ind], mid, plist[lind]], area, depth + 1);
    out.extend(shatter(&[plist[lind], plist[nind], mid], area, depth + 1));
    out
}

pub fn triangulate(plist: &[Point], area: f64, convex: bool, optimize: bool) -> Vec<Vec<Point>> {
    triangulate_depth(plist, area, convex, optimize, 0)
}

fn triangulate_depth(
    plist: &[Point],
    area: f64,
    convex: bool,
    optimize: bool,
    _depth: u32,
) -> Vec<Vec<Point>> {
    if plist.is_empty() {
        return Vec::new();
    }
    if plist.len() <= 3 {
        return shatter(plist, area, 0);
    }

    let cut = best_ear(plist, convex, optimize);
    if cut.1.is_empty() {
        if cut.0.len() == plist.len() {
            return vec![cut.0];
        }
        return shatter(&cut.0, area, 0);
    }

    let mut out = shatter(&cut.0, area, 0);
    out.extend(triangulate_depth(
        &cut.1,
        area,
        convex,
        optimize,
        _depth + 1,
    ));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn triangle_shatters_into_pieces() {
        let tris = triangulate(
            &[(0.0, 0.0), (100.0, 0.0), (50.0, 100.0)],
            100.0,
            true,
            false,
        );
        assert!(!tris.is_empty());
    }

    #[test]
    fn point_in_poly_works() {
        let square = [(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)];
        // Keep the test ray away from a corner: the parity test used by the
        // generator counts both edges at an exact vertex crossing.
        assert!(pt_in_poly((5.0, 7.0), &square));
        assert!(!pt_in_poly((15.0, 7.0), &square));
    }
}
