//! Raster, PNG, SVG and text rendering.
//!
//! Implementation lives here so the generator can be checked and tested
//! without pulling in the GUI feature.

pub mod raster;
pub mod svg;
pub mod text;

pub use raster::{
    render_scene, render_shapes, render_shapes_when, save_png, Bitmap, RenderOptions,
};
pub use svg::{save_svg, save_svg_sized, to_svg, to_svg_sized};
