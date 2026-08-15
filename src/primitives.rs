use crate::model::Point;

#[derive(Clone, Copy, Debug)]
pub(crate) struct CubicBezier {
    pub(crate) p0: Point,
    pub(crate) p1: Point,
    pub(crate) p2: Point,
    pub(crate) p3: Point,
}

impl CubicBezier {
    pub(crate) fn sample(self, t: f32) -> Point {
        let u = 1.0 - t;
        let uu = u * u;
        let tt = t * t;
        let uuu = uu * u;
        let ttt = tt * t;

        Point {
            x: uuu * self.p0.x
                + 3.0 * uu * t * self.p1.x
                + 3.0 * u * tt * self.p2.x
                + ttt * self.p3.x,
            y: uuu * self.p0.y
                + 3.0 * uu * t * self.p1.y
                + 3.0 * u * tt * self.p2.y
                + ttt * self.p3.y,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum GeneratedPrimitive {
    Line { from: Point, to: Point },
    CubicBezier(CubicBezier),
}

#[derive(Clone, Debug, Default)]
pub(crate) struct GeneratedScene {
    pub(crate) primitives: Vec<GeneratedPrimitive>,
    pub(crate) preview_paths: Vec<Vec<Point>>,
}

impl GeneratedScene {
    pub(crate) fn line_count(&self) -> usize {
        self.primitives
            .iter()
            .filter(|primitive| matches!(primitive, GeneratedPrimitive::Line { .. }))
            .count()
    }

    pub(crate) fn bezier_count(&self) -> usize {
        self.primitives
            .iter()
            .filter(|primitive| matches!(primitive, GeneratedPrimitive::CubicBezier(_)))
            .count()
    }
}
