//! The Complication pane (#1576, Hugh's spec, Peter's calls of 13 Nov
//! 2026): the guided test with a complication off, on, then off again in
//! one position, its table, and three plots on one time axis, Amplitude,
//! Rate, then Beat Error, each with the two Offs joined and its finding in
//! words. Every figure and word is core's
//! ([`timegrapher_core::complication`]); this lays them out.

use super::*;
use timegrapher_core::chart_scale;
use timegrapher_core::complication::{self, Change, Quantity};
use timegrapher_core::figure_text::true_minus;
use timegrapher_core::session::Measurement;

/// The table's width: the stretch's name and its three figures.
const TABLE_W: f32 = 400.0;
/// The least width of the plots.
const PLOTS_MIN_W: f32 = 360.0;
/// The pane width Info needs to sit beside the rest.
const INFO_BESIDE_W: f32 = 1180.0;
/// Each plot's least height, the track's over them, and the time axis's
/// under them, points.
const PLOT_MIN_H: f32 = 110.0;
const TRACK_H: f32 = 16.0;
const AXIS_H: f32 = 34.0;
/// A plot's margins: left for its numbers, top for its title and the
/// room its top number takes above its line.
const LEFT: f32 = 54.0;
const RIGHT: f32 = 14.0;
const TITLE_H: f32 = 30.0;
/// The room under a plot's lowest line, for its number, before the next
/// plot's title.
const FOOT_H: f32 = 14.0;
/// A point's radius, its ± bar's width, and the ring's radius.
const DOT_R: f32 = 4.2;
const BAR_W: f32 = 1.5;
const RING_R: f32 = 4.0;
/// The bracket's reach right of the On point, points.
const BRACKET: f32 = 8.0;
/// The Offs' dashed line: its dashes and gaps, points.
const DASH: f32 = 6.0;
const DASH_GAP: f32 = 4.0;
/// The time axis's steps to choose from, seconds, and how many marks it
/// wants at most.
const TIME_STEPS: [f32; 9] = [15.0, 30.0, 60.0, 120.0, 300.0, 600.0, 900.0, 1800.0, 3600.0];
const TIME_MARKS: f32 = 6.0;

/// One stretch on the plots: its middle, seconds from the test's start,
/// and its measurement.
struct Point<'a> {
    t: f64,
    m: &'a Measurement,
}

impl TimegrapherApp {
    /// The Complication Test, if that is the guided test on the Bench.
    pub(super) fn complication_test(&self) -> Option<&guided::Test> {
        self.guided
            .as_ref()
            .filter(|t| t.kind == guided::Kind::Complication)
    }

    /// The pane's toolbar: New Test or Cancel Test while none runs, the
    /// Settle and Each Stretch every guided test shares, and Copy.
    fn complication_settings(&mut self, ui: &mut egui::Ui) {
        let running = self.guided_running();
        if !running {
            if self.ready_test() == Some(NewTake::Complication) {
                if theme::tool_word_button(
                    ui,
                    Icon::Back,
                    "Cancel Test",
                    "Leave the test set up, with nothing started or kept",
                    false,
                    None,
                ) {
                    self.leave_ready_test();
                }
            } else if theme::tool_word_button(
                ui,
                Icon::Listen,
                "New Test",
                "Set a Complication Test up on a clear Bench, in the position the card shows, \
                 then press Start Test on the Listen card. It measures the watch with the \
                 complication off, on, then off again.",
                true,
                None,
            ) {
                self.new_take(NewTake::Complication, false);
            }
            theme::bar_gap(ui);
        }
        self.positions_lengths(ui, running, "Each Stretch");
        theme::bar_gap(ui);
        let nothing = self
            .complication_test()
            .is_none_or(|t| t.readings_so_far().is_empty())
            .then_some("Nothing measured yet");
        if theme::tool_word_button(
            ui,
            Icon::Copy,
            "Copy",
            "Copy the watch, its calibre, the table with tabs between the columns, and the \
             findings in words, to paste into a spreadsheet or a message",
            false,
            nothing,
        ) {
            let about = self.guided_about();
            if let Some(t) = self.complication_test() {
                ui.ctx().copy_text(guided::export_text(t, &about, '\t'));
                self.info_msg("Copied the Complication table".to_string());
            }
        }
    }

