//! The Dynamic Poise pane (#1474, Hugh's spec of 12 Nov): the guided test
//! through the eight upright stops, its table, and the rate by the hour
//! at the top drawn as a plot or round a dial, with core's words on the
//! wave and the heavy spot over it. Every figure and word is core's
//! ([`timegrapher_core::poise`]); this lays them out.

use super::*;
use timegrapher_core::chart_scale;
use timegrapher_core::poise::{self, PoiseFit};

/// How the pane draws the rate by the hour at the top: along a line, or
/// round a watch face. Kept between runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum PoiseView {
    #[default]
    Plot,
    Dial,
}

/// The picture's height, points, and the width below which it goes over
/// the table rather than beside it (Hugh, #1499).
const PICTURE_H: f32 = 300.0;
const PICTURE_MIN_W: f32 = 320.0;
/// The plot's height to its width at most, as in Hugh's spec.
const PLOT_ASPECT: f32 = 0.5;
/// The gap between the table and the picture.
const GAP: f32 = 28.0;
/// The Info panel's width beside the rest, and the pane width it needs.
const INFO_W: f32 = 300.0;
const INFO_BESIDE_W: f32 = 1100.0;
/// A point's radius, and its ± bar's width.
const DOT_R: f32 = 4.2;
const BAR_W: f32 = 1.5;
/// The picture's margins: left for the rate's numbers, bottom for the
/// hours and their name, top for the fastest hour's label.
const LEFT: f32 = 46.0;
const RIGHT: f32 = 14.0;
const TOP: f32 = 30.0;
const BOTTOM: f32 = 36.0;
/// The drawn wave's step, points along the plot's width.
const CURVE_PX: f32 = 4.0;
/// The steps the drawn wave is cut into round the dial.
const DIAL_STEPS: f32 = 120.0;
/// How far a line leans, as a share of its length across, before a
/// label at its end is set off that way rather than centred.
const LEAN: f32 = 0.3;
/// The key's line length, points, and the room round a backed label.
const KEY_LINE: f32 = 14.0;
const BACKING: f32 = 2.0;
/// A small label's height, points: a ring's rate this near the mean's
/// name is left out.
const LABEL_H: f32 = 14.0;

/// The upright stops round the face, 45° apart, and the step between.
fn hours() -> (usize, f64) {
    let n = core_session::Position::UPRIGHT.len();
    (n, 360.0 / n as f64)
}

/// The bearing halfway from the stop at `deg` to the next, degrees from
/// 12H: where the dial's ring numbers (by 12H) and the mean ring's name
/// (by 6H) sit, so no point sits on them (Hugh, #1499).
fn between(deg: f64) -> f64 {
    deg + hours().1 / 2.0
}

/// What the picture draws: each position measured (angle, rate, ±) and
/// core's fit once there is one.
struct Picture<'a> {
    points: Vec<(f64, f64, Option<f64>)>,
    fit: Option<&'a PoiseFit>,
    /// The table's Mean and its name, drawn with the wave.
    mean: Option<(f64, String)>,
}

impl Picture<'_> {
    /// The rates the picture must hold: each point with its ±, and the
    /// wave.
    fn range(&self) -> Option<(f64, f64)> {
        let mut v: Vec<f64> = Vec::new();
        for &(_, r, se) in &self.points {
            let e = se.unwrap_or(0.0);
            v.extend([r - e, r + e]);
        }
        if let Some(f) = self.fit {
            v.extend(f.curve(DIAL_STEPS as usize).into_iter().map(|(_, r)| r));
        }
        chart_scale::data_range(&v)
    }
}

impl TimegrapherApp {
    /// The Dynamic Poise test, if that is the guided test on the Bench.
    pub(super) fn poise_test(&self) -> Option<&guided::Test> {
        self.guided
            .as_ref()
            .filter(|t| t.kind == guided::Kind::Poise)
    }

