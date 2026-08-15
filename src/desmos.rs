use crate::model::Point;
use crate::primitives::{CubicBezier, GeneratedPrimitive};

pub(crate) fn export_lines(primitives: &[GeneratedPrimitive]) -> String {
    let segments = primitives.iter().filter_map(|primitive| match primitive {
        GeneratedPrimitive::Line { from, to } => Some((*from, *to)),
        GeneratedPrimitive::CubicBezier(_) => None,
    });
    batched_segments(segments)
}

pub(crate) fn export_beziers(primitives: &[GeneratedPrimitive]) -> String {
    let beziers = primitives.iter().filter_map(|primitive| match primitive {
        GeneratedPrimitive::CubicBezier(bezier) => Some(*bezier),
        GeneratedPrimitive::Line { .. } => None,
    });
    batched_beziers(beziers)
}

pub(crate) fn export_mixed(primitives: &[GeneratedPrimitive]) -> String {
    let mut groups = Vec::new();
    let lines = export_lines(primitives);
    if !lines.is_empty() {
        groups.push(lines);
    }

    let beziers = export_beziers(primitives);
    if !beziers.is_empty() {
        groups.push(beziers);
    }

    groups.join("\n")
}

pub(crate) fn export_polynomial(coefficients: &[f32]) -> String {
    if coefficients.is_empty() {
        return String::new();
    }

    let mut parts = Vec::new();
    for (power, coefficient) in coefficients.iter().enumerate().rev() {
        if coefficient.abs() < 0.000_001 {
            continue;
        }

        let term = match power {
            0 => fmt_num(coefficient.abs()),
            1 => format!("{}x", fmt_num(coefficient.abs())),
            _ => format!("{}x^{}", fmt_num(coefficient.abs()), power),
        };

        if parts.is_empty() {
            if *coefficient < 0.0 {
                parts.push(format!("-{term}"));
            } else {
                parts.push(term);
            }
        } else if *coefficient < 0.0 {
            parts.push(format!("-{term}"));
        } else {
            parts.push(format!("+{term}"));
        }
    }

    if parts.is_empty() {
        "y=0".to_owned()
    } else {
        format!("y={}", parts.join(""))
    }
}

fn batched_segments(segments: impl IntoIterator<Item = (Point, Point)>) -> String {
    let segments = segments.into_iter().collect::<Vec<_>>();
    if segments.is_empty() {
        return String::new();
    }

    let x1: Vec<_> = segments.iter().map(|(a, _)| a.x).collect();
    let x2: Vec<_> = segments.iter().map(|(_, b)| b.x).collect();
    let y1: Vec<_> = segments.iter().map(|(a, _)| a.y).collect();
    let y2: Vec<_> = segments.iter().map(|(_, b)| b.y).collect();

    [
        format!("LX_1={}", fmt_num_list(&x1)),
        format!("LX_2={}", fmt_num_list(&x2)),
        format!("LY_1={}", fmt_num_list(&y1)),
        format!("LY_2={}", fmt_num_list(&y2)),
        "(LX_1+(LX_2-LX_1)*t,LY_1+(LY_2-LY_1)*t)".to_owned(),
    ]
    .join("\n")
}

fn batched_beziers(beziers: impl IntoIterator<Item = CubicBezier>) -> String {
    let beziers = beziers.into_iter().collect::<Vec<_>>();
    if beziers.is_empty() {
        return String::new();
    }

    let bx0: Vec<_> = beziers.iter().map(|bezier| bezier.p0.x).collect();
    let bx1: Vec<_> = beziers.iter().map(|bezier| bezier.p1.x).collect();
    let bx2: Vec<_> = beziers.iter().map(|bezier| bezier.p2.x).collect();
    let bx3: Vec<_> = beziers.iter().map(|bezier| bezier.p3.x).collect();
    let by0: Vec<_> = beziers.iter().map(|bezier| bezier.p0.y).collect();
    let by1: Vec<_> = beziers.iter().map(|bezier| bezier.p1.y).collect();
    let by2: Vec<_> = beziers.iter().map(|bezier| bezier.p2.y).collect();
    let by3: Vec<_> = beziers.iter().map(|bezier| bezier.p3.y).collect();

    [
        format!("BX_0={}", fmt_num_list(&bx0)),
        format!("BX_1={}", fmt_num_list(&bx1)),
        format!("BX_2={}", fmt_num_list(&bx2)),
        format!("BX_3={}", fmt_num_list(&bx3)),
        format!("BY_0={}", fmt_num_list(&by0)),
        format!("BY_1={}", fmt_num_list(&by1)),
        format!("BY_2={}", fmt_num_list(&by2)),
        format!("BY_3={}", fmt_num_list(&by3)),
        "((1-t)^3*BX_0+3*(1-t)^2*t*BX_1+3*(1-t)*t^2*BX_2+t^3*BX_3,(1-t)^3*BY_0+3*(1-t)^2*t*BY_1+3*(1-t)*t^2*BY_2+t^3*BY_3)".to_owned(),
    ]
    .join("\n")
}

fn fmt_num_list(values: &[f32]) -> String {
    let values = values
        .iter()
        .map(|value| fmt_num(*value))
        .collect::<Vec<_>>()
        .join(",");
    format!("[{values}]")
}

fn fmt_num(value: f32) -> String {
    let rounded = if value.abs() < 0.000_001 { 0.0 } else { value };
    let mut text = format!("{rounded:.4}");
    while text.contains('.') && text.ends_with('0') {
        text.pop();
    }
    if text.ends_with('.') {
        text.pop();
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fmt_num_trims_noise_and_zeroes() {
        assert_eq!(fmt_num(0.0), "0");
        assert_eq!(fmt_num(1.5000), "1.5");
        assert_eq!(fmt_num(-2.1250), "-2.125");
        assert_eq!(fmt_num(0.000_000_1), "0");
    }

    #[test]
    fn line_export_uses_lists_and_one_curve() {
        let export = export_lines(&[
            GeneratedPrimitive::Line {
                from: Point { x: 0.0, y: 1.0 },
                to: Point { x: 2.0, y: 3.0 },
            },
            GeneratedPrimitive::Line {
                from: Point { x: -2.0, y: 1.0 },
                to: Point { x: -2.0, y: -3.0 },
            },
        ]);

        assert_eq!(
            export,
            [
                "LX_1=[0,-2]",
                "LX_2=[2,-2]",
                "LY_1=[1,1]",
                "LY_2=[3,-3]",
                "(LX_1+(LX_2-LX_1)*t,LY_1+(LY_2-LY_1)*t)",
            ]
            .join("\n")
        );
    }

    #[test]
    fn bezier_export_uses_control_point_lists() {
        let export = export_beziers(&[GeneratedPrimitive::CubicBezier(CubicBezier {
            p0: Point { x: 0.0, y: 0.0 },
            p1: Point { x: 1.0, y: 0.0 },
            p2: Point { x: 1.0, y: 1.0 },
            p3: Point { x: 2.0, y: 1.0 },
        })]);

        assert!(export.contains("BX_0=[0]"));
        assert!(export.contains("BY_3=[1]"));
        assert!(export.contains("(1-t)^3*BX_0"));
    }

    #[test]
    fn polynomial_export_is_real_function() {
        assert_eq!(export_polynomial(&[1.0, 2.0, -3.0]), "y=-3x^2+2x+1");
    }
}
