//! The Isochronism pane: how the rate moves with the amplitude as the
//! mainspring lets down, from the same steadiness report as the
//! Steadiness pane (core `steadiness`, as `timegrapher series` gives it).
//! Nothing is computed here: the curve comes ready made with the report,
//! and the walk under it (#741) is core's [`walk_view`], as the chat app's
//! card draws it.

use crate::marks;
use crate::strength::{self, Grid, Strength};
use crate::theme::{self, Icon};
use eframe::egui::{self, RichText};
use egui_plot::{Line, Plot, Points, VLine};
use timegrapher_core::steadiness::IsochronismCurve;
use timegrapher_core::steady_views::{WalkView, WALK_HINT, WALK_KEY_START, WALK_SINCE};

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
    if let Some(c) = curve {
        ui.label(RichText::new(&c.headline).small().color(pal.text_secondary));
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

/// A rate on the axis: as few decimals as the grid step needs, signed as
/// every rate is (core's `signed_text`: a zero has no sign).
fn rate_label(v: f64, step: f64) -> String {
    rate_text(v, timegrapher_core::chart_scale::axis_decimals(step))
}

/// A rate axis label with `decimals` decimals.
fn rate_text(v: f64, decimals: usize) -> String {
    format!(
        "{} s/d",
        timegrapher_core::library::signed_text(v, decimals)
    )
}

/// What the two views share, so a degree sits at one x in either: the
/// amplitude range ([`shared_amp`]) and the rate axis's width.
#[derive(Clone, Copy)]
struct Frame {
    amp: Option<Span>,
    gutter: f32,
}

/// The rate axis's width for either view: room for the widest label at
/// the ends of both views' rate ranges, never under [`GUTTER`].
fn gutter(ui: &egui::Ui, curve: Option<&IsochronismCurve>, walk: Option<&WalkView>) -> f32 {
    let spans = curve
        .and_then(curve_and_bounds)
        .map(|b| b.2)
        .into_iter()
        .chain(walk.map(|w| walk_bounds(w).1));
    let font = egui::TextStyle::Body.resolve(ui.style());
    spans
        .flat_map(|(lo, hi)| [lo, hi])
        .map(|v| {
            // With a decimal, so either view's labels fit whatever step
            // its height picks.
            let text = rate_text(v, 1);
            ui.fonts_mut(|f| {
                f.layout_no_wrap(text, font.clone(), egui::Color32::WHITE)
                    .size()
                    .x
            })
        })
        .fold(GUTTER, |g, w| g.max(w + GUTTER_PAD))
}

fn chart(ui: &mut egui::Ui, c: &IsochronismCurve, h: f32, frame: Frame) {
    let pal = theme::pal(ui);
    let Some((curve, x_bounds, y_bounds)) = curve_and_bounds(c) else {
        return;
    };
    let x_bounds = frame.amp.unwrap_or(x_bounds);
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
    ui.add_space(lane);
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
    let h = plot_h(h - lane);
    let grid = Grid::new(
        |g| theme::even_grid(g, 80.0, &[]),
        |g| theme::even_grid(g, 24.0, &[]),
    )
    .zero_y();
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
    Some((
        curve,
        amp_padded(a_lo, a_hi, c.bin_deg),
        rate_padded(lo, hi),
    ))
}

/// Amplitudes from `lo` to `hi`, padded by 5% and at least `min`, for both
/// charts.
fn amp_padded(lo: f64, hi: f64, min: f64) -> Span {
    let pad = ((hi - lo) * 0.05).max(min);
    (lo - pad, hi + pad)
}

/// Rates from `lo` to `hi`, padded by 15% as the charts are, for both
/// charts.
fn rate_padded(lo: f64, hi: f64) -> Span {
    let pad = ((hi - lo) * 0.15).max(0.05);
    (lo - pad, hi + pad)
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
        .x_axis_formatter(|m, r| {
            theme::clear_of_right_edge(format!("{:.0}°", m.value), m.value, r, 0.03)
        })
        .y_axis_formatter(|m, _| rate_label(m.value, m.step_size))
        .default_x_bounds(x_lo, x_hi)
        .default_y_bounds(y_lo, y_hi)
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

/// The walk's dots, as the chart app draws them.
const DOT_R: f32 = 3.0;
/// How near the pointer must be to a dot, in points, for its hover.
const HOVER_PX: f32 = 8.0;

/// The walk: its hint, the plot of rate against amplitude with each dot
/// coloured by its time, and the colour key, `h` high in all. `amp` is
/// the amplitude range and axis width it shares with the curve.
fn walk_part(ui: &mut egui::Ui, w: &WalkView, h: f32, frame: Frame) {
    let pal = theme::pal(ui);
    ui.label(RichText::new(WALK_HINT).small().color(pal.text_secondary));
    let key_h = ui.text_style_height(&egui::TextStyle::Small) + ui.spacing().item_spacing.y;
    let h = plot_h(h - 2.0 * key_h);
    let (x, y) = walk_bounds(w);
    let x = frame.amp.unwrap_or(x);
    let grid = Grid::new(
        |g| theme::even_grid(g, 80.0, &[]),
        |g| theme::even_grid(g, 24.0, &[]),
    )
    .zero_y();
    let resp = chart_plot("isochronism walk", &grid, (h, frame.gutter), x, y).show(ui, |p| {
        grid.draw(p, pal);
        walk_items(p, w, pal);
    });
    let at = ui
        .ctx()
        .pointer_hover_pos()
        .filter(|_| resp.response.hovered());
    if let Some(i) = at.and_then(|at| nearest_dot(w, &resp.transform, at)) {
        resp.response.on_hover_text_at_pointer(&w.dots[i].hover);
    }
    ui.horizontal(|ui| {
        let small = |t: &str| RichText::new(t).small().color(pal.text_secondary);
        ui.label(small(WALK_KEY_START));
        crate::tfmap::ramp(ui, 120.0);
        ui.label(small(&w.last_text));
        ui.label(small(WALK_SINCE));
    });
}

/// One amplitude range for the curve and the walk: the span of both.
fn shared_amp(c: &IsochronismCurve, w: &WalkView) -> Option<Span> {
    let (_, (a_lo, a_hi), _) = curve_and_bounds(c)?;
    let ((b_lo, b_hi), _) = walk_bounds(w);
    Some((a_lo.min(b_lo), a_hi.max(b_hi)))
}

/// The walk's amplitude and rate bounds, padded as the curve's are.
fn walk_bounds(w: &WalkView) -> (Span, Span) {
    let ends = |f: fn(&timegrapher_core::steady_views::WalkDot) -> f64| {
        w.dots
            .iter()
            .map(f)
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(l, h), v| {
                (l.min(v), h.max(v))
            })
    };
    let (a_lo, a_hi) = ends(|d| d.amplitude_deg);
    let (r_lo, r_hi) = ends(|d| d.rate_s_per_day);
    (
        amp_padded(a_lo, a_hi, timegrapher_core::steadiness::ISO_BIN_DEG),
        rate_padded(r_lo, r_hi),
    )
}

/// The line through the dots in time order, lifted wherever core says a
/// dot is not joined to the one before (a stretch left out or a break:
/// never filled), at axis strength under the dots; then the dots.
fn walk_items(p: &mut egui_plot::PlotUi, w: &WalkView, pal: &theme::Palette) {
    let s = strength::stroke(Strength::Axis, pal);
    for run in runs(w) {
        p.line(Line::new("walk", run).color(s.color).width(s.width));
    }
    for d in &w.dots {
        p.points(
            Points::new("walk dot", vec![[d.amplitude_deg, d.rate_s_per_day]])
                .color(crate::tfmap::viridis(d.when as f32))
                .radius(DOT_R),
        );
    }
}

/// The joined stretches of the walk, each of two dots or more.
fn runs(w: &WalkView) -> Vec<Vec<[f64; 2]>> {
    let mut out: Vec<Vec<[f64; 2]>> = Vec::new();
    for d in &w.dots {
        let pt = [d.amplitude_deg, d.rate_s_per_day];
        match out.last_mut() {
            Some(run) if d.joined => run.push(pt),
            _ => out.push(vec![pt]),
        }
    }
    out.retain(|r| r.len() > 1);
    out
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
        assert_eq!(rate_label(-0.0001, 0.5), "0.0 s/d");
        assert_eq!(rate_label(0.0, 0.5), "0.0 s/d");
        assert_eq!(rate_label(1.5, 0.5), "+1.5 s/d");
        assert_eq!(rate_label(-2.0, 1.0), "-2 s/d");

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

    /// The turn is named "Turn" above the plot, over its line, and the
    /// rate labels stay below it.
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
        let rates: Vec<_> = texts.iter().filter(|t| t.0.ends_with(" s/d")).collect();
        assert!(!rates.is_empty(), "{texts:?}");
        assert!(
            rates.iter().all(|r| r.1.top() >= turn.1.bottom()),
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
            .filter(|t| t.0.ends_with(" s/d"))
            .map(|t| t.1.top())
            .fold(f32::INFINITY, f32::min);
        assert!(top_rate.is_finite(), "{texts:?}");
        if let Some(turn) = turn {
            assert!(turn.1.top() > top_rate - 8.0, "{texts:?}");
        }
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
        let r = runs(&w);
        assert_eq!(r.iter().map(Vec::len).collect::<Vec<_>>(), [6, 3]);
        assert_eq!(r[1][0], [w.dots[6].amplitude_deg, w.dots[6].rate_s_per_day]);
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
        let first = crate::tfmap::viridis(w.dots[0].when as f32);
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
        // Each dot in viridis at its time; the last at the key's end.
        assert_eq!(w.dots.last().map(|d| d.when), Some(1.0));
        for d in &w.dots {
            assert!(
                fills.contains(&crate::tfmap::viridis(d.when as f32)),
                "{fills:?}"
            );
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
                        egui::Shape::Text(t)
                            if t.galley.text().ends_with('°') && !t.galley.text().contains(' ') =>
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
            walk_only.iter().any(|t| t == &curve.headline),
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