    /// The pane's toolbar, with the Info toggle pinned at its right.
    pub(super) fn complication_tools(&mut self, ui: &mut egui::Ui, title: &str) {
        let mut info = self.complication_info;
        let hint = "Info: how On less Off and its ± are worked out";
        theme::pane_toolbar_info(ui, title, &help::COMPLICATION, (&mut info, hint), |ui| {
            self.complication_settings(ui);
            self.export_button(ui, Pane::Complication);
        });
        self.complication_info = info;
    }

    /// The pane: the banner while the test runs, then the table and the
    /// three plots, and the Info panel.
    pub(super) fn complication_pane(&mut self, ui: &mut egui::Ui) {
        // Where Keep This Test? keeps clear of, as the other tests'.
        if self.complication_test().is_some() {
            self.results_rects[0] = Some(ui.max_rect());
        }
        let now = self.live.as_ref().map(|l| l.duration_s());
        let phase = match (self.guided.as_mut(), now) {
            (Some(t), Some(now)) if t.kind == guided::Kind::Complication => Some(t.tick(now)),
            _ => None,
        };
        let live = match phase {
            Some(guided::Phase::Measuring(_)) => self.guided_live(ui.ctx()),
            _ => None,
        };
        let lengths = (self.guided_settle_s, self.guided_measure_s);
        let action = complication_body(
            ui,
            (self.complication_test(), phase, live.as_ref()),
            (now, lengths),
            self.complication_info,
        );
        if let (Some(action), Some(now)) = (action, now) {
            self.apply_guided_action(action, now);
        }
    }
}

/// The pane's body under its toolbar; what a banner button asked for.
fn complication_body(
    ui: &mut egui::Ui,
    (test, phase, live): (
        Option<&guided::Test>,
        Option<guided::Phase>,
        Option<&guided::FreeCell>,
    ),
    (now, (settle_s, measure_s)): (Option<f64>, (f64, f64)),
    info: bool,
) -> Option<guided::Action> {
    let bottom = ui.available_rect_before_wrap().bottom();
    let layout = test_body::Layout {
        table_ws: &[TABLE_W],
        result_min_w: PLOTS_MIN_W,
        info_beside_w: INFO_BESIDE_W,
        rule_h: 3.0 * PLOT_MIN_H,
    };
    let plots = |ui: &mut egui::Ui| plots_block(ui, test, now, bottom);
    test_body::test_body(
        ui,
        ("complication pane", &layout),
        (test, phase, live, None),
        info,
        &test_body::Parts {
            about: about_text(settle_s, measure_s),
            result: &plots,
            info: &info_panel,
        },
    )
}

/// What the pane says before a test: how the test runs.
fn about_text(settle_s: f64, measure_s: f64) -> String {
    format!(
        "Start Test, on the Listen card, measures the watch in the position the card shows, \
         with its complication off, on, then off again:\n\
         1. It says when to turn the complication on or off.\n\
         2. It lets the watch settle {settle_s:.0} s.\n\
         3. It measures it for {measure_s:.0} s and fills in its row.\n\
         After the third, the test ends by itself, gives On less Off and keeps the take."
    )
}

/// The Info panel: how On less Off and its ± are worked out, in core's
/// words.
fn info_panel(ui: &mut egui::Ui) {
    let pal = theme::pal(ui);
    ui.spacing_mut().item_spacing.y = 4.0;
    ui.label(theme::caption("Complication").color(pal.text_secondary));
    ui.add(
        egui::Label::new(
            RichText::new(complication::method_note())
                .small()
                .color(pal.text),
        )
        .wrap()
        .selectable(true),
    );
}

