//! The Charts pane: a chart for each reading shown, sharing the pane's time axis.

use super::*;
use crate::strength::{self, Grid, Strength};
use timegrapher_core::figure_text::{base_digits, figure, true_minus};

impl TimegrapherApp {
    /// The Charts pane: a chart for each reading shown, one above the
    /// other, sharing the pane's height and its time axis.
    pub(super) fn charts_pane(&mut self, ui: &mut egui::Ui) {
        let shown = self.chart_series;
        self.note_need(ui, Pane::Charts, STACKED_MIN_H);
        let mut moved = false;
        self.charts_drawn = stacked(ui, shown, STACKED_MIN_H, |ui, k| {
            moved |= self.chart(ui, k);
        });
        self.charts_fit = !moved;
    }

    /// The pane's one way back to the whole session, in its toolbar as
    /// the Paper Strip's Fit is: on while the charts show it all, turned
    /// off by a pan or a zoom (Walter's walk of #1057: a button on every
    /// chart wrapped over its title and lines, #1063).
    pub(super) fn charts_fit_switch(&mut self, ui: &mut egui::Ui) {
        let mut fit = self.charts_fit;
        let hint = if self.running() {
            "Show the whole session on the charts and keep following new readings. Scrolling, \
             dragging or zooming a chart's time turns it off; double-clicking a chart turns it \
             on."
        } else {
            "Show the whole session on the charts. Scrolling, dragging or zooming a chart's \
             time turns it off; double-clicking a chart turns it on."
        };
        let pal = theme::pal(ui);
        if theme::tool_switch(ui, Icon::Fit, pal.accent, "Fit", &mut fit, hint) && fit {
            self.charts_to_live = true;
            ui.ctx().request_repaint();
        }
    }

    /// One reading's chart, and whether it was moved off the whole
    /// session.
    fn chart(&mut self, ui: &mut egui::Ui, reading: SeriesKind) -> bool {
        let pal = theme::pal(ui);
        // A lane over the plot for the chart's name and its lines' key,
        // so neither sits on the lines (Walter's walk of #1057, #1063).
        let font = egui::TextStyle::Small.resolve(ui.style());
        let (lane, _) = ui.allocate_exact_size(
            Vec2::new(ui.available_width(), font.size + 6.0),
            egui::Sense::hover(),
        );
        let average = self.average_s;
        let (series, unit, decimals) = chart_lines(reading, &self.trend, average, pal);
        let (mut plot, grid) = chart_plot(reading);
        // Scale to hold every point of the lines: the whole take.
        let all: Vec<[f64; 2]> = series.iter().flat_map(|s| s.pts.iter().copied()).collect();
        let y_range = data_range(&all);
        if let Some((lo, hi)) = y_range {
            plot = plot.default_y_bounds(lo, hi);
        }
        let marker = self.strip_end.is_some().then(|| self.strip_end_s());
        let lookup = series.clone();
        let colour = series.first().map_or(pal.text_secondary, |l| l.key);
        let to_live = self.charts_to_live_now;
        // A new averaging time spreads the values wider or narrower, so the
        // scale follows it, keeping the time shown.
        let rescale = self
            .chart_average_drawn
            .insert(steady::title(reading), average)
            .is_some_and(|was| was != average);
        let resp = plot.show(ui, |p| {
            grid.draw(p, pal);
            draw_chart(p, series, to_live, y_range, rescale)
        });
        let (click, hover, moved) = resp.inner;
        // The moment shown and the pointer's time, painted over the plot
        // rather than added to it: a plot line counts towards the automatic
        // bounds, so one under the pointer near the edge widened the chart
        // a little more every frame.
        let frame = *resp.transform.frame();
        let painter = ui.painter_at(frame);
        let t = resp.transform;
        paint_y_labels(ui, &t, unit);
        // Which reading this chart is, in its colour, over its top left,
        // and its lines' key over its top right, in the lane above it.
        let above = egui::Rect::from_min_max(egui::pos2(frame.left(), lane.top()), frame.max);
        ui.painter().text(
            above.left_top() + Vec2::new(0.0, 3.0),
            egui::Align2::LEFT_TOP,
            steady::title(reading),
            font,
            colour,
        );
        let only = steady::SERIES.map(|k| k == reading);
        paint_key(ui.painter(), above, &chart_key(only, pal), ui, pal);
        let at = |x| egui::pos2(t.position_from_point_x(x), frame.top());
        let test = self.in_test();
        if test {
            strip::paint_phases(
                &painter,
                frame,
                &self.strip_settles(),
                &self.guided_measured,
                at,
                true,
                ui.visuals(),
            );
        }
        strip::paint_bands(
            &painter,
            frame,
            &self.position_bands(),
            at,
            true,
            true,
            !test,
            ui.visuals(),
        );
        if hover.is_some() {
            self.cursor_next = hover;
        }
        let cursor = hover.or(self.cursor_t);
        for x in [marker, cursor].into_iter().flatten() {
            let px = resp.transform.position_from_point_x(x);
            if px >= frame.left() && px <= frame.right() {
                painter.vline(
                    px,
                    frame.y_range(),
                    strength::stroke(Strength::CallOut, pal),
                );
            }
        }
        let running = self.running();
        if let Some(x) = hover {
            let trend = &self.trend;
            chart_hover(
                ui,
                frame,
                (x, trend),
                &lookup,
                (unit, decimals),
                reading,
                running,
            );
        }
        if let Some(at) = click {
            if at.x > 0.0 {
                self.strip_fit = false;
                self.strip_end = Some(at.x);
            }
        }
        moved
    }
}

