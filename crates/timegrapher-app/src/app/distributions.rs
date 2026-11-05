//! The Distributions pane: how each reading's values spread over the take.

use super::*;
use crate::strength::{self, Grid, Strength};

/// The width kept for the shares up the side of a distribution's plot.
const SIDE_AXIS_W: f32 = 44.0;

impl TimegrapherApp {
    pub(super) fn histogram_settings(&mut self, ui: &mut egui::Ui) {
        theme::tool_choice(
            ui,
            "Show",
            "The values on probability paper, or their cumulative share",
            &mut self.hist_cumulative,
            &[
                (
                    false,
                    Icon::ProbLine,
                    "Probability",
                    "Each value against its rank on a scale of standard deviations: \
                             one steady state falls on the dashed straight line, two states \
                             draw two lines joined by a bend",
                ),
                (
                    true,
                    Icon::SCurve,
                    "Cumulative",
                    "The share of values at or below each value: one state rises in \
                             one steep stretch, two states rise twice with a flatter stretch \
                             between",
                ),
            ],
        );
        theme::tool_choice(
            ui,
            "Values",
            "What the distributions count",
            &mut self.hist_readings,
            &[
                (
                    true,
                    Icon::Bucket,
                    "Readings",
                    "The readings, each averaged over Average Over, as on the charts \
                             and the strip, one per Average Over so that no two share beats: \
                             states lasting longer than that stand out clearly",
                ),
                (
                    false,
                    Icon::TwoSeconds,
                    "2 s",
                    "Each 2 seconds of beats, the finest the app measures: catches \
                             quicker changes, with more scatter",
                ),
            ],
        );
    }

    /// The 2-second stretches of amplitude and beat error in the position
    /// the watch is in now, settled: core's one rule for which windows a
    /// stretch takes ([`core_session::stretch_windows`], #636), so none
    /// overlaps a masked stretch or reaches across a break, as the
    /// readings do.
    pub(super) fn position_windows(&self) -> Vec<&timegrapher_core::amplitude::AmplitudeWindow> {
        let Some(live) = self.live.as_ref() else {
            return Vec::new();
        };
        let cuts = live.cuts();
        self.session
            .current_spans(self.position)
            .into_iter()
            .flat_map(|(a, b)| core_session::stretch_windows(&cuts, live.amplitude_windows(), a, b))
            .collect()
    }

    /// The Distributions pane: a plot for each reading shown, one above
    /// the other, sharing the pane's height.
    pub(super) fn distributions_pane(&mut self, ui: &mut egui::Ui) {
        let shown = self.dist_series;
        self.note_need(ui, Pane::Distributions, DIST_MIN_H);
        stacked(ui, shown, DIST_MIN_H, |ui, k| self.histogram(ui, k));
    }

    /// How often each value of a reading came up, over the session or the
    /// strip's length: bars, or the cumulative share.
    pub(super) fn histogram(&mut self, ui: &mut egui::Ui, reading: SeriesKind) {
        use timegrapher_core::distribution as dist;
        let pal = theme::pal(ui);
        let short = !self.hist_readings;
        if reading == SeriesKind::Rate && short {
            match self.live.as_ref() {
                Some(live) => self.rate_2s.refresh(live, &self.masked),
                None => self.rate_2s = RateTwoS::default(),
            }
        }
        let (values, kind, color, label, source) = self.hist_values(reading, short, pal);
        if let Some(selected) = source {
            if let Some(source) = selected.beat_error_source {
                ui.label(
                    RichText::new(timegrapher_core::session::beat_error_source_label(Some(
                        source,
                    )))
                    .small()
                    .color(pal.text_secondary),
                );
            }
            for caveat in selected.caveats() {
                if let Some(text) = caveat.long(None) {
                    ui.label(RichText::new(text).small().color(pal.text_secondary));
                }
            }
        }
        let step = if label == "2 s" {
            dist::STRETCH_S
        } else {
            self.average_s
        };
        let Some(d) = dist::distribution(&values, kind, label, step) else {
            ui.label(
                RichText::new(format!(
                    "{}: not enough readings yet",
                    steady::title(reading)
                ))
                .small()
                .color(pal.text_secondary),
            );
            return;
        };
        let only = (!self.session.stretches().is_empty()).then(|| self.position.name());
        let line = format!(
            "{}: {}",
            steady::title(reading),
            d.line(kind, " in the session", only)
        );
        ui.add(egui::Label::new(RichText::new(&line).small().color(pal.text_secondary)).truncate())
            .on_hover_text(line.clone());
        let (x_lo, x_hi) = (d.edges[0], d.edges[d.edges.len() - 1]);
        let unit_owned = kind.suffix.to_string();
        let cumulative = self.hist_cumulative;
        let plot_w = f64::from(ui.available_width() - SIDE_AXIS_W);
        let plot = Plot::new(format!("{} Distribution", steady::title(reading)))
            // As short as its share of the pane, rather than spilling out.
            .min_size(Vec2::new(64.0, 24.0))
            .allow_scroll(false)
            .allow_zoom(false)
            .allow_drag(false)
            .allow_axis_zoom_drag(false)
            .allow_boxed_zoom(false)
            .allow_double_click_reset(false)
            .show_x(false)
            .show_y(false)
            .x_axis_formatter(move |m, r| {
                let v = timegrapher_core::chart_scale::axis_number(m.value, m.step_size);
                theme::clear_of_ends(format!("{v}{unit_owned}"), m.value, r, plot_w)
            })
            .y_axis_min_width(SIDE_AXIS_W)
            .default_x_bounds(x_lo, x_hi);
        if cumulative {
            return cumulative_plot(ui, plot, &values, (x_lo, x_hi), color);
        }
        probability_paper(ui, plot, &values, d.normal_line, (x_lo, x_hi), color);
    }