    /// The pane's toolbar: View first (Hugh's spec), New Test or Cancel
    /// Test while none runs, the Settle and Each Position the Positions
    /// Test shares, and Copy.
    pub(super) fn poise_settings(&mut self, ui: &mut egui::Ui) {
        theme::keep_next(ui);
        theme::tool_field(
            ui,
            "View",
            "The rate by the hour at the top, along a line or round a watch face: the same \
             points, wave and words",
            |ui| {
                theme::segmented(
                    ui,
                    &mut self.poise_view,
                    &[
                        (
                            PoiseView::Plot,
                            "Plot",
                            "Rate along a line, the hour at the top across it",
                        ),
                        (
                            PoiseView::Dial,
                            "Dial",
                            "Rate round a watch face, 12H at the top, further out when faster",
                        ),
                    ],
                );
            },
        );
        theme::bar_gap(ui);
        let running = self.guided_running();
        if !running {
            if self.ready_test() == Some(NewTake::Poise) {
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
                "Set a Dynamic Poise Test up on a clear Bench, then press Start Test on the \
                 Listen card. It begins at 9H at the top and turns the watch 45° at a time \
                 through the eight upright positions.",
                true,
                None,
            ) {
                self.new_take(NewTake::Poise, false);
            }
            theme::bar_gap(ui);
        }
        self.positions_lengths(ui, running);
        theme::bar_gap(ui);
        let nothing = self
            .poise_test()
            .is_none_or(|t| t.readings_so_far().is_empty())
            .then_some("Nothing measured yet");
        if theme::tool_word_button(
            ui,
            Icon::Copy,
            "Copy",
            "Copy the watch, its calibre, the table with tabs between the columns, and the \
             fit in words, to paste into a spreadsheet or a message",
            false,
            nothing,
        ) {
            let about = self.guided_about();
            if let Some(t) = self.poise_test() {
                ui.ctx().copy_text(guided::export_text(t, &about, '\t'));
                self.info_msg("Copied the Dynamic Poise table".to_string());
            }
        }
    }

    /// The pane's toolbar, with the Info toggle pinned at its right.
    pub(super) fn poise_tools(&mut self, ui: &mut egui::Ui, title: &str) {
        let mut info = self.poise_info;
        let mut pin = |ui: &mut egui::Ui| {
            let accent = theme::pal(ui).accent;
            if theme::icon_toggle(ui, Icon::Info, info, accent)
                .on_hover_text(
                    "Info: the fit in full, the twice-round check, and why near 220° the \
                     heavy spot is not stated",
                )
                .clicked()
            {
                info = !info;
            }
        };
        theme::pane_toolbar_pinned(ui, title, &help::DYNAMIC_POISE, Some(&mut pin), |ui| {
            self.poise_settings(ui);
            self.export_button(ui, Pane::DynamicPoise);
        });
        self.poise_info = info;
    }

    /// The pane: the banner while the test runs, then the table and the
    /// picture with core's words over it, and the Info panel.
    pub(super) fn poise_pane(&mut self, ui: &mut egui::Ui) {
        // Where Keep This Test? keeps clear of, as the Positions Test's.
        if self.poise_test().is_some() {
            self.results_rects[0] = Some(ui.max_rect());
        }
        let now = self.live.as_ref().map(|l| l.duration_s());
        let phase = match (self.guided.as_mut(), now) {
            (Some(t), Some(now)) if t.kind == guided::Kind::Poise => Some(t.tick(now)),
            _ => None,
        };
        let live = match phase {
            Some(guided::Phase::Measuring(_)) => self.guided_live(ui.ctx()),
            _ => None,
        };
        // Before the first stop is in, the amplitude so far, so a watch
        // near 220° can be let down before the run (Hugh's spec).
        let amp_now = self
            .poise_test()
            .filter(|t| t.step == 0 && t.readings_so_far().is_empty())
            .and_then(|_| self.card_reading())
            .filter(|r| readouts::heard(Some(r)))
            .and_then(|r| {
                Some(poise::amplitude_now_text(
                    r.amplitude_deg?,
                    r.amplitude_error_deg,
                ))
            });
        let info = self.poise_info;
        let view = self.poise_view;
        let settle = (self.guided_settle_s, self.guided_measure_s);
        let action = poise_body(
            ui,
            (self.poise_test(), phase, live.as_ref()),
            amp_now.as_deref(),
            (view, info),
            settle,
        );
        let (Some(action), Some(now)) = (action, now) else {
            return;
        };
        let Some(mut test) = self.guided.take() else {
            return;
        };
        match action {
            guided::Action::Next => {
                if let Some(p) = test.advance(now) {
                    self.change_position(p);
                }
            }
            guided::Action::Redo => test.redo(now),
            guided::Action::Pause => test.pause(now),
            guided::Action::Resume => test.resume(now),
            guided::Action::ClearEarlier => {}
        }
        self.guided_unused = test.unused(now);
        self.guided_measured = test.measured(now);
        self.guided = Some(test);
        self.mask_engine();
    }
}

