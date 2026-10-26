use std::sync::Arc;

use crate::color::Palette;
use crate::noise::Noise;
use crate::rng::Rng;

#[derive(Clone)]
pub struct Ctx {
    pub rng: Rng,
    pub noise: Noise,
    pub palette: Arc<Palette>,
    pub vegetation: bool,
    pub architecture: f64,
    pub people: f64,
    pub boats: f64,
    pub dust: f64,
}

impl Ctx {
    pub fn new(rng: Rng, noise: Noise, palette: Arc<Palette>) -> Self {
        Self {
            rng,
            noise,
            palette,
            vegetation: true,
            architecture: 1.0,
            people: 1.0,
            boats: 1.0,
            dust: 1.0,
        }
    }

    pub fn child(&self, label: &str) -> Ctx {
        Self {
            rng: self.rng.child(label),
            noise: self.noise.clone(),
            palette: self.palette.clone(),
            vegetation: self.vegetation,
            architecture: self.architecture,
            people: self.people,
            boats: self.boats,
            dust: self.dust,
        }
    }
}
