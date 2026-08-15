use crate::desmos;
use crate::geometry::{simplify_points, smooth_points};
use crate::model::{DrawStroke, Point};
use crate::primitives::{CubicBezier, GeneratedPrimitive, GeneratedScene};
use crate::settings::{GenerationSettings, OutputMode, QualityPreset};

pub(crate) struct GenerationResult {
    pub(crate) scene: GeneratedScene,
    pub(crate) export_text: String,
    pub(crate) status: String,
}

pub(crate) fn generate(strokes: &[DrawStroke], settings: GenerationSettings) -> GenerationResult {
    if strokes.is_empty() {
        return GenerationResult {
            scene: GeneratedScene::default(),
            export_text: String::new(),
            status: "Nothing to generate yet.".to_owned(),
        };
    }

    if settings.output_mode == OutputMode::FunctionFit {
        return generate_function_fit(strokes, settings);
    }

    let mode = resolve_mode(settings);
    let mut scene = GeneratedScene::default();
    let mut skipped = 0;

    for stroke in strokes {
        let points = preprocess_points(&stroke.points, settings);
        if points.len() < 2 {
            skipped += 1;
            continue;
        }

        match mode {
            OutputMode::Lines => push_line_primitives(&points, &mut scene),
            OutputMode::Bezier => push_bezier_primitives(&points, &mut scene),
            OutputMode::Mixed => push_mixed_primitives(&points, &mut scene),
            OutputMode::Auto | OutputMode::FunctionFit => unreachable!(),
        }
    }

    let export_text = match mode {
        OutputMode::Lines => desmos::export_lines(&scene.primitives),
        OutputMode::Bezier => desmos::export_beziers(&scene.primitives),
        OutputMode::Mixed => desmos::export_mixed(&scene.primitives),
        OutputMode::Auto | OutputMode::FunctionFit => unreachable!(),
    };

    let primitive_count = scene.primitives.len();
    let status = if primitive_count == 0 {
        "Generation failed: strokes are too short after preprocessing.".to_owned()
    } else {
        let skipped = if skipped > 0 {
            format!(" Skipped {skipped} tiny stroke(s).")
        } else {
            String::new()
        };
        format!(
            "Generated {primitive_count} primitive(s): {} line(s), {} Bezier curve(s).{skipped}",
            scene.line_count(),
            scene.bezier_count()
        )
    };

    GenerationResult {
        scene,
        export_text,
        status,
    }
}

fn resolve_mode(settings: GenerationSettings) -> OutputMode {
    match settings.output_mode {
        OutputMode::Auto => match settings.quality {
            QualityPreset::Rough | QualityPreset::Precise => OutputMode::Lines,
            QualityPreset::Default | QualityPreset::Smooth => OutputMode::Bezier,
        },
        mode => mode,
    }
}

fn preprocess_points(points: &[Point], settings: GenerationSettings) -> Vec<Point> {
    let mut points = points.to_vec();
    for _ in 0..settings.quality.smoothing_passes() {
        points = smooth_points(&points);
    }
    simplify_points(&points, settings.effective_tolerance())
}

fn push_line_primitives(points: &[Point], scene: &mut GeneratedScene) {
    for pair in points.windows(2) {
        let from = pair[0];
        let to = pair[1];
        if from.distance(to) >= f32::EPSILON {
            scene.primitives.push(GeneratedPrimitive::Line { from, to });
        }
    }
    scene.preview_paths.push(points.to_vec());
}

fn push_bezier_primitives(points: &[Point], scene: &mut GeneratedScene) {
    if points.len() < 2 {
        return;
    }

    let mut preview = Vec::new();
    for index in 0..points.len() - 1 {
        let bezier = catmull_rom_segment(points, index);
        if bezier.p0.distance(bezier.p3) < f32::EPSILON {
            continue;
        }

        sample_bezier_into(bezier, &mut preview);
        scene
            .primitives
            .push(GeneratedPrimitive::CubicBezier(bezier));
    }

    if !preview.is_empty() {
        scene.preview_paths.push(preview);
    }
}

fn push_mixed_primitives(points: &[Point], scene: &mut GeneratedScene) {
    if points.len() < 3 {
        push_line_primitives(points, scene);
        return;
    }

    let mut preview = Vec::new();
    for index in 0..points.len() - 1 {
        let from = points[index];
        let to = points[index + 1];
        let before = if index == 0 { from } else { points[index - 1] };
        let after = if index + 2 >= points.len() {
            to
        } else {
            points[index + 2]
        };

        if is_corner_like(before, from, to, after) {
            scene.primitives.push(GeneratedPrimitive::Line { from, to });
            if preview
                .last()
                .is_none_or(|last: &Point| last.distance(from) > 0.001)
            {
                preview.push(from);
            }
            preview.push(to);
        } else {
            let bezier = catmull_rom_segment(points, index);
            sample_bezier_into(bezier, &mut preview);
            scene
                .primitives
                .push(GeneratedPrimitive::CubicBezier(bezier));
        }
    }

    if !preview.is_empty() {
        scene.preview_paths.push(preview);
    }
}

fn catmull_rom_segment(points: &[Point], index: usize) -> CubicBezier {
    let p0 = points[index];
    let p3 = points[index + 1];
    let previous = if index == 0 { p0 } else { points[index - 1] };
    let next = if index + 2 >= points.len() {
        p3
    } else {
        points[index + 2]
    };

    CubicBezier {
        p0,
        p1: Point {
            x: p0.x + (p3.x - previous.x) / 6.0,
            y: p0.y + (p3.y - previous.y) / 6.0,
        },
        p2: Point {
            x: p3.x - (next.x - p0.x) / 6.0,
            y: p3.y - (next.y - p0.y) / 6.0,
        },
        p3,
    }
}

