use std::sync::Arc;

use crate::rng::Rng;

/// Port of the p5.js Perlin-style noise used by the original project.
#[derive(Clone, Debug)]
pub struct Noise {
    table: Arc<Vec<f64>>,
    octaves: u32,
    amp_falloff: f64,
}

impl Noise {
    pub const PERLIN_YWRAPB: i64 = 4;
    pub const PERLIN_YWRAP: i64 = 1 << Self::PERLIN_YWRAPB;
    pub const PERLIN_ZWRAPB: i64 = 8;
    pub const PERLIN_ZWRAP: i64 = 1 << Self::PERLIN_ZWRAPB;
    pub const PERLIN_SIZE: i64 = 4095;

    pub fn new(rng: Rng) -> Self {
        let mut rng = rng;
        let mut table = Vec::with_capacity((Self::PERLIN_SIZE + 1) as usize);
        for _ in 0..=Self::PERLIN_SIZE {
            table.push(rng.random());
        }
        Self {
            table: Arc::new(table),
            octaves: 4,
            amp_falloff: 0.5,
        }
    }

    pub fn noise(&self, x: f64, y: f64, z: f64) -> f64 {
        let perlin = &self.table;
        let x = x.abs();
        let y = y.abs();
        let z = z.abs();

        let mut xi = x.floor() as i64;
        let mut yi = y.floor() as i64;
        let mut zi = z.floor() as i64;
        let mut xf = x - xi as f64;
        let mut yf = y - yi as f64;
        let mut zf = z - zi as f64;

        let pi = std::f64::consts::PI;
        let size = Self::PERLIN_SIZE;
        let ywrap = Self::PERLIN_YWRAP;
        let zwrap = Self::PERLIN_ZWRAP;
        let ywrapb = Self::PERLIN_YWRAPB;
        let zwrapb = Self::PERLIN_ZWRAPB;

        let mut r = 0.0;
        let mut ampl = 0.5;
        for _ in 0..self.octaves {
            let mut of = xi + (yi << ywrapb) + (zi << zwrapb);
            let rxf = 0.5 * (1.0 - (xf * pi).cos());
            let ryf = 0.5 * (1.0 - (yf * pi).cos());

            let idx = (of & size) as usize;
            let mut n1 = perlin[idx];
            n1 += rxf * (perlin[((of + 1) & size) as usize] - n1);
            let mut n2 = perlin[((of + ywrap) & size) as usize];
            n2 += rxf * (perlin[((of + ywrap + 1) & size) as usize] - n2);
            n1 += ryf * (n2 - n1);

            of += zwrap;
            n2 = perlin[(of & size) as usize];
            n2 += rxf * (perlin[((of + 1) & size) as usize] - n2);
            let mut n3 = perlin[((of + ywrap) & size) as usize];
            n3 += rxf * (perlin[((of + ywrap + 1) & size) as usize] - n3);
            n2 += ryf * (n3 - n2);

            n1 += (0.5 * (1.0 - (zf * pi).cos())) * (n2 - n1);
            r += n1 * ampl;
            ampl *= self.amp_falloff;

            xi <<= 1;
            xf *= 2.0;
            yi <<= 1;
            yf *= 2.0;
            zi <<= 1;
            zf *= 2.0;

            if xf >= 1.0 {
                xi += 1;
                xf -= 1.0;
            }
            if yf >= 1.0 {
                yi += 1;
                yf -= 1.0;
            }
            if zf >= 1.0 {
                zi += 1;
                zf -= 1.0;
            }
        }

        r
    }

    pub fn noise2(&self, x: f64, y: f64) -> f64 {
        self.noise(x, y, 0.0)
    }

    pub fn noise1(&self, x: f64) -> f64 {
        self.noise(x, 0.0, 0.0)
    }

    pub fn noise_detail(&mut self, lod: f64, falloff: f64) {
        if lod > 0.0 {
            self.octaves = lod as u32;
        }
        if falloff > 0.0 {
            self.amp_falloff = falloff;
        }
    }

    pub fn re_seed(&mut self, rng: Rng) {
        *self = Noise::new(rng);
    }
}
