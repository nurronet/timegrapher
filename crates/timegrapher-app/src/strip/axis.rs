//! The labels on the strip's ms axis, across the time.

use super::fmt_ms;
use timegrapher_core::chart_scale::round_step;

/// Which grid lines across the strip, `step` ms and `step_px` points
/// apart, carry a label: every `n`th, the fewest skipped that leaves
/// `need` points between labels and keeps them on the 1-2-5 series
/// (Hugh, #1512): 5 ms lines labelled every 10, else 20, else 50 ms,
/// never 25. `None` when none does.
pub(super) fn label_every(step: f64, step_px: f32, need: f32) -> Option<i64> {
    // Every series step is one of these many lines, whatever the step;
    // the rest (4 of 1 ms, 5 of 5 ms) are filtered out as off it.
    [1, 2, 4, 5, 10].into_iter().find(|&n| {
        let at = step * n as f64;
        step_px * n as f32 > need && (round_step(at) - at).abs() < at * 1e-9
    })
}

/// A signed number of ms for an axis label.
pub(super) fn axis_ms(ms: f64) -> String {
    if ms.abs() < 1e-9 {
        "0 ms".into()
    } else {
        let sign = if ms > 0.0 { "+" } else { "−" };
        if ms.abs() >= 1000.0 {
            format!("{sign}{} s", fmt_ms(ms.abs() / 1000.0))
        } else {
            format!("{sign}{}", fmt_ms(ms.abs()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn axis_labels_turn_to_seconds() {
        assert_eq!(axis_ms(0.0), "0 ms");
        assert_eq!(axis_ms(-250.0), "−250");
        assert_eq!(axis_ms(2000.0), "+2.0 s");
        assert_eq!(axis_ms(-20_000.0), "−20 s");
    }

    /// Lines far enough apart are all labelled; closer, the labels step
    /// up the 1-2-5 series, never off it (2 ms lines labelled 4 apart,
    /// 5 ms lines 25 apart); none when nothing on it leaves room (#1512: ±20 ms on a short strip, lines 5 ms and
    /// 21 pt apart, labelled −10, 0, +10).
    #[test]
    fn the_ms_axis_labels_every_line_or_every_second_or_fifth() {
        assert_eq!(label_every(5.0, 30.0, 26.0), Some(1));
        assert_eq!(label_every(5.0, 21.0, 26.0), Some(2));
        // 5 ms lines: 20 ms (every 4th) before 50, never 25 (every 5th).
        assert_eq!(label_every(5.0, 8.0, 26.0), Some(4));
        assert_eq!(label_every(5.0, 6.0, 26.0), Some(10));
        assert_eq!(label_every(5.0, 2.0, 26.0), None);
        assert_eq!(label_every(1.0, 8.0, 26.0), Some(5));
        assert_eq!(label_every(1.0, 2.0, 26.0), None);
        // 2 ms lines: 10 ms, never 4.
        assert_eq!(label_every(2.0, 21.0, 26.0), Some(5));
        assert_eq!(label_every(1.0, 21.0, 26.0), Some(2));
        assert_eq!(label_every(0.5, 21.0, 26.0), Some(2));
    }
}
