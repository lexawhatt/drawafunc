use crate::model::Point;

pub(crate) fn batched_segments(segments: &[(Point, Point)]) -> String {
    if segments.is_empty() {
        return String::new();
    }

    let x1: Vec<_> = segments.iter().map(|(a, _)| a.x).collect();
    let x2: Vec<_> = segments.iter().map(|(_, b)| b.x).collect();
    let y1: Vec<_> = segments.iter().map(|(a, _)| a.y).collect();
    let y2: Vec<_> = segments.iter().map(|(_, b)| b.y).collect();

    [
        format!("X_1={}", fmt_num_list(&x1)),
        format!("X_2={}", fmt_num_list(&x2)),
        format!("Y_1={}", fmt_num_list(&y1)),
        format!("Y_2={}", fmt_num_list(&y2)),
        "(X_1+(X_2-X_1)*t,Y_1+(Y_2-Y_1)*t)".to_owned(),
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
    fn batched_export_uses_lists_and_one_curve() {
        let export = batched_segments(&[
            (Point { x: 0.0, y: 1.0 }, Point { x: 2.0, y: 3.0 }),
            (Point { x: -2.0, y: 1.0 }, Point { x: -2.0, y: -3.0 }),
        ]);

        assert_eq!(
            export,
            [
                "X_1=[0,-2]",
                "X_2=[2,-2]",
                "Y_1=[1,1]",
                "Y_2=[3,-3]",
                "(X_1+(X_2-X_1)*t,Y_1+(Y_2-Y_1)*t)",
            ]
            .join("\n")
        );
    }

    #[test]
    fn batched_export_is_empty_without_segments() {
        assert!(batched_segments(&[]).is_empty());
    }
}