/// A click on a chart, the time under the pointer, and whether it was moved
/// off the whole session.
type ChartInput = (Option<egui_plot::PlotPoint>, Option<f64>, bool);

/// The lines of one reading's chart over the session, each in its reading's
/// colour, with the unit and decimals its values show with.
fn chart_lines(
    reading: SeriesKind,
    trend: &[TrendPoint],
    average: f64,
    pal: &theme::Palette,
) -> (Vec<ChartLine>, &'static str, usize) {
    let pick = |f: fn(&TrendPoint) -> Option<f64>| -> Vec<[f64; 2]> {
        trend
            .iter()
            .filter_map(|p| f(p).map(|v| [p.t, v]))
            .collect()
    };
    // Each line in its reading's colour, as on the strip and in the
    // distributions. Tick's amplitude dashes violet over blue and Tock's
    // violet over deep orange, the key drawn the same; beat error from the
    // drop is a fainter dashed amber.
    let solid = LineStyle::Solid;
    match reading {
        SeriesKind::Rate => (
            vec![ChartLine {
                name: format!(
                    "Rate in seconds per day, {} average",
                    fields::duration(average)
                ),
                pts: pick(|p| p.rate_s_per_day),
                se: |p| p.rate_error_s_per_day,
                color: pal.trace_rate,
                style: solid,
                key: pal.trace_rate,
                under: None,
                main: true,
            }],
            " s/d",
            base_digits::RATE,
        ),
        SeriesKind::Amplitude => (
            vec![
                ChartLine {
                    name: "Average Amplitude".into(),
                    pts: pick(|p| p.amplitude_deg),
                    se: |p| p.amplitude_error_deg,
                    color: pal.trace_amplitude,
                    style: solid,
                    key: pal.trace_amplitude,
                    under: None,
                    main: true,
                },
                ChartLine {
                    name: "Amplitude from Tick".into(),
                    pts: pick(|p| p.amplitude_even_deg),
                    se: |p| p.amplitude_error_deg,
                    color: pal.trace_amplitude,
                    style: LineStyle::Dashed { length: 6.0 },
                    key: pal.tick,
                    under: Some(pal.tick),
                    main: false,
                },
                ChartLine {
                    name: "Amplitude from Tock".into(),
                    pts: pick(|p| p.amplitude_odd_deg),
                    se: |p| p.amplitude_error_deg,
                    color: pal.trace_amplitude,
                    style: LineStyle::Dashed { length: 6.0 },
                    key: pal.tock,
                    under: Some(pal.tock),
                    main: false,
                },
            ],
            "°",
            base_digits::AMPLITUDE,
        ),
        SeriesKind::BeatError => (
            vec![
                ChartLine {
                    name: "From the Unlock".into(),
                    pts: pick(|p| p.beat_error_unlock_ms.map(f64::abs)),
                    se: |p| p.beat_error_unlock_error_ms,
                    color: pal.trace_beat_error,
                    style: solid,
                    key: pal.trace_beat_error,
                    under: None,
                    main: true,
                },
                ChartLine {
                    name: "From the Drop".into(),
                    pts: pick(|p| p.beat_error_ms.map(f64::abs)),
                    se: |_| None,
                    color: pal.trace_beat_error.gamma_multiply(0.6),
                    style: LineStyle::dashed_dense(),
                    key: pal.trace_beat_error.gamma_multiply(0.6),
                    under: None,
                    main: false,
                },
            ],
            " ms",
            base_digits::BEAT_ERROR,
        ),
    }
}