/// The three plots on one time axis, with the track naming each stretch
/// over them and the key under them, in the room left above `bottom`.
fn plots_block(ui: &mut egui::Ui, test: Option<&guided::Test>, now: Option<f64>, bottom: f32) {
    let w = ui.available_width().max(PLOTS_MIN_W);
    let key_h = ui.text_style_height(&egui::TextStyle::Body) + 6.0;
    let room = bottom - ui.cursor().top() - TRACK_H - AXIS_H - key_h;
    let plot_h = (room / 3.0).max(PLOT_MIN_H);
    let h = TRACK_H + 3.0 * plot_h + AXIS_H;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(w, h), egui::Sense::hover());
    draw_plots(ui, rect, plot_h, test, now);
    plot_key(ui);
}

/// The time the plots span, seconds of the session: from the test's start
/// to now, or to the last stretch's end, and never under the three
/// stretches' own length.
fn time_span(test: &guided::Test, now: Option<f64>) -> (f64, f64) {
    let t0 = test.began_s().unwrap_or(0.0);
    let end = test
        .stretches()
        .iter()
        .map(|&(_, _, b)| b)
        .fold(now.unwrap_or(t0), f64::max);
    let least = complication::ORDER.len() as f64 * (test.settle_s + test.measure_s);
    (t0, end.max(t0 + least))
}

/// The time axis's step, seconds: the first of [`TIME_STEPS`] that gives
/// no more than [`TIME_MARKS`] marks over `span`.
fn time_step(span: f64) -> f64 {
    let most = f64::from(TIME_MARKS);
    TIME_STEPS
        .iter()
        .map(|&s| f64::from(s))
        .find(|s| span / s <= most)
        .unwrap_or_else(|| f64::from(TIME_STEPS[TIME_STEPS.len() - 1]))
}

/// Seconds as the axis writes them: "2:30".
fn min_s(t: f64) -> String {
    let s = t.round() as i64;
    format!("{}:{:02}", s / 60, s % 60)
}

/// The three plots, the track over them and the time axis under them.
fn draw_plots(
    ui: &egui::Ui,
    rect: egui::Rect,
    plot_h: f32,
    test: Option<&guided::Test>,
    now: Option<f64>,
) {
    let pal = theme::pal(ui);
    let painter = ui.painter_at(rect);
    let left = rect.left() + LEFT;
    let right = rect.right() - RIGHT;
    let (t0, t1) = test.map_or((0.0, 1.0), |t| time_span(t, now));
    let x = |t: f64| left + ((t - t0) / (t1 - t0)) as f32 * (right - left);
    if let Some(t) = test {
        draw_track(&painter, t, now, (rect.top(), &x), pal);
    }
    let readings = test.map_or(&[][..], |t| t.readings_so_far());
    let points: Vec<Point<'_>> = readings
        .iter()
        .map(|r| Point {
            t: complication::middle(&r.measurement),
            m: &r.measurement,
        })
        .collect();
    let measurements: Vec<Measurement> = readings.iter().map(|r| r.measurement.clone()).collect();
    let grid = egui::Stroke::new(1.0_f32, pal.separator);
    let step = time_step(t1 - t0);
    let plots_top = rect.top() + TRACK_H;
    let plots_bottom = plots_top + 3.0 * plot_h;
    let mut k = 0.0;
    let mut marks = Vec::new();
    while t0 + k * step <= t1 {
        let at = x(t0 + k * step);
        marks.push(at);
        painter.text(
            egui::pos2(at, plots_bottom + 4.0),
            egui::Align2::CENTER_TOP,
            min_s(k * step),
            egui::FontId::proportional(11.0),
            pal.text_secondary,
        );
        k += 1.0;
    }
    painter.text(
        egui::pos2(right, plots_bottom + AXIS_H),
        egui::Align2::RIGHT_BOTTOM,
        "Time, min:s",
        egui::FontId::proportional(11.0),
        pal.text_secondary,
    );
    for (i, q) in Quantity::ALL.into_iter().enumerate() {
        let top = plots_top + i as f32 * plot_h;
        let area = egui::Rect::from_min_max(
            egui::pos2(left, top + TITLE_H),
            egui::pos2(right, top + plot_h - FOOT_H),
        );
        for &at in &marks {
            painter.vline(at, area.y_range(), grid);
        }
        let change = complication::change(q, &measurements);
        plot_title(&painter, egui::pos2(rect.left(), top), q, change, pal);
        draw_one(&painter, area, (q, &points, change), &x, pal);
    }
}

