use crate::color::Color;

/// A point in scene coordinates.
pub type Point = (f64, f64);

#[derive(Clone, Debug, PartialEq)]
pub struct Poly {
    pub pts: Vec<Point>,
    pub fill: Option<Color>,
    pub stroke: Option<Color>,
    pub width: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Circle {
    pub cx: f64,
    pub cy: f64,
    pub r: f64,
    pub fill: Option<Color>,
    pub stroke: Option<Color>,
    pub width: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GradientRect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub top: Color,
    pub bottom: Color,
    pub vertical: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Text {
    pub x: f64,
    pub y: f64,
    pub text: String,
    pub size: f64,
    pub color: Color,
    pub angle: f64,
    pub family: String,
    pub anchor: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Seal {
    pub x: f64,
    pub y: f64,
    pub size: f64,
    pub text: String,
    pub color: Color,
    pub text_color: Color,
}

impl Poly {
    pub fn new(pts: Vec<Point>, fill: Option<Color>) -> Self {
        Self {
            pts,
            fill,
            stroke: None,
            width: 0.0,
        }
    }

    pub fn stroked(pts: Vec<Point>, fill: Option<Color>, stroke: Color, width: f64) -> Self {
        Self {
            pts,
            fill,
            stroke: Some(stroke),
            width,
        }
    }
}

impl Circle {
    pub fn filled(cx: f64, cy: f64, r: f64, fill: Color) -> Self {
        Self {
            cx,
            cy,
            r,
            fill: Some(fill),
            stroke: None,
            width: 0.0,
        }
    }
}

impl Text {
    pub fn new(x: f64, y: f64, text: impl Into<String>, size: f64, color: Color) -> Self {
        Self {
            x,
            y,
            text: text.into(),
            size,
            color,
            angle: 0.0,
            family: "serif".into(),
            anchor: "middle".into(),
        }
    }
}

impl Seal {
    pub fn new(x: f64, y: f64, size: f64, text: impl Into<String>, color: Color) -> Self {
        Self {
            x,
            y,
            size,
            text: text.into(),
            color,
            text_color: Color::new(250, 245, 235, 1.0),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Shape {
    Poly(Poly),
    Circle(Circle),
    GradientRect(GradientRect),
    Text(Text),
    Seal(Seal),
}

impl From<Poly> for Shape {
    fn from(value: Poly) -> Self {
        Shape::Poly(value)
    }
}

impl From<Circle> for Shape {
    fn from(value: Circle) -> Self {
        Shape::Circle(value)
    }
}

impl From<GradientRect> for Shape {
    fn from(value: GradientRect) -> Self {
        Shape::GradientRect(value)
    }
}

impl From<Text> for Shape {
    fn from(value: Text) -> Self {
        Shape::Text(value)
    }
}

impl From<Seal> for Shape {
    fn from(value: Seal) -> Self {
        Shape::Seal(value)
    }
}