/// A reading's chart, linked in time to the others: the wheel or a drag
/// pans time, Cmd or Ctrl with the wheel or a pinch zooms it, and the
/// scale fits the lines on its own. Its grid, with a zero line on the
/// rate's (0 s/d).
fn chart_plot(reading: SeriesKind) -> (Plot<'static>, Grid) {
    // Short charts still get a label at the top and the bottom: the
    // labels are painted below, in small text, so they fit closer than
    // the plot's own.
    let mut grid = Grid::new(
        |g| theme::even_grid(g, 80.0, &theme::TIME_STEPS),
        theme::short_y_grid,
    );
    if reading == SeriesKind::Rate {
        grid = grid.zero_y();
    }
    let plot = grid
        .on(Plot::new(steady::title(reading))
            // As short as its share of the pane, rather than spilling out.
            .min_size(Vec2::new(64.0, 24.0))
            .link_axis("trend", [true, false])
            .link_cursor("trend", [true, false])
            .allow_scroll(false)
            // The wheel and a drag pan time and Cmd/Ctrl with the wheel
            // zooms it (below), kept to the take. The scale fits the lines
            // on its own, so it neither zooms nor pans: a zoom would only
            // be put back.
            .allow_zoom(false)
            .allow_drag(false)
            .allow_axis_zoom_drag(false)
            .allow_boxed_zoom(false))
        .custom_x_axes(vec![egui_plot::AxisHints::new_x()
            .formatter(|m, r| chart_time_label(m.value, m.step_size, r))])
        .y_axis_min_width(GUTTER)
        .y_axis_formatter(|_, _| String::new())
        // The values at the pointer are shown for every line at once
        // (below), instead of only when the pointer is on a line.
        .show_x(false)
        .show_y(false);
    (plot, grid)
}

/// Draw a chart's lines, broken where there is no reading, and return a
/// click, the time under the pointer and whether the chart was moved off
/// the whole session.
fn draw_chart(
    p: &mut egui_plot::PlotUi<'_>,
    series: Vec<ChartLine>,
    to_live: bool,
    y_range: Option<(f64, f64)>,
    rescale: bool,
) -> ChartInput {
    if to_live {
        p.set_auto_bounds([true, true]);
    }
    if let (Some((lo, hi)), true) = (y_range, rescale) {
        p.set_plot_bounds_y(lo..=hi);
    }
    pan_and_zoom(p, &series);
    for l in series {
        let w = if l.main { 1.8_f32 } else { 1.2_f32 };
        // Broken where there is no reading (a masked stretch, a
        // pause, beats not found), not drawn straight across.
        for seg in segments(&l.pts) {
            if let Some(c) = l.under {
                p.line(
                    Line::new(format!("{} gaps", l.name), seg.clone())
                        .color(c)
                        .width(w),
                );
            }
            p.line(
                Line::new(l.name.clone(), seg)
                    .color(l.color)
                    .style(l.style)
                    .width(w),
            );
        }
    }
    let (hovered, clicked) = {
        let r = p.response();
        (r.hovered(), r.clicked() && !r.double_clicked())
    };
    let hover = p.pointer_coordinate().filter(|_| hovered);
    let click = clicked.then(|| p.pointer_coordinate()).flatten();
    // Panned or zoomed away from the whole session.
    let moved = !to_live && !p.auto_bounds().x;
    (click, hover.map(|h| h.x), moved)
}

