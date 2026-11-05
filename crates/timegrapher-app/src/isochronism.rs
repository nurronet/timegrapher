//! The Isochronism pane: how the rate moves with the amplitude as the
//! mainspring lets down, from the same steadiness report as the
//! Steadiness pane (core `steadiness`, as `timegrapher series` gives it).
//! Nothing is computed here: the curve comes ready made with the report,
//! and the walk under it (#741) is core's [`walk_view`], as the chat app's
//! card draws it.

use crate::marks;
use crate::strength::{self, Grid, Strength};
use crate::theme::{self, Icon};
use eframe::egui::{self, Align2, Color32, Pos2, RichText, Stroke};
use egui_plot::{Line, Plot, PlotPoint, PlotTransform, Points, VLine};
use timegrapher_core::chart_scale::{axis_number, round_step};
use timegrapher_core::figure_text::{fixed_text, true_minus};
use timegrapher_core::steadiness::IsochronismCurve;
use timegrapher_core::steady_views::{
    curve_amp, headline_lines, rate_padded, walk_amp, walk_axis, walk_spans, WalkDot, WalkView,
    WALK_HINT, WALK_KEY_START, WALK_SINCE, WALK_START,
};

/// Room for the vertical axis labels, as on the Steadiness pane.
const GUTTER: f32 = 64.0;
/// Room beside the widest rate label for its tick.
const GUTTER_PAD: f32 = 12.0;

/// How the pane shows isochronism: the classic curve of rate by
/// amplitude, or the walk through rate and amplitude over time (Peter).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum View {
    #[default]
    Curve,
    Walk,
}

/// The toolbar's choice of [`View`].
pub fn view_choice(ui: &mut egui::Ui, view: &mut View) {
    theme::tool_choice(
        ui,
        "View",
        "Isochronism the classic way, or as a walk through time",
        view,
        &[
            (
                View::Curve,
                Icon::IsoCurve,
                "Curve",
                "The median rate in each 2° of amplitude, and the curve through them",
            ),
            (
                View::Walk,
                Icon::IsoWalk,
                "Walk",
                "The rate at each amplitude as the watch runs down, each dot coloured by its \
                 time",
            ),
        ],
    );
}

/// The Walk view's note when core has no walk yet.
const NO_WALK: &str = "No walk yet: it needs the rate and amplitude over a longer stretch of \
                       the take.";

/// Draw the pane in `view`: the rate by amplitude, or the walk through
/// rate and amplitude over time when core has one. Both span the same
/// amplitudes, so a degree stays put when the view changes.
pub fn ui(
    ui: &mut egui::Ui,
    curve: Option<&IsochronismCurve>,
    walk: Option<&WalkView>,
    view: View,
) {
    let pal = theme::pal(ui);
    let missing = "No isochronism curve yet: the amplitude has not moved enough. It needs at \
                   least three 2° steps of amplitude with 10 readings in each, which a run-down \
                   over some hours gives.";
    let frame = Frame {
        amp: curve.zip(walk).and_then(|(c, w)| shared_amp(c, w)),
        gutter: gutter(ui, curve, walk),
    };
    for line in curve
        .map(|c| headline_lines(&c.headline))
        .unwrap_or_default()
    {
        ui.label(RichText::new(line).small().color(pal.text_secondary));
    }
    match (view, curve, walk) {
        (View::Curve, None, _) => note(ui, missing),
        (View::Curve, Some(c), _) => {
            ui.label(RichText::new(caption(c)).small().color(pal.text_tertiary));
            let h = room(ui);
            chart(ui, c, h, frame);
        }
        (View::Walk, _, None) => note(ui, NO_WALK),
        (View::Walk, _, Some(w)) => {
            let h = room(ui);
            walk_part(ui, w, h, frame);
        }
    }
}

/// The height left for the pane's charts, at least what one needs.
fn room(ui: &egui::Ui) -> f32 {
    ui.available_height().max(120.0)
}

/// A plot's height from what is left of `h`, never under what shows one.
fn plot_h(h: f32) -> f32 {
    h.max(40.0)
}

/// The plot's height from what is left of `h`, at most 1.1 times its
/// width so it is never stretched tall, and the room left over above it
/// that centres it (Hugh's F2).
fn shaped(ui: &egui::Ui, h: f32, gutter: f32) -> (f32, f32) {
    let h = plot_h(h);
    let most = plot_h(1.1 * (ui.available_width() - gutter));
    let shown = h.min(most);
    (shown, (h - shown) / 2.0)
}

/// An axis from `span` with round ends on a 1-2-5 step, cut into about 6
/// to 10 steps (Hugh's F3; core's [`walk_axis`]): the ends and the step.
fn round_axis(span: Span) -> (Span, f64) {
    let (lo, hi, step) = walk_axis(span);
    ((lo, hi), step)
}

/// Grid marks on `step`, or the next 1-2-5 step up where the labels
/// would sit closer than `px` points.
fn round_grid(g: egui_plot::GridInput, px: f64, step: f64) -> Vec<egui_plot::GridMark> {
    let ladder: Vec<f64> = std::iter::successors(Some(step), |s| Some(round_step(s * 1.01)))
        .take(12)
        .collect();
    theme::even_grid(g, px, &ladder)
}

/// The rounded axes over `x` and `y` and their grid, zero line across.
fn axes(x: Span, y: Span) -> (Span, Span, Grid) {
    let ((x, xs), (y, ys)) = (round_axis(x), round_axis(y));
    let grid = Grid::new(
        move |g| round_grid(g, 80.0, xs),
        move |g| round_grid(g, 24.0, ys),
    )
    .zero_y();
    (x, y, grid)
}

/// The height of a row of axis names.
fn name_row(ui: &egui::Ui) -> f32 {
    ui.text_style_height(&egui::TextStyle::Small) + 2.0
}