/// The pane's body under its toolbar; what a banner button asked for.
fn poise_body(
    ui: &mut egui::Ui,
    (test, phase, live): (
        Option<&guided::Test>,
        Option<guided::Phase>,
        Option<&guided::FreeCell>,
    ),
    amp_now: Option<&str>,
    (view, info): (PoiseView, bool),
    (settle_s, measure_s): (f64, f64),
) -> Option<guided::Action> {
    let pal = theme::pal(ui);
    let mut action = None;
    let readings = test.map_or(&[][..], |t| t.readings_so_far());
    let fit = guided::poise_fit(readings);
    let mean = guided::poise_mean(readings);
    let full = ui.available_rect_before_wrap();
    let beside = info && full.width() >= INFO_BESIDE_W;
    let main_w = if beside {
        // Less the spacing round the two gaps, so Info ends inside the pane.
        full.width() - INFO_W - GAP - 3.0 * ui.spacing().item_spacing.x
    } else {
        full.width()
    };
    egui::ScrollArea::both()
        .id_salt("poise pane")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 6.0;
            ui.horizontal_top(|ui| {
                ui.vertical(|ui| {
                    ui.set_width(main_w);
                    if let (Some(t), Some(ph)) = (test, phase) {
                        if ph != guided::Phase::Finished {
                            action = guided::banner(ui, t, ph, amp_now);
                            ui.add_space(6.0);
                        }
                    }
                    // Beside the picture at the table's widths, or tight, while
                    // the picture keeps its least width; else under it.
                    let side = [guided::TABLE_W, guided::TIGHT_TABLE_W]
                        .into_iter()
                        .find(|w| main_w >= w + GAP + PICTURE_MIN_W);
                    let table = |ui: &mut egui::Ui| match test {
                        Some(t) => guided::test_table(ui, t, phase, live),
                        None => {
                            ui.label(
                                RichText::new(about_text(settle_s, measure_s))
                                    .color(pal.text_secondary),
                            );
                        }
                    };
                    let picture = |ui: &mut egui::Ui| {
                        picture_block(ui, (readings, fit.as_ref(), &mean), view, full.bottom());
                    };
                    if let Some(table_w) = side {
                        ui.horizontal_top(|ui| {
                            ui.vertical(|ui| {
                                ui.set_width(table_w);
                                table(ui);
                            });
                            ui.add_space(GAP);
                            ui.vertical(|ui| {
                                picture(ui);
                            });
                        });
                    } else {
                        // The result first (Hugh, #1499).
                        picture(ui);
                        ui.add_space(8.0);
                        table(ui);
                    }
                    if info && !beside {
                        ui.add_space(8.0);
                        info_panel(ui, fit.as_ref());
                    }
                });
                if beside {
                    ui.add_space(GAP / 2.0);
                    let top = ui.cursor().top();
                    ui.painter().vline(
                        ui.cursor().left(),
                        egui::Rangef::new(top, top + PICTURE_H),
                        egui::Stroke::new(1.0_f32, pal.separator),
                    );
                    ui.add_space(GAP / 2.0);
                    ui.vertical(|ui| {
                        ui.set_width(INFO_W);
                        info_panel(ui, fit.as_ref());
                    });
                }
            });
        });
    action
}