fn sample_bezier_into(bezier: CubicBezier, output: &mut Vec<Point>) {
    let steps = 12;
    if output
        .last()
        .is_none_or(|last| last.distance(bezier.p0) > 0.001)
    {
        output.push(bezier.p0);
    }

    for step in 1..=steps {
        output.push(bezier.sample(step as f32 / steps as f32));
    }
}

fn is_corner_like(before: Point, from: Point, to: Point, after: Point) -> bool {
    let incoming = Point {
        x: from.x - before.x,
        y: from.y - before.y,
    };
    let outgoing = Point {
        x: after.x - to.x,
        y: after.y - to.y,
    };
    let incoming_len = (incoming.x * incoming.x + incoming.y * incoming.y).sqrt();
    let outgoing_len = (outgoing.x * outgoing.x + outgoing.y * outgoing.y).sqrt();
    if incoming_len < 0.001 || outgoing_len < 0.001 {
        return false;
    }

    let dot = (incoming.x * outgoing.x + incoming.y * outgoing.y) / (incoming_len * outgoing_len);
    dot < 0.35
}

fn generate_function_fit(strokes: &[DrawStroke], settings: GenerationSettings) -> GenerationResult {
    let mut lines = Vec::new();
    let mut scene = GeneratedScene::default();
    let mut fitted = 0;
    let mut skipped = 0;

    for stroke in strokes {
        let mut points = preprocess_points(&stroke.points, settings);
        points.sort_by(|a, b| a.x.total_cmp(&b.x));
        points.dedup_by(|a, b| (a.x - b.x).abs() < 0.001);

        if points.len() < 2 {
            skipped += 1;
            continue;
        }

        let degree = settings.polynomial_degree.min(points.len() - 1);
        let Some(coefficients) = fit_polynomial(&points, degree) else {
            skipped += 1;
            continue;
        };

        lines.push(desmos::export_polynomial(&coefficients));
        scene.preview_paths.push(sample_polynomial_path(
            &coefficients,
            points.first().unwrap().x,
            points.last().unwrap().x,
        ));
        fitted += 1;
    }

    let export_text = lines.join("\n");
    let status = if fitted == 0 {
        "Function fit failed: strokes are not suitable for y=f(x).".to_owned()
    } else {
        let skipped = if skipped > 0 {
            format!(" Skipped {skipped} stroke(s).")
        } else {
            String::new()
        };
        format!(
            "Generated {fitted} experimental polynomial function(s). This mode works best for single-valued y=f(x) strokes.{skipped}"
        )
    };

    GenerationResult {
        scene,
        export_text,
        status,
    }
}

fn sample_polynomial_path(coefficients: &[f32], min_x: f32, max_x: f32) -> Vec<Point> {
    let mut points = Vec::new();
    let steps = 96;
    let width = max_x - min_x;
    if width.abs() < f32::EPSILON {
        return points;
    }

    for step in 0..=steps {
        let x = min_x + width * step as f32 / steps as f32;
        points.push(Point {
            x,
            y: eval_polynomial(coefficients, x),
        });
    }
    points
}

fn eval_polynomial(coefficients: &[f32], x: f32) -> f32 {
    coefficients
        .iter()
        .enumerate()
        .map(|(power, coefficient)| coefficient * x.powi(power as i32))
        .sum()
}

fn fit_polynomial(points: &[Point], degree: usize) -> Option<Vec<f32>> {
    let n = degree + 1;
    let mut matrix = vec![vec![0.0; n + 1]; n];

    for row in 0..n {
        for col in 0..n {
            matrix[row][col] = points
                .iter()
                .map(|point| point.x.powi((row + col) as i32))
                .sum();
        }
        matrix[row][n] = points
            .iter()
            .map(|point| point.y * point.x.powi(row as i32))
            .sum();
    }

    solve_linear_system(matrix)
}

fn solve_linear_system(mut matrix: Vec<Vec<f32>>) -> Option<Vec<f32>> {
    let n = matrix.len();

    for pivot in 0..n {
        let best_row = (pivot..n)
            .max_by(|a, b| matrix[*a][pivot].abs().total_cmp(&matrix[*b][pivot].abs()))?;

        if matrix[best_row][pivot].abs() < 0.000_001 {
            return None;
        }

        matrix.swap(pivot, best_row);

        let pivot_value = matrix[pivot][pivot];
        for col in pivot..=n {
            matrix[pivot][col] /= pivot_value;
        }

        for row in 0..n {
            if row == pivot {
                continue;
            }
            let factor = matrix[row][pivot];
            for col in pivot..=n {
                matrix[row][col] -= factor * matrix[pivot][col];
            }
        }
    }

    Some(matrix.into_iter().map(|row| row[n]).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn polynomial_fit_recovers_line() {
        let points = vec![
            Point { x: -1.0, y: -1.0 },
            Point { x: 0.0, y: 1.0 },
            Point { x: 1.0, y: 3.0 },
        ];

        let coefficients = fit_polynomial(&points, 1).unwrap();

        assert!((coefficients[0] - 1.0).abs() < 0.001);
        assert!((coefficients[1] - 2.0).abs() < 0.001);
    }

    #[test]
    fn catmull_rom_bezier_starts_and_ends_on_points() {
        let points = vec![
            Point { x: 0.0, y: 0.0 },
            Point { x: 1.0, y: 1.0 },
            Point { x: 2.0, y: 0.0 },
        ];

        let bezier = catmull_rom_segment(&points, 0);

        assert_eq!(bezier.p0.x, 0.0);
        assert_eq!(bezier.p0.y, 0.0);
        assert_eq!(bezier.p3.x, 1.0);
        assert_eq!(bezier.p3.y, 1.0);
    }
}
