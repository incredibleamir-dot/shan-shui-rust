use std::sync::{Arc, OnceLock};

use crate::rng::Rng;

fn clamp(value: f64, lo: f64, hi: f64) -> f64 {
    if value < lo {
        lo
    } else if value > hi {
        hi
    } else {
        value
    }
}

/// Round half to even, matching the reference renderer.
#[inline]
pub fn round_half_even(value: f64) -> f64 {
    value.round_ties_even()
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: f64,
}

impl Color {
    pub fn new(r: i32, g: i32, b: i32, a: f64) -> Self {
        Self {
            r: clamp(r as f64, 0.0, 255.0) as u8,
            g: clamp(g as f64, 0.0, 255.0) as u8,
            b: clamp(b as f64, 0.0, 255.0) as u8,
            a: clamp(a, 0.0, 1.0),
        }
    }

    pub fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 1.0 }
    }

    pub fn hex(value: &str, a: f64) -> Self {
        let v = value.trim_start_matches('#');
        let parse = |i: usize| u8::from_str_radix(&v[i..i + 2], 16).unwrap_or(0) as i32;
        Self::new(parse(0), parse(2), parse(4), a)
    }

    pub fn with_alpha(self, alpha: f64) -> Self {
        Self::new(self.r as i32, self.g as i32, self.b as i32, alpha)
    }

    pub fn mix(self, other: Self, t: f64) -> Self {
        let t = clamp(t, 0.0, 1.0);
        Self::new(
            round_half_even(self.r as f64 + (other.r as f64 - self.r as f64) * t) as i32,
            round_half_even(self.g as f64 + (other.g as f64 - self.g as f64) * t) as i32,
            round_half_even(self.b as f64 + (other.b as f64 - self.b as f64) * t) as i32,
            self.a + (other.a - self.a) * t,
        )
    }

    pub fn scale(self, factor: f64) -> Self {
        Self::new(
            round_half_even(self.r as f64 * factor) as i32,
            round_half_even(self.g as f64 * factor) as i32,
            round_half_even(self.b as f64 * factor) as i32,
            self.a,
        )
    }

    pub fn lighten(self, t: f64) -> Self {
        self.mix(Color::new(255, 255, 255, self.a), t)
    }

    pub fn darken(self, t: f64) -> Self {
        self.mix(Color::new(0, 0, 0, self.a), t)
    }

    pub fn css(self) -> String {
        format!("rgba({},{},{},{:.3})", self.r, self.g, self.b, self.a)
    }

    pub fn rgb_tuple(self) -> (u8, u8, u8) {
        (self.r, self.g, self.b)
    }

    pub fn rgba(self) -> (u8, u8, u8, u8) {
        (
            self.r,
            self.g,
            self.b,
            round_half_even(self.a * 255.0) as u8,
        )
    }

    /// Blend toward another color in linear-ish RGB space (used by the UI).
    pub fn blend(self, other: Self, t: f64) -> Self {
        self.mix(other, t)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Palette {
    pub name: String,
    pub paper: Color,
    pub ink: Color,
    pub haze: Color,
    pub rock: Color,
    pub fill: Color,
    pub wash: Color,
    pub foliage: Vec<Color>,
    pub trunk: Color,
    pub roof: Color,
    pub wall: Color,
    pub water: Color,
    pub sky_top: Color,
    pub sky_bottom: Color,
    pub mist: Color,
    pub accent: Color,
    pub sun: Color,
    pub distant: Color,
}

impl Palette {
    fn depth(depth: f64) -> f64 {
        clamp(depth, 0.0, 1.0)
    }

    pub fn shade(&self, base: Color, depth: f64, amount: f64) -> Color {
        base.mix(self.haze, (1.0 - Self::depth(depth)) * amount)
    }

    pub fn ink_at(&self, alpha: f64, depth: f64) -> Color {
        self.shade(self.ink, depth, 0.7).with_alpha(alpha)
    }

    pub fn rock_at(&self, alpha: f64, depth: f64) -> Color {
        self.shade(self.rock, depth, 0.62).with_alpha(alpha)
    }

    pub fn wash_at(&self, alpha: f64, depth: f64) -> Color {
        self.shade(self.wash, depth, 0.5).with_alpha(alpha)
    }

    pub fn foliage_at(&self, rng: &mut Rng, depth: f64, alpha: f64) -> Color {
        let base = self.foliage[(self.foliage.len() as f64 * rng.random()) as usize];
        let jitter = 0.9 + 0.2 * rng.random();
        self.shade(base.scale(jitter), depth, 0.62)
            .with_alpha(alpha)
    }

    pub fn trunk_at(&self, alpha: f64, depth: f64) -> Color {
        self.shade(self.trunk, depth, 0.62).with_alpha(alpha)
    }

    pub fn roof_at(&self, alpha: f64, depth: f64) -> Color {
        self.shade(self.roof, depth, 0.62).with_alpha(alpha)
    }

    pub fn wall_at(&self, alpha: f64, depth: f64) -> Color {
        self.shade(self.wall, depth, 0.35).with_alpha(alpha)
    }

    pub fn water_at(&self, alpha: f64, depth: f64) -> Color {
        self.shade(self.water, depth, 0.45).with_alpha(alpha)
    }

    pub fn mist_at(&self, alpha: f64) -> Color {
        self.mist.with_alpha(alpha)
    }

    pub fn accent_at(&self, alpha: f64, depth: f64) -> Color {
        self.shade(self.accent, depth, 0.25).with_alpha(alpha)
    }

    pub fn sun_at(&self, alpha: f64) -> Color {
        self.sun.with_alpha(alpha)
    }

    pub fn distant_at(&self, t: f64, _depth: f64) -> Color {
        self.distant
            .mix(self.fill, clamp(t, 0.0, 1.0) * 0.55 + 0.15)
            .with_alpha(1.0)
    }

    pub fn tone(&self, t: f64, depth: f64, alpha: f64) -> Color {
        let t = clamp(t, 0.0, 1.0);
        if self.name == "ink" {
            let c = (50.0 + 200.0 * t) as i32;
            return Color::new(c, c, c, alpha);
        }
        let base = self.foliage[0].mix(*self.foliage.last().unwrap(), t);
        self.shade(base, depth, 0.62).with_alpha(alpha)
    }
}

fn p(
    name: &str,
    paper: (i32, i32, i32),
    ink: (i32, i32, i32),
    haze: (i32, i32, i32),
    rock: (i32, i32, i32),
    fill: (i32, i32, i32),
    wash: (i32, i32, i32),
    foliage: &[(i32, i32, i32)],
    trunk: (i32, i32, i32),
    roof: (i32, i32, i32),
    wall: (i32, i32, i32),
    water: (i32, i32, i32),
    sky_top: (i32, i32, i32),
    sky_bottom: (i32, i32, i32),
    mist: (i32, i32, i32),
    accent: (i32, i32, i32),
    sun: (i32, i32, i32),
    distant: (i32, i32, i32),
) -> Palette {
    Palette {
        name: name.into(),
        paper: Color::new(paper.0, paper.1, paper.2, 1.0),
        ink: Color::new(ink.0, ink.1, ink.2, 1.0),
        haze: Color::new(haze.0, haze.1, haze.2, 1.0),
        rock: Color::new(rock.0, rock.1, rock.2, 1.0),
        fill: Color::new(fill.0, fill.1, fill.2, 1.0),
        wash: Color::new(wash.0, wash.1, wash.2, 1.0),
        foliage: foliage
            .iter()
            .map(|c| Color::new(c.0, c.1, c.2, 1.0))
            .collect(),
        trunk: Color::new(trunk.0, trunk.1, trunk.2, 1.0),
        roof: Color::new(roof.0, roof.1, roof.2, 1.0),
        wall: Color::new(wall.0, wall.1, wall.2, 1.0),
        water: Color::new(water.0, water.1, water.2, 1.0),
        sky_top: Color::new(sky_top.0, sky_top.1, sky_top.2, 1.0),
        sky_bottom: Color::new(sky_bottom.0, sky_bottom.1, sky_bottom.2, 1.0),
        mist: Color::new(mist.0, mist.1, mist.2, 1.0),
        accent: Color::new(accent.0, accent.1, accent.2, 1.0),
        sun: Color::new(sun.0, sun.1, sun.2, 1.0),
        distant: Color::new(distant.0, distant.1, distant.2, 1.0),
    }
}

fn themes() -> &'static Vec<Palette> {
    static THEMES: OnceLock<Vec<Palette>> = OnceLock::new();
    THEMES.get_or_init(|| {
        vec![
            p(
                "ink",
                (247, 244, 236),
                (100, 100, 100),
                (247, 244, 236),
                (120, 120, 120),
                (255, 255, 255),
                (100, 100, 100),
                &[
                    (100, 100, 100),
                    (120, 120, 120),
                    (140, 140, 140),
                    (90, 90, 90),
                ],
                (100, 100, 100),
                (100, 100, 100),
                (255, 255, 255),
                (110, 110, 110),
                (255, 255, 255),
                (247, 244, 236),
                (255, 255, 255),
                (170, 56, 46),
                (220, 205, 170),
                (185, 185, 185),
            ),
            p(
                "qinglv",
                (243, 235, 216),
                (38, 43, 40),
                (201, 214, 210),
                (94, 122, 110),
                (248, 243, 231),
                (86, 116, 104),
                &[(63, 107, 79), (92, 138, 99), (47, 90, 70), (127, 160, 107)],
                (91, 70, 54),
                (49, 64, 65),
                (238, 230, 212),
                (127, 169, 171),
                (220, 235, 231),
                (243, 235, 216),
                (234, 239, 231),
                (178, 58, 47),
                (232, 201, 138),
                (159, 181, 172),
            ),
            p(
                "jinbi",
                (245, 235, 208),
                (46, 42, 34),
                (216, 201, 168),
                (166, 124, 61),
                (251, 244, 223),
                (150, 111, 55),
                &[(76, 107, 69), (109, 138, 78), (54, 91, 58), (138, 158, 90)],
                (107, 79, 47),
                (58, 58, 54),
                (243, 231, 201),
                (127, 163, 163),
                (234, 221, 187),
                (247, 239, 217),
                (240, 230, 202),
                (166, 50, 38),
                (212, 166, 60),
                (185, 168, 127),
            ),
            p(
                "autumn",
                (247, 238, 220),
                (51, 40, 33),
                (220, 196, 168),
                (138, 94, 66),
                (251, 243, 228),
                (128, 85, 58),
                &[(180, 85, 45), (201, 130, 46), (142, 59, 42), (214, 163, 58)],
                (90, 59, 40),
                (62, 58, 54),
                (241, 227, 204),
                (143, 165, 160),
                (242, 217, 184),
                (250, 241, 223),
                (242, 229, 208),
                (165, 42, 42),
                (224, 163, 74),
                (196, 169, 139),
            ),
            p(
                "snow",
                (238, 242, 245),
                (44, 54, 64),
                (200, 212, 220),
                (107, 126, 140),
                (247, 249, 250),
                (98, 117, 132),
                &[
                    (76, 102, 104),
                    (107, 133, 131),
                    (61, 86, 89),
                    (140, 160, 154),
                ],
                (74, 64, 56),
                (57, 67, 76),
                (232, 238, 241),
                (168, 192, 203),
                (215, 227, 234),
                (240, 244, 246),
                (232, 238, 242),
                (163, 58, 50),
                (232, 226, 210),
                (169, 186, 197),
            ),
            p(
                "dusk",
                (247, 232, 212),
                (58, 46, 53),
                (216, 185, 168),
                (122, 93, 90),
                (251, 240, 225),
                (114, 80, 82),
                &[(95, 75, 87), (122, 94, 94), (75, 60, 74), (147, 112, 94)],
                (75, 56, 47),
                (64, 54, 61),
                (240, 221, 200),
                (176, 143, 154),
                (242, 180, 138),
                (239, 217, 200),
                (236, 214, 194),
                (180, 70, 47),
                (232, 137, 74),
                (201, 162, 143),
            ),
            p(
                "pastel",
                (250, 246, 238),
                (74, 74, 85),
                (220, 216, 224),
                (140, 154, 154),
                (252, 249, 243),
                (130, 146, 146),
                &[
                    (143, 176, 138),
                    (168, 192, 154),
                    (122, 158, 140),
                    (194, 201, 143),
                ],
                (122, 102, 86),
                (90, 100, 112),
                (244, 238, 228),
                (169, 198, 204),
                (228, 239, 240),
                (250, 244, 236),
                (242, 239, 231),
                (196, 106, 90),
                (242, 216, 166),
                (201, 207, 210),
            ),
            // --- Colours added for the Rust studio -------------------------
            p(
                "jade",
                (240, 244, 235),
                (28, 48, 44),
                (188, 210, 200),
                (72, 112, 98),
                (250, 250, 242),
                (64, 112, 94),
                &[(46, 112, 84), (84, 148, 104), (30, 82, 64), (132, 176, 112)],
                (76, 56, 42),
                (34, 54, 52),
                (238, 240, 226),
                (104, 166, 164),
                (198, 228, 216),
                (240, 246, 236),
                (224, 238, 228),
                (196, 62, 48),
                (240, 214, 142),
                (152, 186, 170),
            ),
            p(
                "nocturne",
                (24, 34, 52),
                (208, 220, 236),
                (52, 68, 96),
                (70, 86, 112),
                (30, 42, 62),
                (88, 106, 138),
                &[(46, 70, 88), (66, 94, 108), (34, 54, 74), (98, 126, 126)],
                (46, 38, 40),
                (28, 34, 46),
                (58, 66, 84),
                (64, 92, 118),
                (20, 28, 48),
                (52, 66, 92),
                (46, 58, 82),
                (192, 70, 64),
                (236, 206, 160),
                (76, 92, 116),
            ),
            p(
                "sepia",
                (240, 228, 204),
                (58, 44, 30),
                (212, 190, 158),
                (148, 112, 72),
                (250, 240, 220),
                (142, 104, 64),
                &[(120, 86, 48), (154, 114, 62), (96, 70, 42), (182, 142, 82)],
                (86, 60, 36),
                (64, 50, 38),
                (238, 222, 192),
                (150, 160, 148),
                (230, 206, 166),
                (246, 234, 210),
                (236, 218, 188),
                (152, 60, 42),
                (214, 168, 96),
                (190, 162, 122),
            ),
            p(
                "porcelain",
                (244, 246, 248),
                (44, 58, 78),
                (208, 220, 230),
                (100, 126, 150),
                (252, 252, 252),
                (84, 116, 148),
                &[
                    (70, 106, 120),
                    (96, 138, 148),
                    (54, 84, 104),
                    (132, 166, 156),
                ],
                (74, 66, 58),
                (44, 62, 82),
                (240, 244, 246),
                (128, 170, 192),
                (214, 230, 240),
                (248, 250, 250),
                (236, 242, 246),
                (176, 54, 48),
                (238, 216, 168),
                (164, 188, 204),
            ),
            p(
                "blossom",
                (250, 242, 240),
                (70, 52, 58),
                (226, 206, 210),
                (168, 118, 122),
                (253, 246, 242),
                (172, 112, 116),
                &[
                    (196, 112, 124),
                    (224, 146, 148),
                    (158, 92, 108),
                    (238, 178, 162),
                ],
                (98, 66, 58),
                (84, 62, 68),
                (248, 238, 232),
                (152, 180, 196),
                (238, 216, 222),
                (252, 242, 238),
                (244, 228, 226),
                (184, 62, 70),
                (244, 196, 150),
                (206, 178, 186),
            ),
        ]
    })
}

pub fn theme_names() -> Vec<&'static str> {
    themes().iter().map(|p| p.name.as_str()).collect()
}

pub fn get_palette(name: &str) -> Result<Palette, String> {
    themes()
        .iter()
        .find(|p| p.name == name)
        .cloned()
        .ok_or_else(|| {
            format!(
                "unknown theme '{}' (available: {})",
                name,
                theme_names().join(", ")
            )
        })
}

/// Colour swatches for UI theme cards: sky, paper, foliage, accent, ink.
pub fn theme_swatch(name: &str) -> Option<[Color; 4]> {
    themes()
        .iter()
        .find(|p| p.name == name)
        .map(|p| [p.sky_top, p.paper, p.foliage[0], p.accent])
}

pub fn palette_arc(name: &str) -> Result<Arc<Palette>, String> {
    get_palette(name).map(Arc::new)
}
