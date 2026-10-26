//! Post-processing presets shared by the raster renderer and the studio UI.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Finish {
    Wash,
    Crisp,
    Vivid,
    Vintage,
    Noir,
}

impl Finish {
    pub const ALL: [Finish; 5] = [
        Finish::Wash,
        Finish::Crisp,
        Finish::Vivid,
        Finish::Vintage,
        Finish::Noir,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Finish::Wash => "Wash",
            Finish::Crisp => "Crisp",
            Finish::Vivid => "Vivid",
            Finish::Vintage => "Vintage",
            Finish::Noir => "Noir",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Finish::Wash => "wash",
            Finish::Crisp => "crisp",
            Finish::Vivid => "vivid",
            Finish::Vintage => "vintage",
            Finish::Noir => "noir",
        }
    }

    pub fn from_key(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|f| f.key() == value)
    }
}

impl Default for Finish {
    fn default() -> Self {
        Finish::Wash
    }
}