/// The axes' names with their units, as #1052 set them (Hugh's F1):
/// "Rate" over "s/d" above the rate labels, and "Amp." with "°" under
/// the amplitude labels at the right. `plot` is the whole plot with its
/// labels; `frame` the data within it.
fn axis_names(ui: &egui::Ui, plot: egui::Rect, frame: egui::Rect) {
    let pal = theme::pal(ui);
    let font = egui::TextStyle::Small.resolve(ui.style());
    let name = theme::semibold(font.size);
    let p = ui.painter();
    let right = frame.left() - 4.0;
    let unit = p.text(
        Pos2::new(right, frame.top() - font.size / 2.0 - 2.0),
        Align2::RIGHT_BOTTOM,
        "s/d",
        font.clone(),
        pal.text_secondary,
    );
    p.text(
        Pos2::new(right, unit.top()),
        Align2::RIGHT_BOTTOM,
        "Rate",
        name.clone(),
        pal.text,
    );
    let deg = p.text(
        Pos2::new(frame.right(), plot.bottom() + 2.0),
        Align2::RIGHT_TOP,
        "°",
        font,
        pal.text_secondary,
    );
    p.text(
        Pos2::new(deg.left() - 3.0, deg.top()),
        Align2::RIGHT_TOP,
        "Amp.",
        name,
        pal.text,
    );
}

/// The line under the headline naming what the chart draws.
fn caption(c: &IsochronismCurve) -> String {
    // The slope and its amplitudes are the headline's: said once.
    let mut s = format!(
        "Dots: the median rate in each {:.0}° of amplitude, with bars over its middle half · \
         Line: the curve through them",
        c.bin_deg
    );
    if let Some(t) = c.turn_deg {
        s += &format!(" · Turn: where the curve turns, {t:.0}°");
    }
    s
}

/// The rate at amplitude `a` on the quadratic.
fn curve_at(c: &IsochronismCurve, a: f64) -> f64 {
    let x = a - c.curve_centre_deg;
    c.curve[0] + c.curve[1] * x + c.curve[2] * x * x
}

/// A rate on the axis: a bare number with as few decimals as the grid
/// step needs and a true minus, its unit named over the axis (Hugh's F1).
fn rate_label(v: f64, step: f64) -> String {
    true_minus(&axis_number(v, step))
}

/// What the two views share, so a degree sits at one x in either: the
/// amplitude range ([`shared_amp`]) and the rate axis's width.
#[derive(Clone, Copy)]
struct Frame {
    amp: Option<Span>,
    gutter: f32,
}

/// The rate axis's width for either view: room for the widest label at
/// the round ends of both views' rate ranges and for the axis's name,
/// never under [`GUTTER`].
fn gutter(ui: &egui::Ui, curve: Option<&IsochronismCurve>, walk: Option<&WalkView>) -> f32 {
    let spans = curve
        .and_then(curve_and_bounds)
        .map(|b| b.2)
        .into_iter()
        .chain(walk.map(|w| walk_bounds(w).1));
    let body = egui::TextStyle::Body.resolve(ui.style());
    let name = theme::semibold(egui::TextStyle::Small.resolve(ui.style()).size);
    let width = |text: String, font: egui::FontId| {
        ui.fonts_mut(|f| f.layout_no_wrap(text, font, Color32::WHITE).size().x)
    };
    spans
        .flat_map(|s| {
            let (lo, hi) = round_axis(s).0;
            [lo, hi]
        })
        // With a decimal, so either view's labels fit whatever step its
        // height picks.
        .map(|v| width(true_minus(&fixed_text(v, 1)), body.clone()))
        .chain([width("Rate".into(), name.clone())])
        .fold(GUTTER, |g, w| g.max(w + GUTTER_PAD))
}

fn chart(ui: &mut egui::Ui, c: &IsochronismCurve, h: f32, frame: Frame) {
    let pal = theme::pal(ui);
    let Some((curve, x_bounds, y_bounds)) = curve_and_bounds(c) else {
        return;
    };
    let (x_bounds, y_bounds, grid) = axes(frame.amp.unwrap_or(x_bounds), y_bounds);
    let (head, foot) = (2.0 * name_row(ui), name_row(ui));
    let (h, centre) = shaped(ui, h - head - foot, frame.gutter);
    ui.add_space(centre);
    // The turn is named in a lane of one row above the plot, as every
    // named line is (see `marks`); on a short plot, inside along its top.
    let names_id = ui.id().with("turn");
    let lane_h = marks::ROW_H + marks::LANE_GAP;
    let inside = c.turn_deg.is_some()
        && marks::short(
            ui,
            names_id.with("short"),
            h.min(ui.available_height()) - lane_h,
        );
    let lane = if c.turn_deg.is_some() && !inside {
        lane_h
    } else {
        0.0
    };
    // The lane, and room above it for the rate axis's name.
    ui.add_space(head.max(lane));
    // What a name inside keeps clear of: the curve, the dots and the bars'
    // tops.
    let tops: Vec<[f64; 2]> = curve
        .iter()
        .copied()
        .chain(c.bins.iter().map(|b| [b.amplitude_deg, b.rate_q75]))
        .collect();
    let names: Vec<(f64, String, marks::Rank)> = c
        .turn_deg
        .map(|t| (t, "Turn".to_string(), marks::Rank::Strong))
        .into_iter()
        .collect();
    let fill = ui.visuals().extreme_bg_color.gamma_multiply(0.8);
    let h = plot_h(h + head - head.max(lane));
    let resp =
        chart_plot("isochronism", &grid, (h, frame.gutter), x_bounds, y_bounds).show(ui, |p| {
            grid.draw(p, pal);
            chart_items(p, c, curve, pal);
            if inside {
                p.add(marks::InsideNames {
                    base: egui_plot::PlotItemBase::new("turn name".into()),
                    names: names.clone(),
                    tops: tops.clone(),
                    fill,
                });
            }
        });
    axis_names(ui, resp.response.rect, *resp.transform.frame());
    ui.add_space(foot);
    let hover = |t: f64| format!("Turn: {t:.0}°");
    if inside {
        let placed = marks::inside_placed(ui, &resp.transform, &names, &tops);
        let hover_of = |_: &str| c.turn_deg.map_or_else(String::new, hover);
        marks::inside_hovers(ui, *resp.transform.frame(), &placed, &hover_of, names_id);
    } else if let Some(t) = c.turn_deg {
        let turn = marks::Named {
            x: resp.transform.position_from_point_x(t),
            name: "Turn".into(),
            rank: marks::Rank::Strong,
            ink: pal.text_secondary,
            hover: hover(t),
        };
        marks::lane_above_one(ui, *resp.transform.frame(), &[turn], names_id);
    }
}