/// The track over the plots: each stretch named, faint while it settles.
fn draw_track(
    painter: &egui::Painter,
    test: &guided::Test,
    now: Option<f64>,
    (top, x): (f32, &dyn Fn(f64) -> f32),
    pal: &theme::Palette,
) {
    let font = egui::FontId::proportional(11.0);
    let spans = test.measured(now.unwrap_or(0.0));
    for (i, &(from, to)) in spans.iter().enumerate() {
        let settle_from = from - test.settle_s;
        let band = |a: f64, b: f64, c: egui::Color32| {
            let r =
                egui::Rect::from_min_max(egui::pos2(x(a), top), egui::pos2(x(b), top + TRACK_H));
            painter.rect_filled(r, 2.0, c);
        };
        // Lighter while it settles (Hugh's spec).
        band(settle_from, from, pal.text_tertiary);
        band(from, to, pal.control);
        painter.text(
            egui::pos2(x(settle_from) + 4.0, top + TRACK_H / 2.0),
            egui::Align2::LEFT_CENTER,
            test.step_code(i),
            font.clone(),
            pal.text,
        );
    }
}

/// A plot's title: its name in bold, then core's finding once all three
/// stretches are in.
fn plot_title(
    painter: &egui::Painter,
    at: egui::Pos2,
    q: Quantity,
    change: Option<Change>,
    pal: &theme::Palette,
) {
    let name = painter.layout_no_wrap(q.name().to_string(), theme::semibold(13.0), pal.text);
    let w = name.size().x;
    painter.galley(at, name, pal.text);
    if let Some(c) = change {
        painter.text(
            at + Vec2::new(w, 0.0),
            egui::Align2::LEFT_TOP,
            format!(
                " · {}",
                true_minus(&complication::finding_text(q, c.on_less_off))
            ),
            egui::FontId::proportional(13.0),
            pal.text,
        );
    }
}

/// The figure's colour on the plots, as the readings take it everywhere.
fn ink(q: Quantity, pal: &theme::Palette) -> egui::Color32 {
    match q {
        Quantity::Amplitude => pal.trace_amplitude,
        Quantity::Rate => pal.trace_rate,
        Quantity::BeatError => pal.trace_beat_error,
    }
}

