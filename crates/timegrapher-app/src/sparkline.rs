//! The reading cards' sparklines ("Reading Card Sparklines", Hugh's spec,
//! approved 28 Oct): Rate, Amplitude and Beat Error each draw their own
//! figure over the last minute, so a glance says whether it is steady or
//! moving. The trend's home is the Charts pane; these are the agreed
//! exception, since Charts and the strip's overlays start hidden.
//!
//! Where it sits: beside the figure, across the figure and controls rows,
//! when the card leaves [`BESIDE_FREE`] or more free beside the figure;
//! else in the card's bottom row under it. Either way the card keeps its
//! height, so nothing moves, and the three cards, always one width,
//! switch together.

use crate::theme::{self, Palette};
use eframe::egui::{self, Color32, Pos2, Rect, Stroke, Vec2};
use timegrapher_core::figure_text::{figure, true_minus};
use timegrapher_core::library::base_digits as base;
use timegrapher_core::live::TrendReading;

/// How far back a sparkline reaches, seconds.
pub const SPAN_S: i64 = 60;

/// The room the figure and its unit keep beside a sparkline, pt.
const FIGURE_ROOM: f32 = 220.0;

/// The room beside the figure a sparkline needs to sit there, pt.
const BESIDE_FREE: f32 = 200.0;

/// Room between the figure and a sparkline beside it, and between the
/// sparkline and its labels, pt.
const SPACE: f32 = 12.0;

/// A reading a card draws: how to read it from a trend reading, and how
/// its figures are written.
pub struct Reading {
    /// Its figure in a trend reading.
    pub value: fn(&TrendReading) -> Option<f64>,
    /// Written after a figure: " s/d", "°", " ms".
    pub unit: &'static str,
    /// The narrowest the scale is, so a steady watch draws a nearly flat
    /// line instead of its noise blown up into swings.
    pub least: f64,
    /// Decimals, as the card writes the figure with no ±.
    pub digits: usize,
    /// Written with its sign, as a rate is.
    pub signed: bool,
}

pub const RATE: Reading = Reading {
    value: |p| p.rate_s_per_day,
    unit: " s/d",
    least: 2.0,
    digits: base::RATE,
    signed: true,
};

pub const AMPLITUDE: Reading = Reading {
    value: |p| p.amplitude_deg,
    unit: "°",
    least: 4.0,
    digits: base::AMPLITUDE,
    signed: false,
};

/// Beat error as the card shows it: its size.
pub const BEAT_ERROR: Reading = Reading {
    value: |p| timegrapher_core::session::beat_error_size(p.beat_error_unlock_ms, p.beat_error_ms),
    unit: " ms",
    least: 0.1,
    digits: base::BEAT_ERROR,
    signed: false,
};

/// Where a card's sparkline goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
    /// Beside the figure, across the figure and controls rows, with its
    /// highest and lowest values at its left.
    Beside,
    /// In the bottom row under the figure, full width, without labels.
    Under,
}

/// Where the sparkline goes in a reading card whose rows inside its
/// padding are `inner`, `figure` and `controls`: the room left for the
/// figure, the sparkline's, and which place that is.
pub fn lay(inner: Rect, figure: Rect, controls: Rect) -> (Rect, Rect, Place) {
    if inner.width() - FIGURE_ROOM >= BESIDE_FREE {
        let split = inner.left() + FIGURE_ROOM;
        let fig = Rect::from_x_y_ranges(figure.left()..=split, figure.y_range());
        let spark = Rect::from_x_y_ranges(
            split + SPACE..=inner.right(),
            figure.top()..=controls.bottom(),
        );
        (fig, spark, Place::Beside)
    } else {
        (figure, controls, Place::Under)
    }
}

/// The points to draw, time and figure: `None` where a second has no
/// figure (masked, unheard or not a whole window), which leaves a gap.
pub fn points(trend: &[TrendReading], r: &Reading) -> Vec<Option<[f64; 2]>> {
    trend
        .iter()
        .map(|p| (r.value)(p).filter(|_| p.heard).map(|v| [p.t, v]))
        .collect()
}

