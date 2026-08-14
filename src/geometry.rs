use crate::model::Point;

pub(crate) fn smooth_points(points: &[Point]) -> Vec<Point> {
    if points.len() < 3 {
        return points.to_vec();
    }

    let mut smoothed = Vec::with_capacity(points.len());
    smoothed.push(points[0]);
    for index in 1..points.len() - 1 {
        let previous = points[index - 1];
        let current = points[index];
        let next = points[index + 1];
        smoothed.push(Point {
            x: previous.x * 0.2 + current.x * 0.6 + next.x * 0.2,
            y: previous.y * 0.2 + current.y * 0.6 + next.y * 0.2,
        });
    }
    smoothed.push(*points.last().unwrap());
    smoothed
}

pub(crate) fn simplify_points(points: &[Point], tolerance: f32) -> Vec<Point> {
    if points.len() <= 2 {
        return points.to_vec();
    }

    let mut keep = vec![false; points.len()];
    keep[0] = true;
    keep[points.len() - 1] = true;
    simplify_range(points, 0, points.len() - 1, tolerance, &mut keep);

    points
        .iter()
        .zip(keep)
        .filter_map(|(point, keep)| keep.then_some(*point))
        .collect()
}

fn simplify_range(points: &[Point], start: usize, end: usize, tolerance: f32, keep: &mut [bool]) {
    if end <= start + 1 {
        return;
    }

    let mut max_distance = 0.0;
    let mut max_index = start;
    for index in start + 1..end {
        let distance = distance_to_segment(points[index], points[start], points[end]);
        if distance > max_distance {
            max_distance = distance;
            max_index = index;
        }
    }

    if max_distance > tolerance {
        keep[max_index] = true;
        simplify_range(points, start, max_index, tolerance, keep);
        simplify_range(points, max_index, end, tolerance, keep);
    }
}

pub(crate) fn distance_to_polyline(point: Point, points: &[Point]) -> f32 {
    match points.len() {
        0 => f32::INFINITY,
        1 => point.distance(points[0]),
        _ => points
            .windows(2)
            .map(|pair| distance_to_segment(point, pair[0], pair[1]))
            .fold(f32::INFINITY, f32::min),
    }
}

fn distance_to_segment(point: Point, a: Point, b: Point) -> f32 {
    let ab_x = b.x - a.x;
    let ab_y = b.y - a.y;
    let ap_x = point.x - a.x;
    let ap_y = point.y - a.y;
    let ab_len_sq = ab_x * ab_x + ab_y * ab_y;

    if ab_len_sq <= f32::EPSILON {
        return point.distance(a);
    }

    let t = ((ap_x * ab_x + ap_y * ab_y) / ab_len_sq).clamp(0.0, 1.0);
    point.distance(a.lerp(b, t))
}

pub(crate) fn nice_grid_step(zoom: f32) -> f32 {
    let target_pixels = 64.0;
    let raw = target_pixels / zoom;
    let power = 10.0_f32.powf(raw.log10().floor());
    let normalized = raw / power;

    let nice = if normalized < 1.5 {
        1.0
    } else if normalized < 3.5 {
        2.0
    } else if normalized < 7.5 {
        5.0
    } else {
        10.0
    };

    nice * power
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simplify_keeps_line_endpoints() {
        let points = vec![
            Point { x: 0.0, y: 0.0 },
            Point { x: 1.0, y: 0.01 },
            Point { x: 2.0, y: 0.0 },
        ];

        let simplified = simplify_points(&points, 0.05);

        assert_eq!(simplified.len(), 2);
        assert_eq!(simplified[0].x, 0.0);
        assert_eq!(simplified[1].x, 2.0);
    }

    #[test]
    fn simplify_preserves_visible_corner() {
        let points = vec![
            Point { x: 0.0, y: 0.0 },
            Point { x: 1.0, y: 1.0 },
            Point { x: 2.0, y: 0.0 },
        ];

        let simplified = simplify_points(&points, 0.05);

        assert_eq!(simplified.len(), 3);
    }
}