/// A low and high bound on one axis.
type Span = (f64, f64);

/// The fitted curve across the bins' amplitudes, and the chart's
/// amplitude and rate bounds; none with no bins.
fn curve_and_bounds(c: &IsochronismCurve) -> Option<(Vec<[f64; 2]>, Span, Span)> {
    let (Some(first), Some(last)) = (c.bins.first(), c.bins.last()) else {
        return None;
    };
    let (a_lo, a_hi) = (first.amplitude_deg, last.amplitude_deg);
    let curve: Vec<[f64; 2]> = (0..=60)
        .map(|i| {
            let a = a_lo + (a_hi - a_lo) * i as f64 / 60.0;
            [a, curve_at(c, a)]
        })
        .collect();
    // Scale to the bars and the curve, padded by 15% as the charts are.
    let (lo, hi) = c
        .bins
        .iter()
        .flat_map(|b| [b.rate_q25, b.rate_q75])
        .chain(curve.iter().map(|p| p[1]))
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(l, h), v| {
            (l.min(v), h.max(v))
        });
    let amps: Vec<f64> = c.bins.iter().map(|b| b.amplitude_deg).collect();
    Some((curve, curve_amp(&amps, c.bin_deg)?, rate_padded(lo, hi)))
}

/// The chart's plot, `id`, fixed in place, `h` high over the given bounds, on
/// `grid`.
fn chart_plot(
    id: &str,
    grid: &Grid,
    (h, gutter): (f32, f32),
    (x_lo, x_hi): Span,
    (y_lo, y_hi): Span,
) -> Plot<'static> {
    grid.on(Plot::new(id))
        .allow_scroll(false)
        .allow_zoom(false)
        .allow_drag(false)
        .allow_axis_zoom_drag(false)
        .allow_boxed_zoom(false)
        .allow_double_click_reset(false)
        .show_x(false)
        .show_y(false)
        .y_axis_min_width(gutter)
        .height(h)
        .x_axis_formatter(|m, r| amp_label(m.value, r))
        .y_axis_formatter(|m, _| rate_label(m.value, m.step_size))
        .default_x_bounds(x_lo, x_hi)
        .default_y_bounds(y_lo, y_hi)
}

/// An amplitude axis label, a bare number with its unit named under the
/// axis (Hugh's F1), or none within 3% of either end: past the right it
/// runs off the plot, and at the left it sits on the lowest rate label in
/// the corner (Walter's walk of #1133).
fn amp_label(v: f64, range: &std::ops::RangeInclusive<f64>) -> String {
    let edge = 0.03;
    if theme::near_an_end(v, range, edge, edge) {
        String::new()
    } else {
        format!("{v:.0}")
    }
}

/// The bins' middle halves, the curve, the medians and the turn's line.
fn chart_items(
    p: &mut egui_plot::PlotUi,
    c: &IsochronismCurve,
    curve: Vec<[f64; 2]>,
    pal: &theme::Palette,
) {
    let colour = pal.trace_rate;
    for b in &c.bins {
        p.line(
            Line::new(
                "middle half",
                vec![[b.amplitude_deg, b.rate_q25], [b.amplitude_deg, b.rate_q75]],
            )
            .color(colour)
            .width(strength::DATA_W),
        );
    }
    p.line(
        Line::new("curve", curve)
            .color(colour)
            .width(strength::DATA_W),
    );
    p.points(
        Points::new(
            "median",
            c.bins
                .iter()
                .map(|b| [b.amplitude_deg, b.rate_s_per_day])
                .collect::<Vec<_>>(),
        )
        .color(colour)
        .radius(3.0_f32),
    );
    if let Some(t) = c.turn_deg {
        let s = strength::stroke(Strength::CallOut, pal);
        p.vline(VLine::new("turn", t).color(s.color).width(s.width));
    }
}

/// How near the pointer must be to a dot, in points, for its hover.
const HOVER_PX: f32 = 8.0;

/// The walk: its hint, the plot of rate against amplitude with each dot
/// coloured by its time, and the colour key, `h` high in all. `amp` is
/// the amplitude range and axis width it shares with the curve.
fn walk_part(ui: &mut egui::Ui, w: &WalkView, h: f32, frame: Frame) {
    let pal = theme::pal(ui);
    ui.label(RichText::new(WALK_HINT).small().color(pal.text_secondary));
    let key_h = ui.text_style_height(&egui::TextStyle::Small) + ui.spacing().item_spacing.y;
    let (head, foot) = (2.0 * name_row(ui), name_row(ui));
    let (h, centre) = shaped(ui, h - 2.0 * key_h - head - foot, frame.gutter);
    ui.add_space(centre + head);
    let (x, y) = walk_bounds(w);
    let (x, y, grid) = axes(frame.amp.unwrap_or(x), y);
    let resp = chart_plot("isochronism walk", &grid, (h, frame.gutter), x, y)
        .show(ui, |p| grid.draw(p, pal));
    paint_walk(ui, &resp.transform, w);
    axis_names(ui, resp.response.rect, *resp.transform.frame());
    ui.add_space(foot);
    let at = ui
        .ctx()
        .pointer_hover_pos()
        .filter(|_| resp.response.hovered());
    if let Some(i) = at.and_then(|at| nearest_dot(w, &resp.transform, at)) {
        resp.response.on_hover_text_at_pointer(&w.dots[i].hover);
    }
    let dark = ui.visuals().dark_mode;
    ui.horizontal(|ui| {
        let small = |t: &str| RichText::new(t).small().color(pal.text_secondary);
        ui.label(small(WALK_KEY_START));
        crate::tfmap::ramp(ui, 120.0, |t| crate::tfmap::time_colour(dark, t));
        ui.label(small(&w.last_text));
        ui.label(small(WALK_SINCE));
    });
}