    /// The values of `reading` a distribution counts, in the position the
    /// watch is in now: each 2 s stretch (`short`) or one reading per
    /// averaging time. With them, the scale they are on, their colour and
    /// what each value is.
    pub(super) fn hist_values(
        &self,
        reading: SeriesKind,
        short: bool,
        pal: &theme::Palette,
    ) -> (
        Vec<f64>,
        timegrapher_core::distribution::Kind,
        Color32,
        &'static str,
        Option<core_session::BeatErrorSelection>,
    ) {
        // Only the position the watch is in now: values from another
        // position are a different measurement.
        let inside = |t: f64| self.in_position(t);
        let windows = self.position_windows();
        // One reading per averaging time: a new reading comes every half
        // second, and the ones between share most of their beats, so counting
        // them all would stack up repeats and look surer than it is.
        let every = self.average_s;
        let trend = &self.trend;
        let readings = |f: fn(&TrendPoint) -> Option<f64>| -> Vec<f64> {
            timegrapher_core::live::spaced_readings(
                trend
                    .iter()
                    .filter(|p| inside(p.t))
                    .filter_map(|p| Some((p.t, f(p)?))),
                every,
            )
        };
        use timegrapher_core::distribution::{AMPLITUDE, BEAT_ERROR, RATE};
        match reading {
            SeriesKind::Rate if short => (
                self.rate_2s
                    .values
                    .iter()
                    .filter(|&&(a, _)| inside(a + RateTwoS::STEP_S) && self.in_position(a + 1e-6))
                    .map(|&(_, v)| v)
                    .collect(),
                RATE,
                pal.trace_rate,
                "2 s",
                None,
            ),
            SeriesKind::Rate => (
                readings(|p| p.rate_s_per_day),
                RATE,
                pal.trace_rate,
                "readings",
                None,
            ),
            SeriesKind::Amplitude if short => (
                windows.iter().filter_map(|w| w.mean()).collect(),
                AMPLITUDE,
                pal.trace_amplitude,
                "2 s",
                None,
            ),
            SeriesKind::Amplitude => (
                readings(|p| p.amplitude_deg),
                AMPLITUDE,
                pal.trace_amplitude,
                "readings",
                None,
            ),
            SeriesKind::BeatError => {
                let selected = self.hist_beat_errors(short);
                let values = selected.values.iter().map(|(_, v)| *v).collect();
                (
                    values,
                    BEAT_ERROR,
                    pal.trace_beat_error,
                    if short { "2 s" } else { "readings" },
                    Some(selected),
                )
            }
        }
    }
    fn hist_beat_errors(&self, short: bool) -> core_session::BeatErrorSelection {
        if short {
            return core_session::select_beat_errors(self.position_windows().iter().map(|w| {
                core_session::chosen_beat_error(w.beat_error_unlock_ms, w.beat_error_ms)
            }));
        }
        let readings = timegrapher_core::live::spaced_readings(
            self.trend
                .iter()
                .filter(|p| self.in_position(p.t))
                .filter_map(|p| Some((p.t, p.beat_error()?))),
            self.average_s,
        );
        core_session::select_beat_errors(readings.into_iter().map(Some))
    }
}

/// A distribution as the share of values at or below each value, as a step
/// line between `x_lo` and `x_hi`.
fn cumulative_plot(
    ui: &mut egui::Ui,
    plot: Plot<'_>,
    values: &[f64],
    (x_lo, x_hi): (f64, f64),
    color: Color32,
) {
    use timegrapher_core::histogram as hist;
    // The share of values at or below each value, as a step line.
    let mut pts: Vec<[f64; 2]> = Vec::new();
    for [x, q] in hist::ecdf(values) {
        if x < x_lo || x > x_hi {
            continue;
        }
        if let Some(&[_, prev]) = pts.last() {
            pts.push([x, prev]);
        }
        pts.push([x, q * 100.0]);
    }
    let pal = theme::pal(ui);
    let grid = Grid::new(value_grid, |g| theme::even_grid(g, 30.0, &[]));
    grid.on(plot)
        .y_axis_formatter(|m, _| {
            if m.value < -1e-9 || m.value > 100.0 + 1e-9 {
                String::new()
            } else {
                format!("{}%", m.value.abs())
            }
        })
        .default_y_bounds(0.0, 100.0)
        .show(ui, |p| {
            grid.draw(p, pal);
            p.line(
                Line::new("share at or below", pts)
                    .color(color)
                    .width(1.8_f32),
            );
        });
}