/// The picture with core's words over it, in the room left above `bottom`.
fn picture_block(
    ui: &mut egui::Ui,
    (readings, fit, mean): (
        &[timegrapher_core::session::Reading],
        Option<&PoiseFit>,
        &Option<(f64, String)>,
    ),
    view: PoiseView,
    bottom: f32,
) {
    words(ui, fit);
    let points = readings
        .iter()
        .filter_map(|r| poise::point(r.position, &r.measurement))
        .map(|p| (p.angle_deg, p.rate_s_per_day, p.rate_se))
        .collect();
    let pic = Picture {
        points,
        fit,
        mean: mean.clone(),
    };
    let w = ui.available_width().max(PICTURE_MIN_W);
    // The room left in the pane, never under PICTURE_H; the dial never
    // taller than wide, the plot half as tall.
    let tallest = match view {
        PoiseView::Plot => w * PLOT_ASPECT,
        PoiseView::Dial => w,
    };
    let h = (bottom - ui.cursor().top()).min(tallest).max(PICTURE_H);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(w, h), egui::Sense::hover());
    match view {
        PoiseView::Plot => draw_plot(ui, rect, &pic),
        PoiseView::Dial => draw_dial(ui, rect, &pic),
    }
}

/// What the pane says before a test: how the test runs.
fn about_text(settle_s: f64, measure_s: f64) -> String {
    format!(
        "Start Test, on the Listen card, leads you through the eight upright positions, \
         45° apart, from 9H at the top, turning the watch the same way round each time:\n\
         1. It says which hour to turn to the top.\n\
         2. It lets the watch settle {settle_s:.0} s.\n\
         3. It measures it for {measure_s:.0} s and fills in its row.\n\
         The wave is fitted once {} positions are in. After the eighth the test ends by \
         itself and keeps the take.",
        poise::MIN_SHOWN
    )
}

/// The title line over the picture (core's short sentence, measured
/// facts) and the line under it (the heavy spot, or a grey chip where
/// the amplitude is near 220°), or core's words for a fit still to come.
/// The room is kept from the start, so nothing moves when they come.
fn words(ui: &mut egui::Ui, fit: Option<&PoiseFit>) {
    let pal = theme::pal(ui);
    let line_h = ui.text_style_height(&egui::TextStyle::Body);
    ui.allocate_ui(Vec2::new(ui.available_width(), 2.0 * line_h + 8.0), |ui| {
        ui.spacing_mut().item_spacing.y = 3.0;
        let Some(f) = fit else {
            ui.label(RichText::new(poise::pending_text()).color(pal.text_secondary));
            return;
        };
        ui.add(egui::Label::new(RichText::new(&f.short_text).color(pal.text)).wrap());
        let band = matches!(f.withheld, Some(poise::Withheld::NullBand { .. }));
        if band {
            // The chip holds the band's fact; its caveat follows, plain.
            egui::Frame::new()
                .fill(pal.control)
                .corner_radius(egui::CornerRadius::same(10))
                .inner_margin(egui::Margin::symmetric(10, 2))
                .show(ui, |ui| {
                    ui.add(
                        egui::Label::new(
                            RichText::new(f.heavy_spot_line()).small().color(pal.text),
                        )
                        .wrap(),
                    )
                    .on_hover_text(poise::null_band_note());
                });
            let caveat = f.caveat_line();
            if !caveat.is_empty() {
                ui.add(egui::Label::new(RichText::new(caveat).color(pal.text_secondary)).wrap());
            }
        } else {
            ui.add(egui::Label::new(RichText::new(&f.heavy_text).color(pal.text_secondary)).wrap());
        }
    });
}

/// The Info panel: core's full sentence (the wave, its twice-round check,
/// the amplitude and the heavy spot), and the note on the band near 220°,
/// which alone says why 219.5° matters.
fn info_panel(ui: &mut egui::Ui, fit: Option<&PoiseFit>) {
    let pal = theme::pal(ui);
    ui.spacing_mut().item_spacing.y = 4.0;
    let text = |t: &str, c: egui::Color32| {
        egui::Label::new(RichText::new(t).small().color(c))
            .wrap()
            .selectable(true)
    };
    ui.label(theme::caption("Dynamic Poise").color(pal.text_secondary));
    match fit {
        Some(f) => ui.add(text(&f.sentence_beside_note(), pal.text)),
        None => ui.add(text(&poise::pending_text(), pal.text_secondary)),
    };
    ui.add_space(6.0);
    ui.add(text(&poise::null_band_note(), pal.text_secondary));
}