/// One amplitude range for the curve and the walk: the span of both
/// (core's [`walk_amp`]).
fn shared_amp(c: &IsochronismCurve, w: &WalkView) -> Option<Span> {
    let (_, amp, _) = curve_and_bounds(c)?;
    Some(walk_amp(&w.dots, Some(amp)))
}

/// The walk's amplitude and rate bounds, padded as the curve's are
/// (core's [`walk_spans`]).
fn walk_bounds(w: &WalkView) -> (Span, Span) {
    let [amp, rate] = walk_spans(&w.dots);
    (amp, rate)
}

/// The walk as Hugh's look C draws it (Peter, 11-05): a 2 pt trail
/// through the dots in time order, each step shading from its first dot's
/// colour to its last's and lifted wherever core says a dot is not joined
/// (a stretch left out or a break: never filled); the dots over it; a
/// ring at each of core's hour beads; a ring at the start and a large dot
/// at the end; then their names, over everything.
fn paint_walk(ui: &egui::Ui, t: &PlotTransform, w: &WalkView) {
    let (pal, dark) = (theme::pal(ui), ui.visuals().dark_mode);
    let bg = ui.visuals().extreme_bg_color;
    let colour = |when: f64| crate::tfmap::time_colour(dark, when as f32);
    let pos = |p: [f64; 2]| t.position_from_point(&PlotPoint::new(p[0], p[1]));
    let dots: Vec<Pos2> = w.dots.iter().map(|d| pos(dot_at(d))).collect();
    let p = ui.painter().with_clip_rect(t.frame().expand(8.0));
    for (from, to, (t0, t1)) in steps(w) {
        let (a, b) = (pos(from), pos(to));
        // In short runs, so its colour moves from one dot's to the next's.
        let runs = 8;
        for k in 0..runs {
            let f = |k: usize| k as f32 / runs as f32;
            let mid = (f(k) + f(k + 1)) as f64 / 2.0;
            p.line_segment(
                [a.lerp(b, f(k)), a.lerp(b, f(k + 1))],
                Stroke::new(2.0_f32, colour(t0 + (t1 - t0) * mid)),
            );
        }
    }
    for (d, at) in w.dots.iter().zip(&dots) {
        p.circle_filled(*at, 2.5, colour(d.when));
    }
    let ring = |at: Pos2, r: f32, when: f64| {
        p.circle(at, r, bg, Stroke::new(2.5_f32, colour(when)));
    };
    for b in &w.beads {
        ring(dots[b.dot], 5.0, w.dots[b.dot].when);
    }
    let (Some(start), Some(end)) = (dots.first(), dots.last()) else {
        return;
    };
    ring(*start, 7.0, 0.0);
    p.circle(*end, 6.5, colour(1.0), Stroke::new(2.0_f32, bg));
    let font = egui::FontId::proportional(12.0);
    // Each name on a halo of the plot's background, so a trail that
    // crosses it where the walk knots leaves it readable (Walter's walk of
    // #1148).
    let name = |at: Pos2, align: Align2, text: &str| {
        let g = p.layout_no_wrap(text.to_string(), font.clone(), pal.text);
        let r = align.anchor_size(at, g.size());
        p.rect_filled(r.expand(2.0), 3.0, bg.gamma_multiply(0.85));
        p.galley(r.min, g, pal.text);
    };
    name(
        *start + egui::vec2(-10.0, -10.0),
        Align2::RIGHT_BOTTOM,
        WALK_START,
    );
    // The end's hour off the trail, as the beads' are, past its larger
    // dot (Walter's walk of #1148: below it, the trail crossed it).
    let (side, align) = bead_side(&dots, dots.len() - 1);
    name(*end + 14.0 * side, align, &w.last_text);
    for b in &w.beads {
        let (side, align) = bead_side(&dots, b.dot);
        name(dots[b.dot] + 11.0 * side, align, &b.text);
    }
}

/// Where a bead's name goes: square to the trail at dot `i`, on the side
/// clear of the other dots, so it never sits on the line; and how it is
/// anchored there.
fn bead_side(dots: &[Pos2], i: usize) -> (egui::Vec2, Align2) {
    let along = dots[(i + 1).min(dots.len() - 1)] - dots[i.saturating_sub(1)];
    let n = egui::vec2(-along.y, along.x).normalized();
    let n = if n.is_finite() {
        n
    } else {
        egui::vec2(0.0, -1.0)
    };
    let room = |n: egui::Vec2| {
        let at = dots[i] + 16.0 * n;
        dots.iter()
            .map(|d| d.distance(at))
            .fold(f32::INFINITY, f32::min)
    };
    let n = if room(-n) > room(n) { -n } else { n };
    // Anchored by the side it leans to: a component past half is a lean.
    let side = |v: f32| (v.round() + 1.0) as usize;
    let x = [egui::Align::RIGHT, egui::Align::Center, egui::Align::LEFT][side(n.x)];
    let y = [egui::Align::BOTTOM, egui::Align::Center, egui::Align::TOP][side(n.y)];
    (n, Align2([x, y]))
}