/// A distribution on probability paper: the share at or below each value
/// on a scale of standard deviations, labelled in percent, with the line
/// one steady state would follow.
fn probability_paper(
    ui: &mut egui::Ui,
    plot: Plot<'_>,
    values: &[f64],
    normal_line: Option<timegrapher_core::distribution::NormalLine>,
    (x_lo, x_hi): (f64, f64),
    color: Color32,
) {
    use timegrapher_core::histogram as hist;
    let pal = theme::pal(ui);
    // Probability paper: the share at or below each value on a scale of
    // standard deviations, labelled in percent.
    let pts: Vec<[f64; 2]> = hist::probability_plot(values)
        .into_iter()
        .filter(|p| p[0] >= x_lo && p[0] <= x_hi)
        .collect();
    let z_top = hist::normal_quantile(1.0 - 0.5 / values.len().max(2) as f64).min(3.3) + 0.25;
    let line = normal_line.map(|l| {
        vec![
            [l.centre - l.spread * z_top, -z_top],
            [l.centre + l.spread * z_top, z_top],
        ]
    });
    let grid = Grid::new(value_grid, move |g: egui_plot::GridInput| {
        // From 50% outwards, leaving out a share whose label would crowd
        // the one before it.
        let px = 8.0 / g.base_step_size;
        let mut kept: Vec<f64> = Vec::new();
        for &(pc, _) in &PROBABILITY_MARKS[5..] {
            let z = hist::normal_quantile(pc / 100.0);
            if z <= z_top && kept.last().is_none_or(|&k| (z - k) * px >= 16.0) {
                kept.push(z);
            }
        }
        kept.iter()
            .flat_map(|&z| if z == 0.0 { vec![z] } else { vec![z, -z] })
            .map(|value| GridMark {
                value,
                step_size: 1.0,
            })
            .collect()
    });
    grid.on(plot)
        .y_axis_formatter(|m, _| {
            PROBABILITY_MARKS
                .iter()
                .find(|&&(pc, _)| (hist::normal_quantile(pc / 100.0) - m.value).abs() < 1e-6)
                .map_or(String::new(), |&(_, label)| label.to_string())
        })
        .default_y_bounds(-z_top, z_top)
        .show(ui, |p| {
            grid.draw(p, pal);
            // The line one steady state would follow: a call-out, dashed.
            if let Some(l) = line {
                let s = strength::stroke(Strength::CallOut, pal);
                p.line(
                    Line::new("one steady state", l)
                        .color(s.color)
                        .style(LineStyle::dashed_loose())
                        .width(s.width),
                );
            }
            p.points(
                Points::new("values", pts)
                    .color(color)
                    .radius(1.6_f32)
                    .filled(true),
            );
        });
}

/// A distribution's value axis: every grid line and label alike, at
/// least 80 points apart.
fn value_grid(g: egui_plot::GridInput) -> Vec<GridMark> {
    theme::even_grid(g, 80.0, &[])
}

#[cfg(test)]
mod source_tests {
    use super::*;
    #[test]
    fn beat_error_distribution_selects_after_spacing_and_reports_left_out_readings() {
        let mut app = super::super::tests::synthetic(30.0, 0.0);
        app.average_s = 2.0;
        app.trend = [Some(0.2), None, None, None, Some(0.4)]
            .into_iter()
            .enumerate()
            .map(|(i, unlock)| TrendPoint {
                t: i as f64,
                rate_s_per_day: None,
                amplitude_deg: None,
                amplitude_even_deg: None,
                amplitude_odd_deg: None,
                beat_error_unlock_ms: unlock,
                beat_error_ms: Some(0.8),
                rate_error_s_per_day: None,
                amplitude_error_deg: None,
                beat_error_unlock_error_ms: None,
                heard: true,
            })
            .collect();
        let selected = app.hist_beat_errors(false);
        assert_eq!(
            selected.beat_error_sources,
            core_session::BeatErrorSources { unlock: 2, drop: 1 }
        );
        assert_eq!(selected.values, vec![(0, 0.2), (2, 0.4)]);
        assert!(matches!(
            selected.caveats().as_slice(),
            [Caveat::BeatErrorSourceReadings {
                used: 2,
                readings: 3,
                ..
            }]
        ));
        for p in &mut app.trend {
            p.beat_error_unlock_ms = None;
        }
        let selected = app.hist_beat_errors(false);
        assert_eq!(
            selected.beat_error_source,
            Some(core_session::BeatErrorSource::Drop)
        );
        assert_eq!(selected.values.len(), 3);
        assert!(selected.caveats().contains(&Caveat::BeatErrorFromDrop));
    }
}
