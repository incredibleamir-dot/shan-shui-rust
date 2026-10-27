//! Procedural Chinese landscape paintings.
//!
//! This crate is a faithful Rust port of the original {Shan, Shui}*
//! browser generator. The generator is deterministic: the same seed and
//! options produce the same shapes.

pub mod architecture;
pub mod color;
pub mod context;
pub mod finish;
pub mod geometry;
pub mod mountains;
pub mod noise;
pub mod people;
pub mod planner;
pub mod primitives;
pub mod render;
pub mod rng;
pub mod scene;
pub mod shapes;
pub mod trees;
pub mod water;

pub use color::{get_palette, theme_names, Color, Palette};
pub use context::Ctx;
pub use scene::{Scene, SceneOptions};
pub use shapes::{Circle, GradientRect, Poly, Seal, Shape, Text};

pub const VERSION: &str = "0.2.0";