/// The rate scale for the picture: round ends round every rate it holds.
fn rate_scale(pic: &Picture<'_>) -> (f64, f64, f64) {
    let (lo, hi) = pic.range().unwrap_or((-1.0, 1.0));
    chart_scale::round_ends(lo, hi)
}

/// The plot: x the hour at the top, 12H to 12H, a mark every 45°; y the
/// rate. The points with their ±, then the wave and its fastest hour in
/// grey once core has fitted it.
fn draw_plot(ui: &egui::Ui, rect: egui::Rect, pic: &Picture<'_>) {
    let pal = theme::pal(ui);
    let painter = ui.painter_at(rect);
    let area = egui::Rect::from_min_max(
        rect.min + Vec2::new(LEFT, TOP),
        rect.max - Vec2::new(RIGHT, BOTTOM),
    );
    let (lo, hi, step) = rate_scale(pic);
    let x = |deg: f64| area.left() + (deg / 360.0) as f32 * area.width();
    let y = |r: f64| area.bottom() - ((r - lo) / (hi - lo)) as f32 * area.height();
    let small = egui::FontId::proportional(11.0);
    let grid = egui::Stroke::new(1.0_f32, pal.separator);
    let n = ((hi - lo) / step).round() as i64;
    for k in 0..=n {
        let v = lo + k as f64 * step;
        painter.hline(area.x_range(), y(v), grid);
        // No rate's numbers until there is a rate.
        if pic.points.is_empty() {
            continue;
        }
        painter.text(
            egui::pos2(area.left() - 6.0, y(v)),
            egui::Align2::RIGHT_CENTER,
            chart_scale::axis_number(v, step),
            small.clone(),
            pal.text_secondary,
        );
    }
    painter.text(
        egui::pos2(area.left() - 6.0, area.top() - 10.0),
        egui::Align2::RIGHT_BOTTOM,
        "s/d",
        small.clone(),
        pal.text_secondary,
    );
    let (n_hours, hour_step) = hours();
    for i in 0..=n_hours {
        let deg = i as f64 * hour_step;
        let half = i % 2 == 1;
        let ink = if half {
            pal.text_tertiary
        } else {
            pal.text_secondary
        };
        let top = egui::pos2(x(deg), area.top());
        let bottom = egui::pos2(x(deg), area.bottom());
        if half {
            painter.extend(egui::Shape::dashed_line(&[top, bottom], grid, 2.0, 3.0));
        } else {
            painter.line_segment([top, bottom], grid);
        }
        painter.text(
            egui::pos2(x(deg), area.bottom() + 4.0),
            egui::Align2::CENTER_TOP,
            poise::hour_code(deg),
            small.clone(),
            ink,
        );
    }
    painter.text(
        egui::pos2(area.center().x, rect.bottom() - 2.0),
        egui::Align2::CENTER_BOTTOM,
        "Hour at the top",
        small.clone(),
        pal.text_secondary,
    );
    if let Some(f) = pic.fit {
        plot_wave(&painter, area, (&x, &y), f, pal);
        if let Some((m, name)) = &pic.mean {
            let dotted = egui::Stroke::new(1.0_f32, pal.text_secondary);
            let line = [
                egui::pos2(area.left(), y(*m)),
                egui::pos2(area.right(), y(*m)),
            ];
            painter.extend(egui::Shape::dashed_line(&line, dotted, 1.0, 3.0));
            backed(
                &painter,
                // Above the line at the slowest hour, where the wave is
                // below it (Hugh, #1499).
                egui::pos2(x(f.slowest().0), y(*m) - 3.0),
                egui::Align2::CENTER_BOTTOM,
                name,
                pal,
            );
        }
    }
    plot_key(&painter, area, pic, pal);
    // 12H is at both ends: its point is drawn at each.
    for &(deg, r, se) in &pic.points {
        let ends: &[f64] = if deg.rem_euclid(360.0) == 0.0 {
            &[0.0, 360.0]
        } else {
            &[deg]
        };
        for &d in ends {
            point(
                &painter,
                egui::pos2(x(d), y(r)),
                se.map(|e| y(r - e) - y(r)),
                pal,
            );
        }
    }
}