/// The scale the points are drawn on: their lowest to highest, widened
/// about its middle to `least`. `None` with fewer than two figures.
pub fn scale(points: &[Option<[f64; 2]>], least: f64) -> Option<(f64, f64)> {
    let mut vs = points.iter().flatten().map(|p| p[1]);
    let first = vs.next()?;
    let (mut lo, mut hi, mut n) = (first, first, 1);
    for v in vs {
        lo = lo.min(v);
        hi = hi.max(v);
        n += 1;
    }
    if n < 2 {
        return None;
    }
    if hi - lo < least {
        let mid = (lo + hi) / 2.0;
        (lo, hi) = (mid - least / 2.0, mid + least / 2.0);
    }
    Some((lo, hi))
}

/// A figure as the card writes it with no ±, with a true minus.
fn text(v: f64, r: &Reading) -> String {
    true_minus(&format!(
        "{}{}",
        figure(v, None, r.digits, r.signed),
        r.unit
    ))
}

/// The sparkline's facts in a line, for its hover: "Last 60 s: −46.9 to
/// −42.1 s/d, now −44.9 s/d". `None` with fewer than two figures.
pub fn facts(points: &[Option<[f64; 2]>], r: &Reading) -> Option<String> {
    let vs: Vec<f64> = points.iter().flatten().map(|p| p[1]).collect();
    if vs.len() < 2 {
        return None;
    }
    let lo = vs.iter().copied().fold(f64::INFINITY, f64::min);
    let hi = vs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let now = *vs.last()?;
    let range = format!(
        "{} to {}",
        figure(lo, None, r.digits, r.signed),
        text(hi, r)
    );
    Some(format!(
        "Last {SPAN_S} s: {}, now {}",
        true_minus(&range),
        text(now, r)
    ))
}