/// One plot: its scale's lines and numbers, each stretch at its middle
/// with its ±, the Offs joined dashed, and once all three are in the ring
/// where On would sit with the complication off and the bracket from it to
/// On.
fn draw_one(
    painter: &egui::Painter,
    area: egui::Rect,
    (q, points, change): (Quantity, &[Point<'_>], Option<Change>),
    x: &dyn Fn(f64) -> f32,
    pal: &theme::Palette,
) {
    let figs: Vec<(f64, complication::Figure)> = points
        .iter()
        .filter_map(|p| Some((p.t, q.figure(&complication::figures(p.m))?)))
        .collect();
    let mut values: Vec<f64> = figs
        .iter()
        .flat_map(|(_, f)| {
            let e = f.se.unwrap_or(0.0);
            [f.value - e, f.value + e]
        })
        .collect();
    values.extend(change.map(|c| c.off.value));
    let grid = egui::Stroke::new(1.0_f32, pal.separator);
    let Some((lo, hi)) = chart_scale::data_range(&values) else {
        painter.hline(area.x_range(), area.bottom(), grid);
        return;
    };
    let (lo, hi, step) = chart_scale::round_ends(lo, hi);
    let y = |v: f64| area.bottom() - ((v - lo) / (hi - lo)) as f32 * area.height();
    let n = ((hi - lo) / step).round() as i64;
    for k in 0..=n {
        let v = lo + k as f64 * step;
        painter.hline(area.x_range(), y(v), grid);
        let mut label = true_minus(&chart_scale::axis_number(v, step));
        if q.signed() && v > 0.0 {
            label = format!("+{label}");
        }
        painter.text(
            egui::pos2(area.left() - 6.0, y(v)),
            egui::Align2::RIGHT_CENTER,
            format!("{label}{}", unit_after(q)),
            egui::FontId::proportional(11.0),
            pal.text_secondary,
        );
    }
    let colour = ink(q, pal);
    let quiet = egui::Stroke::new(1.0_f32, pal.text_secondary);
    // The Offs joined, under the points.
    if let [(ta, a), _, (tc, c)] = figs[..] {
        painter.extend(egui::Shape::dashed_line(
            &[egui::pos2(x(ta), y(a.value)), egui::pos2(x(tc), y(c.value))],
            quiet,
            DASH,
            DASH_GAP,
        ));
    }
    if let (Some(c), Some(&(t_on, on))) = (change, figs.get(1)) {
        let ring = egui::pos2(x(t_on), y(c.off.value));
        painter.circle(ring, RING_R, pal.card, quiet);
        let (a, b) = (y(c.off.value), y(on.value));
        let r = x(t_on) + BRACKET;
        painter.line_segment([egui::pos2(r - 3.0, a), egui::pos2(r, a)], quiet);
        painter.line_segment([egui::pos2(r, a), egui::pos2(r, b)], quiet);
        painter.line_segment([egui::pos2(r - 3.0, b), egui::pos2(r, b)], quiet);
    }
    for &(t, f) in &figs {
        let p = egui::pos2(x(t), y(f.value));
        if let Some(e) = f.se {
            let (a, b) = (
                egui::pos2(p.x, y(f.value + e)),
                egui::pos2(p.x, y(f.value - e)),
            );
            painter.line_segment([a, b], egui::Stroke::new(BAR_W, colour));
        }
        painter.circle(p, DOT_R, colour, egui::Stroke::new(1.5_f32, pal.card));
    }
}

/// The unit after an axis number: the degree sign close up, others spaced.
fn unit_after(q: Quantity) -> &'static str {
    match q {
        Quantity::Amplitude => "°",
        Quantity::Rate => " s/d",
        Quantity::BeatError => " ms",
    }
}

/// The key under the plots, for all three.
fn plot_key(ui: &mut egui::Ui) {
    let pal = theme::pal(ui);
    ui.horizontal_wrapped(|ui| {
        ui.add_space(LEFT);
        theme::dot(ui, pal.text_secondary);
        ui.label(
            RichText::new("Each stretch, with its ±")
                .small()
                .color(pal.text_secondary),
        );
        ui.add_space(12.0);
        theme::dash(ui, pal.text_secondary, egui::Color32::TRANSPARENT);
        ui.label(
            RichText::new("Offs joined")
                .small()
                .color(pal.text_secondary),
        );
        ui.add_space(12.0);
        let (r, _) = ui.allocate_exact_size(Vec2::splat(2.0 * RING_R + 2.0), egui::Sense::hover());
        ui.painter().circle(
            r.center(),
            RING_R,
            pal.card,
            egui::Stroke::new(1.0_f32, pal.text_secondary),
        );
        ui.label(
            RichText::new("Where On would sit with it off")
                .small()
                .color(pal.text_secondary),
        );
    });
}