/// The plot's key, inside its top right: the points, and the wave once
/// it is fitted.
fn plot_key(painter: &egui::Painter, area: egui::Rect, pic: &Picture<'_>, pal: &theme::Palette) {
    if pic.points.is_empty() {
        return;
    }
    let small = egui::FontId::proportional(11.0);
    let row = 16.0;
    let mut rows = vec![format!("Upright positions ({})", pic.points.len())];
    if pic.fit.is_some() {
        rows.push("Fitted wave".to_string());
    }
    let widest = rows
        .iter()
        .map(|t| {
            painter
                .layout_no_wrap(t.clone(), small.clone(), pal.text)
                .size()
                .x
        })
        .fold(0.0_f32, f32::max);
    let w = KEY_LINE + 6.0 + widest;
    let top_left = egui::pos2(area.right() - w - 8.0, area.top() + 6.0);
    let back =
        egui::Rect::from_min_size(top_left, Vec2::new(w, row * rows.len() as f32)).expand(4.0);
    painter.rect_filled(back, 4.0, pal.card);
    for (i, t) in rows.iter().enumerate() {
        let mid = top_left.y + row * (i as f32 + 0.5);
        let at = |dx: f32| egui::pos2(top_left.x + dx, mid);
        if i == 0 {
            painter.circle_filled(at(KEY_LINE / 2.0), DOT_R, pal.trace_rate);
        } else {
            painter.line_segment(
                [at(0.0), at(KEY_LINE)],
                egui::Stroke::new(1.5_f32, pal.text_secondary),
            );
        }
        painter.text(
            at(KEY_LINE + 6.0),
            egui::Align2::LEFT_CENTER,
            t,
            small.clone(),
            pal.text,
        );
    }
}

/// Small text on the pane's background, so no line crosses it.
fn backed(
    painter: &egui::Painter,
    pos: egui::Pos2,
    align: egui::Align2,
    text: &str,
    pal: &theme::Palette,
) {
    let galley = painter.layout_no_wrap(
        text.to_string(),
        egui::FontId::proportional(11.0),
        pal.text_secondary,
    );
    let rect = align.anchor_size(pos, galley.size());
    painter.rect_filled(rect.expand(BACKING), BACKING, pal.card);
    painter.galley(rect.min, galley, pal.text_secondary);
}

/// Core's fit on the plot: the wave in grey, and its fastest hour as an
/// upright line from the top, named, once the wave is clear of the noise.
fn plot_wave(
    painter: &egui::Painter,
    area: egui::Rect,
    (x, y): (&dyn Fn(f64) -> f32, &dyn Fn(f64) -> f32),
    f: &PoiseFit,
    pal: &theme::Palette,
) {
    let small = egui::FontId::proportional(11.0);
    let wave: Vec<egui::Pos2> = f
        .curve((area.width() / CURVE_PX) as usize)
        .into_iter()
        .map(|(d, r)| egui::pos2(x(d), y(r)))
        .collect();
    let grey = egui::Stroke::new(1.5_f32, pal.text_secondary);
    painter.add(egui::Shape::line(wave, grey));
    if f.wave_clear {
        let (peak, top) = f.fastest();
        painter.line_segment(
            [egui::pos2(x(peak), area.top()), egui::pos2(x(peak), y(top))],
            egui::Stroke::new(1.2_f32, pal.text_secondary),
        );
        painter.text(
            egui::pos2(x(peak), area.top() - 2.0),
            egui::Align2::CENTER_BOTTOM,
            format!("Fastest {}", poise::hour_code(peak)),
            small.clone(),
            pal.text,
        );
    }
}