/// Draw the sparkline of `trend` in `rect`, at `place`, in `colour`, with
/// its facts in the hover. Nothing with fewer than two figures.
pub fn draw(
    ui: &mut egui::Ui,
    rect: Rect,
    place: Place,
    trend: &[TrendReading],
    r: &Reading,
    colour: Color32,
) {
    let pts = points(trend, r);
    let (Some((lo, hi)), Some(facts)) = (scale(&pts, r.least), facts(&pts, r)) else {
        return;
    };
    let pal: &Palette = theme::pal(ui);
    let painter = ui.painter().with_clip_rect(rect.expand(3.0));
    let mut plot = rect.shrink2(Vec2::new(3.0, 3.0));
    if place == Place::Beside {
        // Its highest and lowest figures, not the scale's ends, so the
        // size of the wiggle reads without a hover.
        let vs = pts.iter().flatten().map(|p| p[1]);
        let top = vs.clone().fold(f64::NEG_INFINITY, f64::max);
        let bottom = vs.fold(f64::INFINITY, f64::min);
        let font = egui::FontId::proportional(11.0);
        let label = |v: f64| true_minus(&figure(v, None, r.digits, r.signed));
        let top = painter.layout_no_wrap(label(top), font.clone(), pal.text_tertiary);
        let bottom = painter.layout_no_wrap(label(bottom), font, pal.text_tertiary);
        let w = top.size().x.max(bottom.size().x);
        painter.galley(Pos2::new(plot.left(), plot.top()), top, pal.text_tertiary);
        let y = plot.bottom() - bottom.size().y;
        painter.galley(Pos2::new(plot.left(), y), bottom, pal.text_tertiary);
        plot = Rect::from_x_y_ranges(plot.left() + w + SPACE..=plot.right(), plot.y_range());
    }
    let end = trend.last().map_or(0.0, |p| p.t);
    let start = end - SPAN_S as f64;
    let at = |p: [f64; 2]| {
        let x = plot.left() + ((p[0] - start) / SPAN_S as f64) as f32 * plot.width();
        let y = plot.bottom() - ((p[1] - lo) / (hi - lo)) as f32 * plot.height();
        Pos2::new(x, y)
    };
    let stroke = Stroke::new(crate::strength::DATA_W, colour);
    let mut run: Vec<Pos2> = Vec::new();
    for p in pts.iter().chain(std::iter::once(&None)) {
        match p {
            Some(p) => run.push(at(*p)),
            None => {
                if run.len() > 1 {
                    painter.line(std::mem::take(&mut run), stroke);
                }
                run.clear();
            }
        }
    }
    if let Some(Some(last)) = pts.last() {
        painter.circle_filled(at(*last), 2.5, colour);
    }
    ui.interact(
        rect,
        ui.id().with(("sparkline", rect.left() as i32)),
        egui::Sense::hover(),
    )
    .on_hover_text(facts);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(t: f64, rate: Option<f64>) -> TrendReading {
        TrendReading {
            t,
            rate_s_per_day: rate,
            amplitude_deg: None,
            amplitude_even_deg: None,
            amplitude_odd_deg: None,
            beat_error_unlock_ms: None,
            beat_error_ms: None,
            heard: true,
        }
    }

    /// A second with no figure leaves a gap; the scale is the figures'
    /// own, widened about its middle to the least, and the hover says
    /// the range and the newest figure as the card writes them.
    #[test]
    fn a_sparkline_reads_its_figures_with_gaps_and_a_least_scale() {
        let trend = [
            at(1.0, Some(-46.9)),
            at(2.0, None),
            at(3.0, Some(-42.1)),
            at(4.0, Some(-44.94)),
        ];
        let pts = points(&trend, &RATE);
        assert_eq!(pts[1], None);
        assert_eq!(scale(&pts, RATE.least), Some((-46.9, -42.1)));
        assert_eq!(
            facts(&pts, &RATE).as_deref(),
            Some("Last 60 s: −46.9 to −42.1 s/d, now −44.9 s/d")
        );
        // A steady watch: a scale of at least 2 s/d about its middle.
        let steady = [at(1.0, Some(3.0)), at(2.0, Some(3.2))];
        let (lo, hi) = scale(&points(&steady, &RATE), RATE.least).unwrap();
        assert!(
            (lo - 2.1).abs() < 1e-9 && (hi - 4.1).abs() < 1e-9,
            "{lo} {hi}"
        );
        // Fewer than two figures: nothing to draw.
        let one = [at(1.0, Some(3.0)), at(2.0, None)];
        assert_eq!(scale(&points(&one, &RATE), RATE.least), None);
        assert_eq!(facts(&points(&one, &RATE), &RATE), None);
        // An unheard second is a gap too.
        let mut unheard = at(5.0, Some(1.0));
        unheard.heard = false;
        assert_eq!(points(&[unheard], &RATE), vec![None]);
    }

    /// Beside the figure with 200 pt or more free beside it, across the
    /// figure and controls rows; else in the bottom row.
    #[test]
    fn a_sparkline_sits_beside_a_wide_cards_figure_and_under_a_narrow_ones() {
        let rows = |w: f32| {
            let inner = Rect::from_min_size(Pos2::ZERO, Vec2::new(w, 102.0));
            let figure = Rect::from_min_size(Pos2::new(0.0, 26.0), Vec2::new(w, 40.0));
            let controls = Rect::from_min_size(Pos2::new(0.0, 74.0), Vec2::new(w, 28.0));
            lay(inner, figure, controls)
        };
        let (fig, spark, place) = rows(600.0);
        assert_eq!(place, Place::Beside);
        assert_eq!(fig.right(), FIGURE_ROOM);
        assert_eq!((spark.top(), spark.bottom()), (26.0, 102.0));
        assert_eq!(spark.height(), 76.0);
        let (fig, spark, place) = rows(183.0);
        assert_eq!(place, Place::Under);
        assert_eq!(fig.width(), 183.0);
        assert_eq!(spark.height(), 28.0);
    }
}
