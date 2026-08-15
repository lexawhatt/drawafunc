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

    if settings.output_mode == OutputMode::ExperimentalPolynomial {
        return generate_polynomial_function_fit(strokes, settings);
    }
    if settings.output_mode == OutputMode::ExperimentalExponential {
        return generate_exponential_function_fit(strokes, settings);
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
            OutputMode::Auto
            | OutputMode::ExperimentalPolynomial
            | OutputMode::ExperimentalExponential => unreachable!(),
        }
    }

    let export_text = match mode {
        OutputMode::Lines => desmos::export_lines(&scene.primitives),
        OutputMode::Bezier => desmos::export_beziers(&scene.primitives),
        OutputMode::Mixed => desmos::export_mixed(&scene.primitives),
        OutputMode::Auto
        | OutputMode::ExperimentalPolynomial
        | OutputMode::ExperimentalExponential => unreachable!(),
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

fn generate_polynomial_function_fit(
    strokes: &[DrawStroke],
    settings: GenerationSettings,
) -> GenerationResult {
    let mut lines = Vec::new();
    let mut scene = GeneratedScene::default();
    let mut fitted = 0;
    let mut skipped = 0;
    let mut split_count = 0;
    let mut max_degree_used = 0;

    for stroke in strokes {
        let points = preprocess_points(&stroke.points, settings);
        let spans = split_x_monotone_spans(&points);

        if spans.is_empty() {
            skipped += 1;
            continue;
        }

        for span in spans {
            let mut fits = Vec::new();
            split_count += fit_span_recursive(&span, settings, 0, &mut fits);
            if fits.is_empty() {
                skipped += 1;
                continue;
            }

            for fit in fits {
                max_degree_used = max_degree_used.max(fit.degree);
                lines.push(desmos::export_polynomial(
                    &fit.coefficients,
                    Some((fit.min_x, fit.max_x)),
                ));
                scene.preview_paths.push(sample_polynomial_path(
                    &fit.coefficients,
                    fit.min_x,
                    fit.max_x,
                ));
                fitted += 1;
            }
        }
    }

    let export_text = lines.join("\n");
    let status = if fitted == 0 {
        "Experimental polynomial fit failed: strokes are not suitable for y=f(x).".to_owned()
    } else {
        let skipped = if skipped > 0 {
            format!(" Skipped {skipped} stroke(s).")
        } else {
            String::new()
        };
        format!(
            "Generated {fitted} experimental polynomial function(s). Max degree used: {max_degree_used}. X-turns and high-error spans are split into multiple y=f(x) pieces. Recursive splits: {split_count}.{skipped}"
        )
    };

    GenerationResult {
        scene,
        export_text,
        status,
    }
}

fn generate_exponential_function_fit(
    strokes: &[DrawStroke],
    settings: GenerationSettings,
) -> GenerationResult {
    let mut lines = Vec::new();
    let mut scene = GeneratedScene::default();
    let mut fitted = 0;
    let mut skipped = 0;
    let mut split_count = 0;
    let mut polynomial_fallbacks = 0;

    for stroke in strokes {
        let points = preprocess_points(&stroke.points, settings);
        let spans = split_x_monotone_spans(&points);

        if spans.is_empty() {
            skipped += 1;
            continue;
        }

        for span in spans {
            let mut exp_fits = Vec::new();
            split_count += fit_exponential_span_recursive(&span, settings, 0, &mut exp_fits);
            if exp_fits.is_empty() {
                let mut poly_fits = Vec::new();
                split_count += fit_span_recursive(&span, settings, 0, &mut poly_fits);
                if poly_fits.is_empty() {
                    skipped += 1;
                    continue;
                }

                for fit in poly_fits {
                    lines.push(desmos::export_polynomial(
                        &fit.coefficients,
                        Some((fit.min_x, fit.max_x)),
                    ));
                    scene.preview_paths.push(sample_polynomial_path(
                        &fit.coefficients,
                        fit.min_x,
                        fit.max_x,
                    ));
                    fitted += 1;
                    polynomial_fallbacks += 1;
                }
                continue;
            }

            for fit in exp_fits {
                lines.push(desmos::export_exponential(
                    fit.amplitude,
                    fit.rate,
                    fit.offset,
                    Some((fit.min_x, fit.max_x)),
                ));
                scene.preview_paths.push(sample_exponential_path(
                    fit.amplitude,
                    fit.rate,
                    fit.offset,
                    fit.min_x,
                    fit.max_x,
                ));
                fitted += 1;
            }
        }
    }

    let export_text = lines.join("\n");
    let status = if fitted == 0 {
        "Experimental exponential fit failed: strokes are not suitable for y=c+a*e^(b*x)."
            .to_owned()
    } else {
        let skipped = if skipped > 0 {
            format!(" Skipped {skipped} stroke(s).")
        } else {
            String::new()
        };
        format!(
            "Generated {fitted} experimental exponential-family function(s) of form y=c+a*e^(b*x), with polynomial fallback for unstable spans. Fallbacks: {polynomial_fallbacks}. Recursive splits: {split_count}.{skipped}"
        )
    };

    GenerationResult {
        scene,
        export_text,
        status,
    }
}

#[derive(Clone, Debug)]
struct PolynomialFit {
    coefficients: Vec<f32>,
    min_x: f32,
    max_x: f32,
    max_error: f32,
    degree: usize,
}

#[derive(Clone, Debug)]
struct ExponentialFit {
    amplitude: f32,
    rate: f32,
    offset: f32,
    min_x: f32,
    max_x: f32,
    max_error: f32,
}

impl ExponentialFit {
    fn is_stable(&self) -> bool {
        self.amplitude.is_finite()
            && self.rate.is_finite()
            && self.offset.is_finite()
            && self.max_error.is_finite()
            && self.amplitude.abs() >= 0.000_001
            && self.amplitude.abs() <= 1_000_000.0
            && self.rate.abs() <= 12.0
            && self.offset.abs() <= 1_000_000.0
            && (self.max_x - self.min_x).abs() >= 0.25
    }
}

fn fit_span_recursive(
    span: &[Point],
    settings: GenerationSettings,
    depth: usize,
    fits: &mut Vec<PolynomialFit>,
) -> usize {
    const MAX_DEPTH: usize = 8;
    const MIN_POINTS_TO_SPLIT: usize = 6;

    let Some(points) = prepare_function_points(span) else {
        return 0;
    };

    let Some(best_fit) = choose_polynomial_fit(&points, settings) else {
        return 0;
    };

    if best_fit.max_error <= settings.function_fit_error_tolerance()
        || depth >= MAX_DEPTH
        || points.len() < MIN_POINTS_TO_SPLIT
    {
        fits.push(best_fit);
        return 0;
    }

    let split_index = max_error_index(&points, &best_fit.coefficients);
    if split_index == 0 || split_index >= points.len() - 1 {
        fits.push(best_fit);
        return 0;
    }

    let left_splits = fit_span_recursive(&points[..=split_index], settings, depth + 1, fits);
    let right_splits = fit_span_recursive(&points[split_index..], settings, depth + 1, fits);
    left_splits + right_splits + 1
}

fn fit_exponential_span_recursive(
    span: &[Point],
    settings: GenerationSettings,
    depth: usize,
    fits: &mut Vec<ExponentialFit>,
) -> usize {
    const MAX_DEPTH: usize = 8;
    const MIN_POINTS_TO_SPLIT: usize = 6;
    const MIN_EXP_POINTS: usize = 4;
    const MIN_EXP_DOMAIN_WIDTH: f32 = 0.25;

    let Some(points) = prepare_function_points(span) else {
        return 0;
    };

    if points.len() < MIN_EXP_POINTS || domain_width(&points) < MIN_EXP_DOMAIN_WIDTH {
        return 0;
    }

    let Some(best_fit) = choose_exponential_fit(&points) else {
        return 0;
    };

    if !best_fit.is_stable() {
        return 0;
    }

    if best_fit.max_error <= settings.function_fit_error_tolerance() {
        fits.push(best_fit);
        return 0;
    }

    if depth >= MAX_DEPTH || points.len() < MIN_POINTS_TO_SPLIT {
        return 0;
    }

    let split_index = max_exponential_error_index(&points, &best_fit);
    if split_index == 0 || split_index >= points.len() - 1 {
        fits.push(best_fit);
        return 0;
    }

    let left_splits =
        fit_exponential_span_recursive(&points[..=split_index], settings, depth + 1, fits);
    let right_splits =
        fit_exponential_span_recursive(&points[split_index..], settings, depth + 1, fits);
    left_splits + right_splits + 1
}

fn prepare_function_points(span: &[Point]) -> Option<Vec<Point>> {
    let mut points = span.to_vec();
    points.sort_by(|a, b| a.x.total_cmp(&b.x));
    points.dedup_by(|a, b| (a.x - b.x).abs() < 0.001);

    if points.len() < 2 {
        return None;
    }

    let min_x = points.first().unwrap().x;
    let max_x = points.last().unwrap().x;
    if (max_x - min_x).abs() < 0.01 {
        return None;
    }

    Some(points)
}

fn choose_polynomial_fit(points: &[Point], settings: GenerationSettings) -> Option<PolynomialFit> {
    let max_degree = settings.function_fit_degree_cap().min(points.len() - 1);
    let mut best = None;

    for degree in 1..=max_degree {
        let coefficients = fit_polynomial(points, degree)?;
        let max_error = polynomial_max_error(points, &coefficients);
        let fit = PolynomialFit {
            coefficients,
            min_x: points.first().unwrap().x,
            max_x: points.last().unwrap().x,
            max_error,
            degree,
        };

        if fit.max_error <= settings.function_fit_error_tolerance() {
            return Some(fit);
        }

        if best
            .as_ref()
            .is_none_or(|best: &PolynomialFit| fit.max_error < best.max_error)
        {
            best = Some(fit);
        }
    }

    best
}

fn choose_exponential_fit(points: &[Point]) -> Option<ExponentialFit> {
    let min_x = points.first()?.x;
    let max_x = points.last()?.x;
    let min_y = points
        .iter()
        .map(|point| point.y)
        .fold(f32::INFINITY, f32::min);
    let max_y = points
        .iter()
        .map(|point| point.y)
        .fold(f32::NEG_INFINITY, f32::max);
    let range = (max_y - min_y).abs().max(0.5);
    let mut best = None;

    for offset in exponential_offset_candidates(min_y, max_y, range) {
        for sign in [1.0, -1.0] {
            let Some((log_amplitude, rate)) = fit_log_linear(points, offset, sign) else {
                continue;
            };
            if !log_amplitude.is_finite() || !rate.is_finite() {
                continue;
            }
            let amplitude = sign * log_amplitude.exp();
            let max_error = exponential_max_error(points, amplitude, rate, offset);
            let fit = ExponentialFit {
                amplitude,
                rate,
                offset,
                min_x,
                max_x,
                max_error,
            };
            if !fit.is_stable() {
                continue;
            }

            if best
                .as_ref()
                .is_none_or(|best: &ExponentialFit| fit.max_error < best.max_error)
            {
                best = Some(fit);
            }
        }
    }

    best
}

fn exponential_offset_candidates(min_y: f32, max_y: f32, range: f32) -> Vec<f32> {
    let mut candidates = Vec::new();
    for multiplier in [
        0.02, 0.05, 0.1, 0.18, 0.3, 0.42, 0.54, 0.7, 0.9, 1.2, 1.8, 2.6, 4.0,
    ] {
        candidates.push(min_y - range * multiplier);
        candidates.push(max_y + range * multiplier);
    }
    candidates
}

fn fit_log_linear(points: &[Point], offset: f32, sign: f32) -> Option<(f32, f32)> {
    let mut xs = Vec::with_capacity(points.len());
    let mut zs = Vec::with_capacity(points.len());

    for point in points {
        let shifted = sign * (point.y - offset);
        if shifted <= 0.000_001 {
            return None;
        }
        xs.push(point.x);
        zs.push(shifted.ln());
    }

    fit_line(&xs, &zs)
}

fn fit_line(xs: &[f32], ys: &[f32]) -> Option<(f32, f32)> {
    if xs.len() != ys.len() || xs.len() < 2 {
        return None;
    }

    let count = xs.len() as f32;
    let mean_x = xs.iter().sum::<f32>() / count;
    let mean_y = ys.iter().sum::<f32>() / count;
    let numerator: f32 = xs
        .iter()
        .zip(ys)
        .map(|(x, y)| (x - mean_x) * (y - mean_y))
        .sum();
    let denominator: f32 = xs.iter().map(|x| (x - mean_x).powi(2)).sum();

    if denominator.abs() < 0.000_001 {
        return None;
    }

    let slope = numerator / denominator;
    let intercept = mean_y - slope * mean_x;
    Some((intercept, slope))
}

fn polynomial_max_error(points: &[Point], coefficients: &[f32]) -> f32 {
    points
        .iter()
        .map(|point| (point.y - eval_polynomial(coefficients, point.x)).abs())
        .fold(0.0, f32::max)
}

fn domain_width(points: &[Point]) -> f32 {
    let Some(first) = points.first() else {
        return 0.0;
    };
    let Some(last) = points.last() else {
        return 0.0;
    };
    (last.x - first.x).abs()
}

fn exponential_max_error(points: &[Point], amplitude: f32, rate: f32, offset: f32) -> f32 {
    points
        .iter()
        .map(|point| (point.y - eval_exponential(amplitude, rate, offset, point.x)).abs())
        .fold(0.0, f32::max)
}

fn eval_exponential(amplitude: f32, rate: f32, offset: f32, x: f32) -> f32 {
    offset + amplitude * (rate * x).exp()
}

fn max_error_index(points: &[Point], coefficients: &[f32]) -> usize {
    points
        .iter()
        .enumerate()
        .skip(1)
        .take(points.len().saturating_sub(2))
        .max_by(|(_, a), (_, b)| {
            let a_error = (a.y - eval_polynomial(coefficients, a.x)).abs();
            let b_error = (b.y - eval_polynomial(coefficients, b.x)).abs();
            a_error.total_cmp(&b_error)
        })
        .map(|(index, _)| index)
        .unwrap_or(points.len() / 2)
}

fn max_exponential_error_index(points: &[Point], fit: &ExponentialFit) -> usize {
    points
        .iter()
        .enumerate()
        .skip(1)
        .take(points.len().saturating_sub(2))
        .max_by(|(_, a), (_, b)| {
            let a_error = (a.y - eval_exponential(fit.amplitude, fit.rate, fit.offset, a.x)).abs();
            let b_error = (b.y - eval_exponential(fit.amplitude, fit.rate, fit.offset, b.x)).abs();
            a_error.total_cmp(&b_error)
        })
        .map(|(index, _)| index)
        .unwrap_or(points.len() / 2)
}

fn split_x_monotone_spans(points: &[Point]) -> Vec<Vec<Point>> {
    if points.len() < 2 {
        return Vec::new();
    }

    let mut spans = Vec::new();
    const MIN_TURN_RUN: usize = 2;

    let mut current = vec![points[0]];
    let mut direction = 0_i32;
    let epsilon = 0.01;

    for index in 0..points.len() - 1 {
        let next_direction = segment_x_direction(points[index], points[index + 1], epsilon);

        if direction != 0
            && next_direction != 0
            && next_direction != direction
            && direction_run_len(points, index, next_direction, epsilon) >= MIN_TURN_RUN
        {
            if current
                .last()
                .is_none_or(|last| last.distance(points[index]) > 0.001)
            {
                current.push(points[index]);
            }
            if current.len() >= 2 {
                spans.push(current);
            }
            current = vec![points[index]];
            direction = next_direction;
        }

        current.push(points[index + 1]);
        if next_direction != 0 {
            direction = next_direction;
        }
    }

    if current.len() >= 2 {
        spans.push(current);
    }

    spans
}

fn segment_x_direction(a: Point, b: Point, epsilon: f32) -> i32 {
    let dx = b.x - a.x;
    if dx.abs() < epsilon {
        0
    } else if dx > 0.0 {
        1
    } else {
        -1
    }
}

fn direction_run_len(
    points: &[Point],
    start_segment: usize,
    direction: i32,
    epsilon: f32,
) -> usize {
    let mut count = 0;
    for index in start_segment..points.len().saturating_sub(1) {
        let current = segment_x_direction(points[index], points[index + 1], epsilon);
        if current == 0 {
            continue;
        }
        if current != direction {
            break;
        }
        count += 1;
    }
    count
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

fn sample_exponential_path(
    amplitude: f32,
    rate: f32,
    offset: f32,
    min_x: f32,
    max_x: f32,
) -> Vec<Point> {
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
            y: eval_exponential(amplitude, rate, offset, x),
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

    #[test]
    fn function_fit_splits_on_x_turns() {
        let points = vec![
            Point { x: 0.0, y: 0.0 },
            Point { x: 1.0, y: 1.0 },
            Point { x: 2.0, y: 0.0 },
            Point { x: 1.0, y: -1.0 },
            Point { x: 0.0, y: 0.0 },
        ];

        let spans = split_x_monotone_spans(&points);

        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0].first().unwrap().x, 0.0);
        assert_eq!(spans[0].last().unwrap().x, 2.0);
        assert_eq!(spans[1].first().unwrap().x, 2.0);
        assert_eq!(spans[1].last().unwrap().x, 0.0);
    }

    #[test]
    fn function_fit_ignores_single_segment_x_jitter() {
        let points = vec![
            Point { x: 0.0, y: 0.0 },
            Point { x: 1.0, y: 0.2 },
            Point { x: 2.0, y: 0.4 },
            Point { x: 1.995, y: 0.41 },
            Point { x: 3.0, y: 0.6 },
            Point { x: 4.0, y: 0.8 },
        ];

        let spans = split_x_monotone_spans(&points);

        assert_eq!(spans.len(), 1);
    }

    #[test]
    fn adaptive_fit_chooses_lowest_acceptable_degree() {
        let settings = GenerationSettings {
            quality: QualityPreset::Precise,
            output_mode: OutputMode::ExperimentalPolynomial,
            simplify_tolerance: 0.08,
            polynomial_degree: 3,
        };
        let points = vec![
            Point { x: -1.0, y: -1.0 },
            Point { x: 0.0, y: 1.0 },
            Point { x: 1.0, y: 3.0 },
            Point { x: 2.0, y: 5.0 },
        ];

        let fit = choose_polynomial_fit(&points, settings).unwrap();

        assert_eq!(fit.degree, 1);
    }

    #[test]
    fn exponential_fit_recovers_basic_exp_shape() {
        let points = (0..8)
            .map(|index| {
                let x = index as f32 * 0.25;
                Point {
                    x,
                    y: 1.0 + 2.0 * (0.6 * x).exp(),
                }
            })
            .collect::<Vec<_>>();

        let fit = choose_exponential_fit(&points).unwrap();

        assert!(fit.max_error < 0.05);
    }

    #[test]
    fn exponential_recursive_fit_rejects_tiny_domain() {
        let settings = GenerationSettings {
            quality: QualityPreset::Precise,
            output_mode: OutputMode::ExperimentalExponential,
            simplify_tolerance: 0.08,
            polynomial_degree: 3,
        };
        let points = vec![
            Point { x: 1.0, y: 0.0 },
            Point { x: 1.02, y: 0.1 },
            Point { x: 1.04, y: 0.2 },
            Point { x: 1.06, y: 0.3 },
        ];
        let mut fits = Vec::new();

        fit_exponential_span_recursive(&points, settings, 0, &mut fits);

        assert!(fits.is_empty());
    }
}