/// Where the dial puts a rate at an hour: the rate scale's ends at the
/// `inner` and `outer` rings round `centre`.
struct Dial {
    centre: egui::Pos2,
    inner: f32,
    outer: f32,
    scale: (f64, f64, f64),
}

impl Dial {
    fn radius(&self, r: f64) -> f32 {
        let (lo, hi, _) = self.scale;
        self.inner + ((r - lo) / (hi - lo)) as f32 * (self.outer - self.inner)
    }

    /// The point `rad` out from the centre at `deg` round from 12H.
    fn at(&self, deg: f64, rad: f32) -> egui::Pos2 {
        let a = deg.to_radians() as f32;
        self.centre + Vec2::new(a.sin(), -a.cos()) * rad
    }
}

/// The dial: 12H at the top, each point at its hour, further out when
/// faster; the mean as a dotted ring. The zero ring is not drawn, so the
/// sign reads from the rings' numbers.
fn draw_dial(ui: &egui::Ui, rect: egui::Rect, pic: &Picture<'_>) {
    let pal = theme::pal(ui);
    let painter = ui.painter_at(rect);
    let small = egui::FontId::proportional(11.0);
    let label_room = 30.0;
    let outer = (rect.height() / 2.0 - label_room).min(rect.width() / 2.0 - label_room - 20.0);
    let dial = Dial {
        centre: rect.center(),
        inner: outer * 0.15,
        outer,
        scale: rate_scale(pic),
    };
    // The slowest point's bearing once there is a wave; till then, between
    // two stops.
    let numbers = (!pic.points.is_empty()).then(|| {
        let bearing = pic.fit.map_or(between(0.0), |f| f.slowest().0);
        let mean_r = pic.fit.and(pic.mean.as_ref()).map(|m| dial.radius(m.0));
        (bearing, mean_r)
    });
    dial_grid(&painter, &dial, pal, numbers);
    painter.text(
        rect.right_top() + Vec2::new(-4.0, 4.0),
        egui::Align2::RIGHT_TOP,
        "Further out = faster, s/d",
        small,
        pal.text_secondary,
    );
    if let Some(f) = pic.fit {
        dial_wave(&painter, &dial, (f, &pic.mean), pal);
    }
    for &(deg, r, se) in &pic.points {
        let p = dial.at(deg, dial.radius(r));
        // The ± along the spoke, out and in.
        if let Some(e) = se {
            let (a, b) = (
                dial.at(deg, dial.radius(r + e)),
                dial.at(deg, dial.radius(r - e)),
            );
            painter.line_segment([a, b], egui::Stroke::new(BAR_W, pal.trace_rate));
        }
        painter.circle(
            p,
            DOT_R,
            pal.trace_rate,
            egui::Stroke::new(1.5_f32, pal.card),
        );
    }
}

/// The dial's rings at the scale's round rates, with their numbers, and
/// a spoke and name for each upright hour.
/// `numbers`: the bearing the rings' rates go on, and the mean ring's
/// radius, whose name goes there too, so a rate that would sit on it is
/// left out.
fn dial_grid(
    painter: &egui::Painter,
    dial: &Dial,
    pal: &theme::Palette,
    numbers: Option<(f64, Option<f32>)>,
) {
    let small = egui::FontId::proportional(11.0);
    let grid = egui::Stroke::new(1.0_f32, pal.separator);
    let (lo, hi, step) = dial.scale;
    let n = ((hi - lo) / step).round() as i64;
    for k in 0..=n {
        let v = lo + k as f64 * step;
        let r = dial.radius(v);
        painter.circle_stroke(dial.centre, r, grid);
        let Some((bearing, mean_r)) = numbers else {
            continue;
        };
        if mean_r.is_some_and(|m| (m - r).abs() < LABEL_H) {
            continue;
        }
        // Each ring's rate on the slowest point's bearing, inside the
        // wave, on the pane's background (Hugh, #1499).
        backed(
            painter,
            dial.at(bearing, r),
            egui::Align2::CENTER_CENTER,
            &chart_scale::axis_number(v, step),
            pal,
        );
    }
    let (n_hours, hour_step) = hours();
    for i in 0..n_hours {
        let deg = i as f64 * hour_step;
        let half = i % 2 == 1;
        let ink = if half {
            pal.text_tertiary
        } else {
            pal.text_secondary
        };
        let spoke = [dial.centre, dial.at(deg, dial.outer)];
        if half {
            painter.extend(egui::Shape::dashed_line(&spoke, grid, 2.0, 3.0));
        } else {
            painter.line_segment(spoke, grid);
        }
        painter.text(
            dial.at(deg, dial.outer + 14.0),
            egui::Align2::CENTER_CENTER,
            poise::hour_code(deg),
            small.clone(),
            ink,
        );
    }
}