/// A dot's place on the plot: its amplitude and rate.
fn dot_at(d: &WalkDot) -> [f64; 2] {
    [d.amplitude_deg, d.rate_s_per_day]
}

/// A step of the walk: where it goes from and to, and the two dots' times.
type Step = ([f64; 2], [f64; 2], (f64, f64));

/// The walk's steps from each dot to the next one core joins to it.
fn steps(w: &WalkView) -> Vec<Step> {
    w.dots
        .windows(2)
        .filter(|p| p[1].joined)
        .map(|p| (dot_at(&p[0]), dot_at(&p[1]), (p[0].when, p[1].when)))
        .collect()
}

/// The dot nearest the pointer, within [`HOVER_PX`].
fn nearest_dot(w: &WalkView, t: &egui_plot::PlotTransform, at: egui::Pos2) -> Option<usize> {
    w.dots
        .iter()
        .enumerate()
        .map(|(i, d)| {
            let pos = t.position_from_point(&egui_plot::PlotPoint::new(
                d.amplitude_deg,
                d.rate_s_per_day,
            ));
            (i, pos.distance(at))
        })
        .filter(|&(_, dist)| dist <= HOVER_PX)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i)
}

fn note(ui: &mut egui::Ui, text: &str) {
    let pal = theme::pal(ui);
    ui.centered_and_justified(|ui| {
        ui.label(RichText::new(text).small().color(pal.text_tertiary));
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use timegrapher_core::steadiness::AmplitudeBin;

    /// A run-down drawn by hand: flat above 250°, falling below.
    fn run_down() -> IsochronismCurve {
        let bins: Vec<AmplitudeBin> = (0..8)
            .map(|i| {
                let a = 236.0 + 4.0 * i as f64;
                let r = 17.0 - 0.02 * (252.0 - a).max(0.0).powi(2);
                AmplitudeBin {
                    amplitude_deg: a,
                    readings: 200,
                    rate_s_per_day: r,
                    rate_q25: r - 0.5,
                    rate_q75: r + 0.5,
                    from_wind_h: 15.0 - 2.0 * i as f64,
                    to_wind_h: 16.0 - 2.0 * i as f64,
                }
            })
            .collect();
        IsochronismCurve {
            step_s: 10.0,
            bin_deg: 2.0,
            bins,
            amplitude_from_deg: 235.0,
            amplitude_to_deg: 265.0,
            s_per_day_per_deg: Some(0.4),
            s_per_day_per_deg_se: Some(0.02),
            rate_change_s_per_day: Some(12.0),
            reliability: Some(0.9),
            curve_centre_deg: 250.0,
            curve: [16.8, 0.2, -0.015],
            curve_explained: 0.95,
            turn_deg: Some(256.7),
            headline: "The rate falls as the amplitude does.".into(),
        }
    }

    #[test]
    fn draws_a_curve_and_says_why_when_it_is_missing() {
        let curve = run_down();
        assert!(caption(&curve).contains("where the curve turns, 257°"));
        // The slope and its amplitudes only in the headline.
        assert!(
            !caption(&curve).contains("per degree"),
            "{}",
            caption(&curve)
        );
        assert!((curve_at(&curve, 250.0) - 16.8).abs() < 1e-12);
        // Bare numbers, a true minus, no minus zero (Hugh's F1).
        assert_eq!(rate_label(-0.0001, 0.5), "0.0");
        assert_eq!(rate_label(0.0, 0.5), "0.0");
        assert_eq!(rate_label(1.5, 0.5), "1.5");
        assert_eq!(rate_label(-2.0, 1.0), "−2");

        let ctx = egui::Context::default();
        theme::install(&ctx);
        for c in [Some(&curve), None] {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(700.0, 500.0),
                )),
                ..Default::default()
            };
            let out = ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |u| super::ui(u, c, None, View::Curve));
            });
            assert!(!out.shapes.is_empty());
        }
    }

    /// The turn is named "Turn" above the plot, over its line, clear of
    /// the rate labels.
    #[test]
    fn the_turn_is_named_above_the_plot() {
        let curve = run_down();
        let ctx = egui::Context::default();
        theme::install(&ctx);
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(700.0, 500.0),
            )),
            ..Default::default()
        };
        let out = ctx.run(input, |ctx| {
            egui::CentralPanel::default()
                .show(ctx, |u| super::ui(u, Some(&curve), None, View::Curve));
        });
        let texts: Vec<(String, egui::Rect)> = out
            .shapes
            .iter()
            .filter_map(|c| match &c.shape {
                egui::Shape::Text(t) => Some((
                    t.galley.text().to_string(),
                    t.galley.rect.translate(t.pos.to_vec2()),
                )),
                _ => None,
            })
            .collect();
        let turn = texts.iter().find(|t| t.0 == "Turn").expect("Turn is named");
        let rates: Vec<_> = texts.iter().filter(|t| is_rate_label(t)).collect();
        assert!(!rates.is_empty(), "{texts:?}");
        // Above the plot: the top rate label is centred on its top edge,
        // so "Turn" ends above every label's middle and on none.
        assert!(
            rates
                .iter()
                .all(|r| r.1.center().y >= turn.1.bottom() && !r.1.intersects(turn.1)),
            "{texts:?}"
        );
        // Over its line: 256.7° sits right of the middle of 236° to 264°.
        let x = turn.1.center().x;
        assert!(x > 350.0 && x < 700.0, "{turn:?}");
    }

    /// On a short plot the lane gives way: "Turn" goes inside, along the
    /// plot's top (not above the highest rate label), or where the curve
    /// leaves it no room, to its line's hover.
    #[test]
    fn on_a_short_plot_the_turn_leaves_the_lane() {
        let curve = run_down();
        let ctx = egui::Context::default();
        theme::install(&ctx);
        let mut texts: Vec<(String, egui::Rect)> = Vec::new();
        // Twice: the short switch keeps what it was last frame.
        for _ in 0..2 {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(700.0, 160.0),
                )),
                ..Default::default()
            };
            let out = ctx.run(input, |ctx| {
                egui::CentralPanel::default()
                    .show(ctx, |u| super::ui(u, Some(&curve), None, View::Curve));
            });
            texts = out
                .shapes
                .iter()
                .filter_map(|c| match &c.shape {
                    egui::Shape::Text(t) => Some((
                        t.galley.text().to_string(),
                        t.galley.rect.translate(t.pos.to_vec2()),
                    )),
                    _ => None,
                })
                .collect();
        }
        let turn = texts.iter().find(|t| t.0 == "Turn");
        let top_rate = texts
            .iter()
            .filter(|t| is_rate_label(t))
            .map(|t| t.1.top())
            .fold(f32::INFINITY, f32::min);
        assert!(top_rate.is_finite(), "{texts:?}");
        if let Some(turn) = turn {
            assert!(turn.1.top() > top_rate - 8.0, "{texts:?}");
        }
    }

    /// A rate axis label: a bare number in the gutter left of the plot.
    fn is_rate_label(t: &(String, egui::Rect)) -> bool {
        t.0.replace('−', "-").parse::<f64>().is_ok() && t.1.right() < GUTTER + 16.0
    }

    /// A walk on core's 5 min bins: six points, then a stretch left out,
    /// then three more.
    fn walk() -> WalkView {
        use timegrapher_core::walk::{IsochronismWalk, WalkPoint, WALK_BIN_S};
        let at = |k: usize| WalkPoint {
            t_s: (k as f64 + 0.5) * WALK_BIN_S,
            wind_h: 0.0,
            rate_s_per_day: 2.0 - 0.3 * k as f64,
            amplitude_deg: 290.0 - 4.0 * k as f64,
            readings: 30,
            amp_windows: 150,
        };
        let w = IsochronismWalk {
            bin_s: WALK_BIN_S,
            points: (0..6).chain(9..12).map(at).collect(),
        };
        timegrapher_core::steady_views::walk_view(&w).expect("nine points")
    }

    /// The line is lifted where core says a dot is not joined, never
    /// bridging the stretch left out (#741).
    #[test]
    fn the_walk_line_breaks_where_core_says() {
        let w = walk();
        let at = |i: usize| [w.dots[i].amplitude_deg, w.dots[i].rate_s_per_day];
        let s = steps(&w);
        // Five steps through the first six dots, none across to the
        // seventh, two through the last three.
        assert_eq!(s.len(), 7);
        assert!(!s.iter().any(|st| st.0 == at(5) && st.1 == at(6)));
        assert_eq!((s[5].0, s[5].1), (at(6), at(7)));
        // Each shading from the time of the dot it goes from to the
        // next's (Hugh's look C, from Peter's colour by the dot it leaves).
        for (st, from) in s.iter().zip([0, 1, 2, 3, 4, 6, 7]) {
            assert_eq!(st.2, (w.dots[from].when, w.dots[from + 1].when));
        }
    }

    /// The Walk view draws the walk with core's hint, its dots in viridis
    /// by time, the key's ends and core's hover on a dot.
    #[test]
    fn the_walk_view_draws_the_walk() {
        let (curve, w) = (run_down(), walk());
        let ctx = egui::Context::default();
        theme::install(&ctx);
        ctx.style_mut(|s| {
            s.interaction.show_tooltips_only_when_still = false;
            s.interaction.tooltip_delay = 0.0;
        });
        let dark = ctx.style().visuals.dark_mode;
        let colour = |when: f64| crate::tfmap::time_colour(dark, when as f32);
        let first = colour(w.dots[0].when);
        // A first frame finds the first dot; the pointer then rests on it
        // until its hover shows.
        let mut at = None;
        let mut out = None;
        for _ in 0..5 {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(700.0, 700.0),
                )),
                events: at.map(egui::Event::PointerMoved).into_iter().collect(),
                ..Default::default()
            };
            let o = ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |u| {
                    super::ui(u, Some(&curve), Some(&w), View::Walk);
                });
            });
            at = o.shapes.iter().find_map(|c| match &c.shape {
                egui::Shape::Circle(c) if c.fill == first => Some(c.center),
                _ => None,
            });
            out = Some(o);
        }
        let out = out.unwrap();
        let texts: Vec<String> = out
            .shapes
            .iter()
            .filter_map(|c| match &c.shape {
                egui::Shape::Text(t) => Some(t.galley.text().to_string()),
                _ => None,
            })
            .collect();
        for want in [WALK_HINT, WALK_KEY_START, w.last_text.as_str(), WALK_SINCE] {
            assert!(texts.iter().any(|t| t == want), "{want}: {texts:?}");
        }
        assert!(
            texts.iter().any(|t| t == &w.dots[0].hover),
            "the first dot's hover: {texts:?}"
        );
        let fills: Vec<egui::Color32> = out
            .shapes
            .iter()
            .filter_map(|c| match &c.shape {
                egui::Shape::Circle(c) => Some(c.fill),
                _ => None,
            })
            .collect();
        // Each dot on the time map at its time; the last at the key's end.
        assert_eq!(w.dots.last().map(|d| d.when), Some(1.0));
        for d in &w.dots {
            assert!(fills.contains(&colour(d.when)), "{fills:?}");
        }
    }

    /// The curve and the walk under it share one amplitude range, so a
    /// degree sits at the same x in both (Walter's walk of #1111).
    #[test]
    fn the_curve_and_the_walk_share_the_amplitude_axis() {
        let (curve, w) = (run_down(), walk());
        let (_, (c_lo, c_hi), _) = curve_and_bounds(&curve).unwrap();
        let ((w_lo, w_hi), _) = walk_bounds(&w);
        assert_ne!((c_lo, c_hi), (w_lo, w_hi), "the test needs two ranges");
        assert_eq!(
            shared_amp(&curve, &w),
            Some((c_lo.min(w_lo), c_hi.max(w_hi)))
        );
        let ctx = egui::Context::default();
        theme::install(&ctx);
        // The amplitude labels each view draws, and where.
        let labels = |view: View| {
            let mut texts: Vec<(String, f32)> = Vec::new();
            for _ in 0..2 {
                let input = egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(700.0, 700.0),
                    )),
                    ..Default::default()
                };
                let out = ctx.run(input, |ctx| {
                    egui::CentralPanel::default()
                        .show(ctx, |u| super::ui(u, Some(&curve), Some(&w), view));
                });
                texts = out
                    .shapes
                    .iter()
                    .filter_map(|c| match &c.shape {
                        // The amplitude labels: bare numbers, the only
                        // ones over 100 here.
                        egui::Shape::Text(t)
                            if t.galley.text().parse::<f64>().is_ok_and(|v| v >= 100.0) =>
                        {
                            Some((
                                t.galley.text().to_string(),
                                t.galley.rect.translate(t.pos.to_vec2()).center().x,
                            ))
                        }
                        _ => None,
                    })
                    .collect();
            }
            texts
        };
        // Every amplitude label sits at one x in either view, so a degree
        // stays put when the view changes.
        let (curve_x, walk_x) = (labels(View::Curve), labels(View::Walk));
        assert_eq!(curve_x.len(), walk_x.len(), "{curve_x:?} {walk_x:?}");
        assert!(curve_x.len() >= 4, "{curve_x:?}");
        for ((a, x), (b, y)) in curve_x.iter().zip(&walk_x) {
            assert_eq!(a, b);
            assert!((x - y).abs() < 1.0, "{a}: {x} and {y}");
        }
    }

    /// An amplitude label at either end of the axis is left off: at the
    /// left it would sit on the lowest rate label (Walter, #1133).
    #[test]
    fn no_amplitude_label_sits_in_a_corner() {
        let r = 275.0..=285.0;
        assert_eq!(amp_label(275.0, &r), "");
        assert_eq!(amp_label(285.0, &r), "");
        assert_eq!(amp_label(276.0, &r), "276");
        assert_eq!(amp_label(284.0, &r), "284");
    }

    /// The headline goes in short lines, one fact a line (Hugh's F4).
    #[test]
    fn the_summary_is_one_fact_a_line() {
        let h = "Between 145° and 251° the rate goes with the amplitude by -0.23 ± 0.01 s/d \
                 per degree. The rate reads +1.0 s/d at 145° and -3.0 s/d at 251°; it changes \
                 least near 213°.";
        assert_eq!(
            headline_lines(h),
            [
                "Between 145° and 251° the rate goes with the amplitude by -0.23 ± 0.01 s/d per \
                 degree",
                "The rate reads +1.0 s/d at 145° and -3.0 s/d at 251°",
                "It changes least near 213°",
            ]
        );
    }

    /// Both axes run between round values on 1-2-5 steps, about 6 to 10
    /// of them (Hugh's F3).
    #[test]
    fn the_axes_have_round_ends_and_steps() {
        assert_eq!(round_axis((-118.7, -31.2)), ((-120.0, -30.0), 10.0));
        assert_eq!(round_axis((116.0, 263.0)), ((100.0, 280.0), 20.0));
        let ((lo, hi), step) = round_axis((236.3, 263.9));
        assert_eq!((lo, hi, step), (235.0, 265.0, 5.0));
        let ((lo, hi), step) = round_axis((-0.4, 2.35));
        assert!(
            (6.0..=10.0).contains(&((hi - lo) / step).round()),
            "{lo} {hi} {step}"
        );
    }

    /// A walk over 30 hours on core's 5 min bins, drifting down.
    fn long_walk() -> WalkView {
        use timegrapher_core::walk::{IsochronismWalk, WalkPoint, WALK_BIN_S};
        let points = (0..360)
            .map(|k| WalkPoint {
                t_s: (k as f64 + 0.5) * WALK_BIN_S,
                wind_h: 0.0,
                rate_s_per_day: 2.0 - 0.01 * k as f64 + 0.3 * (k as f64 / 9.0).sin(),
                amplitude_deg: 290.0 - 0.2 * k as f64,
                readings: 30,
                amp_windows: 150,
            })
            .collect();
        let w = IsochronismWalk {
            bin_s: WALK_BIN_S,
            points,
        };
        timegrapher_core::steady_views::walk_view(&w).expect("a long walk")
    }

    /// One frame of the pane as shapes, `size` wide and high.
    fn frame_of(
        ctx: &egui::Context,
        size: egui::Vec2,
        w: Option<&WalkView>,
        view: View,
    ) -> Vec<egui::epaint::ClippedShape> {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
            ..Default::default()
        };
        ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |u| super::ui(u, Some(&run_down()), w, view));
        })
        .shapes
    }

    /// The texts drawn, with where.
    fn texts_of(shapes: &[egui::epaint::ClippedShape]) -> Vec<(String, egui::Rect)> {
        shapes
            .iter()
            .filter_map(|c| match &c.shape {
                egui::Shape::Text(t) => Some((
                    t.galley.text().to_string(),
                    t.galley.rect.translate(t.pos.to_vec2()),
                )),
                _ => None,
            })
            .collect()
    }

    /// Look C (Peter, 11-05): a ring named "Start" at the first dot, a
    /// large dot named with its hour at the last, a ring at each of core's
    /// hour beads named off the trail, and the axes named with their units.
    #[test]
    fn the_walk_marks_its_start_end_and_hours() {
        let w = long_walk();
        assert_eq!(
            w.beads.iter().map(|b| b.text.as_str()).collect::<Vec<_>>(),
            ["12 h", "24 h"]
        );
        let ctx = egui::Context::default();
        theme::install(&ctx);
        let (bg, dark) = (
            ctx.style().visuals.extreme_bg_color,
            ctx.style().visuals.dark_mode,
        );
        let colour = |when: f64| crate::tfmap::time_colour(dark, when as f32);
        let shapes = frame_of(&ctx, egui::vec2(800.0, 700.0), Some(&w), View::Walk);
        let texts = texts_of(&shapes);
        for want in [
            "Start",
            w.last_text.as_str(),
            "12 h",
            "24 h",
            "Rate",
            "s/d",
            "Amp.",
            "°",
        ] {
            assert!(texts.iter().any(|t| t.0 == want), "{want}: {texts:?}");
        }
        let circles: Vec<egui::epaint::CircleShape> = shapes
            .iter()
            .filter_map(|c| match &c.shape {
                egui::Shape::Circle(c) => Some(*c),
                _ => None,
            })
            .collect();
        let ring = |r: f32, when: f64| {
            circles
                .iter()
                .find(|c| c.radius == r && c.fill == bg && c.stroke.color == colour(when))
                .map(|c| c.center)
        };
        let start = ring(7.0, 0.0).expect("the start's ring");
        let end = circles
            .iter()
            .find(|c| c.radius == 6.5 && c.fill == colour(1.0))
            .expect("the end's dot");
        let name = |n: &str| texts.iter().find(|t| t.0 == n).unwrap().1;
        assert!(name("Start").max.x <= start.x && name("Start").max.y <= start.y);
        let last = name(&w.last_text);
        assert!(last.distance_to_pos(end.center) > end.radius, "{last:?}");
        // Each bead's name and the end's sit clear of every dot, so never
        // on the trail.
        let dots: Vec<Pos2> = circles
            .iter()
            .filter(|c| c.radius == 2.5)
            .map(|c| c.center)
            .collect();
        assert_eq!(dots.len(), w.dots.len());
        for b in &w.beads {
            assert!(ring(5.0, w.dots[b.dot].when).is_some(), "{}", b.text);
        }
        // Each name sits on a halo of the plot's background, painted
        // just before it.
        for want in ["Start", w.last_text.as_str(), "12 h", "24 h"] {
            let i = shapes
                .iter()
                .position(|c| matches!(&c.shape, egui::Shape::Text(t) if t.galley.text() == want))
                .unwrap();
            let halo = matches!(&shapes[i - 1].shape,
                egui::Shape::Rect(r) if r.fill == bg.gamma_multiply(0.85)
                    && r.rect.contains_rect(name(want)));
            assert!(halo, "{want}");
        }
        for text in w.beads.iter().map(|b| &b.text).chain([&w.last_text]) {
            let r = name(text);
            assert!(
                !dots.iter().any(|d| r.expand(1.0).contains(*d)),
                "{text} {r:?}"
            );
        }
    }

    /// On a tall, narrow pane the plot is never stretched taller than 1.1
    /// times its width, and sits in the middle (Hugh's F2).
    #[test]
    fn the_plot_is_never_stretched_tall() {
        let ctx = egui::Context::default();
        theme::install(&ctx);
        let bg = ctx.style().visuals.extreme_bg_color;
        for view in [View::Curve, View::Walk] {
            let shapes = frame_of(&ctx, egui::vec2(360.0, 1200.0), Some(&long_walk()), view);
            let plot = shapes
                .iter()
                .filter_map(|c| match &c.shape {
                    egui::Shape::Rect(r) if r.fill == bg => Some(r.rect),
                    _ => None,
                })
                .max_by(|a, b| a.area().total_cmp(&b.area()))
                .expect("the plot's background");
            assert!(
                plot.height() <= 1.1 * plot.width() + 1.0,
                "{view:?}: {plot:?}"
            );
            assert!(
                plot.top() > 150.0,
                "{view:?}: centred, not at the top: {plot:?}"
            );
        }
    }

    /// One view at a time: the Curve view has no walk, the Walk view no
    /// curve; and each says what it lacks (Peter).
    #[test]
    fn the_pane_shows_one_view_at_a_time() {
        let (curve, w) = (run_down(), walk());
        let ctx = egui::Context::default();
        theme::install(&ctx);
        let texts = |c: Option<&IsochronismCurve>, wv: Option<&WalkView>, view: View| {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(700.0, 700.0),
                )),
                ..Default::default()
            };
            let out = ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |u| super::ui(u, c, wv, view));
            });
            out.shapes
                .iter()
                .filter_map(|c| match &c.shape {
                    egui::Shape::Text(t) => Some(t.galley.text().to_string()),
                    _ => None,
                })
                .collect::<Vec<_>>()
        };
        let both = texts(Some(&curve), Some(&w), View::Curve);
        assert!(both.iter().any(|t| t == &caption(&curve)), "{both:?}");
        assert!(!both.iter().any(|t| t == WALK_HINT), "{both:?}");
        let walk_only = texts(Some(&curve), Some(&w), View::Walk);
        assert!(walk_only.iter().any(|t| t == WALK_HINT), "{walk_only:?}");
        assert!(
            walk_only
                .iter()
                .any(|t| t == &headline_lines(&curve.headline)[0]),
            "{walk_only:?}"
        );
        assert!(
            !walk_only.iter().any(|t| t == &caption(&curve)),
            "{walk_only:?}"
        );
        let none = texts(Some(&curve), None, View::Walk);
        assert!(
            none.iter().any(|t| t.starts_with("No walk yet")),
            "{none:?}"
        );
    }
}
