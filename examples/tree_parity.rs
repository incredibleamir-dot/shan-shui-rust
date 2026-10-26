use shan_shui::color::get_palette;
use shan_shui::context::Ctx;
use shan_shui::noise::Noise;
use shan_shui::rng::Rng;
use shan_shui::trees::{tree04, tree05, tree06, tree08};

fn ctx_for(seed: i64) -> Ctx {
    Ctx::new(
        Rng::from_i64(seed).child("t"),
        Noise::new(Rng::from_i64(seed).child("noise")),
        get_palette("ink").unwrap().into(),
    )
}

fn main() {
    for seed in 0..20 {
        for (name, shapes) in [
            (
                "tree05",
                tree05(&mut ctx_for(seed), 0.0, 0.0, 200.0, 5.0, 0.7),
            ),
            (
                "tree04",
                tree04(&mut ctx_for(seed), 0.0, 0.0, 300.0, 6.0, 0.7),
            ),
            (
                "tree06",
                tree06(&mut ctx_for(seed), 0.0, 0.0, 100.0, 6.0, 0.7),
            ),
            (
                "tree08",
                tree08(&mut ctx_for(seed), 0.0, 0.0, 80.0, 1.0, 0.7),
            ),
        ] {
            let first = &shapes[0];
            let n = match first {
                shan_shui::Shape::Poly(poly) => poly.pts.len(),
                _ => 0,
            };
            println!("{} {} {} {}", seed, name, shapes.len(), n);
        }
    }
}