/// Core's fit round the dial: the mean as a dotted ring, the wave in
/// grey, and its fastest hour as a line from the centre, named, once the
/// wave is clear of the noise.
fn dial_wave(
    painter: &egui::Painter,
    dial: &Dial,
    (f, mean): (&PoiseFit, &Option<(f64, String)>),
    pal: &theme::Palette,
) {
    let steps = DIAL_STEPS as usize;
    let ring_r = dial.radius(mean.as_ref().map_or(f.wave.mean.value, |m| m.0));
    let ring: Vec<egui::Pos2> = (0..=steps)
        .map(|i| dial.at(360.0 * i as f64 / f64::from(DIAL_STEPS), ring_r))
        .collect();
    let dotted = egui::Stroke::new(1.0_f32, pal.text_secondary);
    painter.extend(egui::Shape::dashed_line(&ring, dotted, 1.0, 3.0));
    let wave: Vec<egui::Pos2> = f
        .curve(steps)
        .into_iter()
        .map(|(d, r)| dial.at(d, dial.radius(r)))
        .collect();
    painter.add(egui::Shape::line(
        wave,
        egui::Stroke::new(1.5_f32, pal.text_secondary),
    ));
    if let Some((m, name)) = mean {
        backed(
            painter,
            dial.at(f.slowest().0, dial.radius(*m)),
            egui::Align2::CENTER_CENTER,
            name,
            pal,
        );
    }
    if !f.wave_clear {
        return;
    }
    let (peak, top) = f.fastest();
    let tip = dial.at(peak, dial.radius(top));
    painter.line_segment(
        [dial.centre, tip],
        egui::Stroke::new(1.2_f32, pal.text_secondary),
    );
    // Named at the line's outer end, just beyond the wave (Hugh, #1499),
    // set off the way the line points.
    let out = (tip - dial.centre).normalized();
    let align = egui::Align2([
        lean(out.x, egui::Align::Min, egui::Align::Max),
        lean(out.y, egui::Align::Min, egui::Align::Max),
    ]);
    let galley = painter.layout_no_wrap(
        format!("Fastest {}", poise::hour_code(peak)),
        egui::FontId::proportional(11.0),
        pal.text,
    );
    let rect = align.anchor_size(tip + out * 8.0, galley.size());
    painter.rect_filled(rect.expand(BACKING), BACKING, pal.card);
    painter.galley(rect.min, galley, pal.text);
}

/// Which way a label leans off a line pointing `d` along one axis: past
/// its end when the line leans that way, centred when it barely does.
fn lean(d: f32, plus: egui::Align, minus: egui::Align) -> egui::Align {
    if d > LEAN {
        plus
    } else if d < -LEAN {
        minus
    } else {
        egui::Align::Center
    }
}

/// A measured point in the rate's colour, with its ± (`half`, points
/// from the dot to the bar's end) as a bar through it.
fn point(painter: &egui::Painter, p: egui::Pos2, half: Option<f32>, pal: &theme::Palette) {
    if let Some(h) = half {
        painter.line_segment(
            [p - Vec2::new(0.0, h), p + Vec2::new(0.0, h)],
            egui::Stroke::new(BAR_W, pal.trace_rate),
        );
    }
    painter.circle(
        p,
        DOT_R,
        pal.trace_rate,
        egui::Stroke::new(1.5_f32, pal.card),
    );
}
