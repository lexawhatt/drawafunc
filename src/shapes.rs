use eframe::egui::Color32;

use crate::model::{DrawStroke, Point};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ShapeTool {
    Line,
    Rectangle,
    Circle,
    Heart,
    Star,
}

pub(crate) fn make_shape_stroke(
    shape: ShapeTool,
    start: Point,
    end: Point,
    color: Color32,
    width: f32,
) -> Option<DrawStroke> {
    let points = shape_points(shape, start, end)?;
    let mut stroke = DrawStroke::new(color, width);
    stroke.points = points;
    Some(stroke)
}

pub(crate) fn shape_points(shape: ShapeTool, start: Point, end: Point) -> Option<Vec<Point>> {
    if start.distance(end) < 0.01 {
        return None;
    }

    let points = match shape {
        ShapeTool::Line => vec![start, end],
        ShapeTool::Rectangle => rectangle_points(start, end),
        ShapeTool::Circle => ellipse_points(start, end, 96),
        ShapeTool::Heart => heart_points(start, end, 120),
        ShapeTool::Star => star_points(start, end),
    };

    Some(points)
}

fn rectangle_points(start: Point, end: Point) -> Vec<Point> {
    vec![
        start,
        Point {
            x: end.x,
            y: start.y,
        },
        end,
        Point {
            x: start.x,
            y: end.y,
        },
        start,
    ]
}

fn ellipse_points(start: Point, end: Point, steps: usize) -> Vec<Point> {
    let center = midpoint(start, end);
    let radius_x = (end.x - start.x).abs() * 0.5;
    let radius_y = (end.y - start.y).abs() * 0.5;
    let mut points = Vec::with_capacity(steps + 1);

    for step in 0..=steps {
        let angle = std::f32::consts::TAU * step as f32 / steps as f32;
        points.push(Point {
            x: center.x + radius_x * angle.cos(),
            y: center.y + radius_y * angle.sin(),
        });
    }

    points
}

fn heart_points(start: Point, end: Point, steps: usize) -> Vec<Point> {
    let center = midpoint(start, end);
    let scale_x = (end.x - start.x).abs() / 34.0;
    let scale_y = (end.y - start.y).abs() / 32.0;
    let mut points = Vec::with_capacity(steps + 1);

    for step in 0..=steps {
        let t = std::f32::consts::TAU * step as f32 / steps as f32;
        let raw_x = 16.0 * t.sin().powi(3);
        let raw_y =
            13.0 * t.cos() - 5.0 * (2.0 * t).cos() - 2.0 * (3.0 * t).cos() - (4.0 * t).cos();
        points.push(Point {
            x: center.x + raw_x * scale_x,
            y: center.y + raw_y * scale_y,
        });
    }

    points
}

fn star_points(start: Point, end: Point) -> Vec<Point> {
    let center = midpoint(start, end);
    let outer = (end.x - start.x).abs().min((end.y - start.y).abs()) * 0.5;
    let inner = outer * 0.42;
    let mut points = Vec::with_capacity(11);

    for index in 0..10 {
        let radius = if index % 2 == 0 { outer } else { inner };
        let angle = -std::f32::consts::FRAC_PI_2 + std::f32::consts::TAU * index as f32 / 10.0;
        points.push(Point {
            x: center.x + radius * angle.cos(),
            y: center.y + radius * angle.sin(),
        });
    }
    points.push(points[0]);
    points
}

fn midpoint(a: Point, b: Point) -> Point {
    Point {
        x: (a.x + b.x) * 0.5,
        y: (a.y + b.y) * 0.5,
    }
}
