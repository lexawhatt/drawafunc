use eframe::egui::Color32;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub(crate) struct Point {
    pub(crate) x: f32,
    pub(crate) y: f32,
}

impl Point {
    pub(crate) fn distance(self, other: Self) -> f32 {
        ((self.x - other.x).powi(2) + (self.y - other.y).powi(2)).sqrt()
    }

    pub(crate) fn lerp(self, other: Self, t: f32) -> Self {
        Self {
            x: self.x + (other.x - self.x) * t,
            y: self.y + (other.y - self.y) * t,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct DrawStroke {
    pub(crate) points: Vec<Point>,
    pub(crate) color: [u8; 4],
    pub(crate) width: f32,
}

impl DrawStroke {
    pub(crate) fn new(color: Color32, width: f32) -> Self {
        Self {
            points: Vec::new(),
            color: [color.r(), color.g(), color.b(), color.a()],
            width,
        }
    }

    pub(crate) fn color32(&self) -> Color32 {
        Color32::from_rgba_unmultiplied(self.color[0], self.color[1], self.color[2], self.color[3])
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Project {
    pub(crate) version: u32,
    pub(crate) strokes: Vec<DrawStroke>,
}

impl Project {
    pub(crate) fn from_strokes(strokes: &[DrawStroke]) -> Self {
        Self {
            version: 1,
            strokes: strokes.to_vec(),
        }
    }
}