/// Pan and zoom a chart's time by hand, kept to the take. The wheel, a
/// swipe either way or a drag pans; Cmd (Mac) or Ctrl with the wheel, or
/// a pinch, zooms about the pointer. Alt and the wheel zooms the strip's
/// width; the charts' scale fits their lines, so here it does nothing.
/// Zoomed out to the whole take, the chart shows the whole session again.
fn pan_and_zoom(p: &mut egui_plot::PlotUi<'_>, series: &[ChartLine]) {
    let r = p.response();
    let dragged = r.dragged_by(egui::PointerButton::Primary);
    if !r.hovered() && !dragged {
        return;
    }
    let drag = if dragged { r.drag_delta().x } else { 0.0 };
    let px = r.rect.width().max(1.0);
    if dragged {
        p.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
    }
    let (s, zoom, alt) = p
        .ctx()
        .input(|i| (i.smooth_scroll_delta, i.zoom_delta(), i.modifiers.alt));
    let shift = drag + if alt { 0.0 } else { s.x + s.y };
    if shift == 0.0 && zoom == 1.0 {
        return;
    }
    let b = p.plot_bounds();
    let (mut lo, mut hi) = (b.min()[0], b.max()[0]);
    if zoom != 1.0 {
        let at = p.pointer_coordinate().map_or((lo + hi) / 2.0, |c| c.x);
        lo = at + (lo - at) / f64::from(zoom);
        hi = at + (hi - at) / f64::from(zoom);
    }
    let dt = -f64::from(shift / px) * (hi - lo);
    let Some(take) = take_span(series) else {
        return;
    };
    match keep_in(lo + dt, hi + dt, take) {
        Some((lo, hi)) => p.set_plot_bounds_x(lo..=hi),
        None => {
            let y = p.auto_bounds().y;
            p.set_auto_bounds([true, y]);
        }
    }
}

/// The earliest and latest times the charts' lines reach.
fn take_span(series: &[ChartLine]) -> Option<(f64, f64)> {
    let mut xs = series
        .iter()
        .flat_map(|l| l.pts.iter().map(|p| p[0]))
        .filter(|x| x.is_finite());
    let first = xs.next()?;
    let (lo, hi) = xs.fold((first, first), |(lo, hi), x| (lo.min(x), hi.max(x)));
    (hi > lo).then_some((lo, hi))
}

/// The time range `lo..hi` moved, not stretched, to lie within the take
/// `t0..t1`, or None when it holds the whole take: the whole session.
fn keep_in(lo: f64, hi: f64, (t0, t1): (f64, f64)) -> Option<(f64, f64)> {
    let w = hi - lo;
    if w >= t1 - t0 {
        None
    } else if lo < t0 {
        Some((t0, t0 + w))
    } else if hi > t1 {
        Some((t1 - w, t1))
    } else {
        Some((lo, hi))
    }
}

/// A line's value at `x` as the values box writes it: the point there
/// to the decimals of the ± of the reading it is, as its card showed it,
/// or with none the figure's `base` decimals (#510 K3, #1063).
pub(super) fn line_value(
    pts: &[[f64; 2]],
    se: fn(&TrendPoint) -> Option<f64>,
    (x, trend): (f64, &[TrendPoint]),
    base: usize,
    signed: bool,
) -> Option<String> {
    let [t, v] = pts[nearest_at(pts, x)?];
    let at = trend.partition_point(|p| p.t < t);
    let se = trend.get(at).and_then(se);
    Some(true_minus(&figure(v, se, base, signed)))
}

/// The values box for the pointer's time: its time and every line's
/// value there, each figure to the decimals the cards write it to
/// (`figure_text`, #1063): to its ±'s decimals, or with none its base
/// decimals. It hangs from the chart's top beside the pointer, so it
/// never covers the time labels along the bottom.
fn chart_hover(
    ui: &egui::Ui,
    frame: egui::Rect,
    (x, trend): (f64, &[TrendPoint]),
    lookup: &[ChartLine],
    (unit, base): (&str, usize),
    reading: SeriesKind,
    running: bool,
) {
    let Some(pointer) = ui.ctx().pointer_hover_pos() else {
        return;
    };
    let signed = reading == SeriesKind::Rate;
    egui::Tooltip::always_open(
        ui.ctx().clone(),
        ui.layer_id(),
        egui::Id::new(("chart values", steady::title(reading))),
        egui::PopupAnchor::Position(egui::pos2(pointer.x + 12.0, frame.top())),
    )
    .show(|ui| {
        ui.set_max_width(300.0);
        ui.label(RichText::new(length_text(x)).strong());
        for ChartLine {
            name, pts, key, se, ..
        } in lookup
        {
            ui.horizontal(|ui| {
                ui.label(RichText::new("■").color(*key));
                let value = line_value(pts, *se, (x, trend), base, signed);
                ui.label(format!(
                    "{name}: {}",
                    value.map_or("none".into(), |v| v + unit)
                ));
            });
        }
        ui.label(
            RichText::new(if running {
                "Scroll or drag pans time, Cmd or Ctrl and scroll zooms it, \
                 click shows that moment on the strip, \
                 double-click follows live again."
            } else {
                "Scroll or drag pans time, Cmd or Ctrl and scroll zooms it, \
                 click shows that moment on the strip, \
                 double-click shows the whole session."
            })
            .small()
            .weak(),
        );
    });
}
