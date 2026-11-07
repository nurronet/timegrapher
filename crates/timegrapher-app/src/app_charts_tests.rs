//! The Charts and Distributions panes, headless: what each plots and
//! where its values come from.

use super::tests::{painted_text, synthetic, synthetic_varying, themed};
use super::*;

/// A trend reading's beat error as core reads one (`beat_error_signed`).
fn trend_beat_error(p: &TrendPoint) -> Option<f64> {
    p.beat_error().map(|(value, _)| value)
}

/// A reading's beat error is core's (`beat_error_signed`): from the
/// unlock where it was timed, else from the drop, so a stretch with only
/// a drop still charts.
#[test]
fn a_readings_beat_error_falls_back_to_the_drop() {
    let point = |unlock, drop| TrendPoint {
        t: 0.0,
        rate_s_per_day: None,
        amplitude_deg: None,
        amplitude_even_deg: None,
        amplitude_odd_deg: None,
        beat_error_unlock_ms: unlock,
        beat_error_ms: drop,
        rate_error_s_per_day: None,
        amplitude_error_deg: None,
        beat_error_unlock_error_ms: None,
        heard: true,
    };
    assert_eq!(trend_beat_error(&point(Some(-0.4), Some(0.5))), Some(-0.4));
    assert_eq!(trend_beat_error(&point(None, Some(0.5))), Some(0.5));
    assert_eq!(trend_beat_error(&point(None, None)), None);
}

/// The rate distribution's 2 s values are the rate over each 2 seconds
/// of beats, none across a break or into a masked stretch.
#[test]
fn rate_distribution_counts_two_second_stretches() {
    let mut app = synthetic(30.0, 12.0);
    let live = app.live.as_mut().unwrap();
    live.set_breaks(vec![15.0]);
    let mut r = RateTwoS::default();
    r.refresh(live, &[(5.0, 7.5)]);
    assert!(r.values.len() >= 10, "{}", r.values.len());
    for &(a, v) in &r.values {
        let b = a + RateTwoS::STEP_S;
        assert!(!(a < 15.0 && 15.0 < b), "across the break at {a}");
        assert!(!(a < 7.5 && b > 5.0), "into the mask at {a}");
        assert!((v - 12.0).abs() < 20.0, "{v} s/d at {a}");
    }
    // Refreshing again adds nothing; a change of mask starts again.
    let n = r.values.len();
    r.refresh(live, &[(5.0, 7.5)]);
    assert_eq!(r.values.len(), n);
    r.refresh(live, &[]);
    assert!(r.values.len() > n);
    // The pane follows Values for rate too.
    app.hist_readings = false;
    let ctx = egui::Context::default();
    let _ = ctx.run(Default::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| app.histogram(ui, SeriesKind::Rate));
    });
    assert!(!app.rate_2s.values.is_empty());
}

/// The Charts and Distributions panes draw a plot for each reading
/// turned on, and its switches keep at least one on.
#[test]
fn the_charts_and_distributions_draw_the_readings_turned_on() {
    let mut app = synthetic(30.0, 20.0);
    for p in [Pane::Charts, Pane::Distributions] {
        set_pane_visible(&mut app.panes.tiles, p, true);
    }
    // Room for every plot: the strip, the profile and the Scope out of
    // the way.
    for p in [Pane::Strip, Pane::Sound, Pane::Scope] {
        set_pane_visible(&mut app.panes.tiles, p, false);
    }
    let ctx = themed();
    // Each pane in turn: they are tabs side by side, one on show.
    let draw = |app: &mut TimegrapherApp| {
        let mut all = String::new();
        for p in [Pane::Charts, Pane::Distributions] {
            for q in [Pane::Charts, Pane::Distributions] {
                set_pane_visible(&mut app.panes.tiles, q, q == p);
            }
            let mut text = String::new();
            for _ in 0..3 {
                let input = egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        Vec2::new(1440.0, 1000.0),
                    )),
                    ..Default::default()
                };
                text = painted_text(&ctx.run(input, |ctx| app.panels(ctx)));
            }
            all.push_str(&text);
        }
        all
    };
    // At first only the rate: no amplitude or beat error plot in
    // either pane.
    assert_eq!((app.chart_series, app.dist_series), (RATE_ONLY, RATE_ONLY));
    let text = draw(&mut app);
    assert!(text.contains("Rate"), "{text}");
    for t in ["Beat Error:", "Amplitude:", "From the Unlock", "From Tick"] {
        assert!(!text.contains(t), "{t}: {text}");
    }
    // All three turned on.
    app.chart_series = [true; 3];
    app.dist_series = [true; 3];
    let text = draw(&mut app);
    for t in [
        "Charts",
        "Distributions",
        "s/d",
        "Beat Error:",
        "Amplitude:",
    ] {
        assert!(text.contains(t), "{t}: {text}");
    }
    // Rates in s/d, as everywhere else (#815).
    assert!(!text.contains("s/day"), "{text}");
    assert!(!text.contains("Match Strip"), "{text}");
    app.chart_series = RATE_ONLY;
    // The last one on stays on; another comes on beside it.
    steady::flip_series(&mut app.chart_series, 0);
    assert_eq!(app.chart_series, [true, false, false]);
    steady::flip_series(&mut app.chart_series, 2);
    assert_eq!(app.chart_series, [true, false, true]);
    steady::flip_series(&mut app.chart_series, 0);
    assert_eq!(app.chart_series, [false, false, true]);
}

/// No two words a Distributions plot draws lie on one another: the first
/// value along the bottom stays clear of the lowest share up the side.
#[test]
fn a_distributions_axis_labels_stay_apart() {
    // Readings spread across a second a day, as a watch's are: each 10 s
    // reading's rate steps to another level.
    let spread = |t: f64| -12.45 + 0.7 * (((t / 10.0) as usize * 7) % 30) as f64 / 29.0;
    let mut app = synthetic_varying(300.0, 12.0, spread);
    app.hist_readings = true;
    app.update_dist_readings(None);
    let ctx = themed();
    fn texts(shape: &egui::Shape, into: &mut Vec<(String, egui::Rect)>) {
        match shape {
            egui::Shape::Text(t) => into.push((
                t.galley.text().to_string(),
                t.galley.rect.translate(t.pos.to_vec2()),
            )),
            egui::Shape::Vec(v) => v.iter().for_each(|s| texts(s, into)),
            _ => {}
        }
    }
    for w in [360.0, 700.0, 1400.0] {
        let mut all = Vec::new();
        for _ in 0..3 {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    Vec2::new(w, 260.0),
                )),
                ..Default::default()
            };
            let out = ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| app.histogram(ui, SeriesKind::Rate));
            });
            all.clear();
            out.shapes.iter().for_each(|c| texts(&c.shape, &mut all));
        }
        assert!(all.iter().any(|(t, _)| t.ends_with('%')), "{all:?}");
        assert!(all.iter().any(|(t, _)| t.ends_with("s/d")), "{all:?}");
        // None runs off the plot's right end either.
        for (t, r) in &all {
            assert!(r.right() <= w, "at {w}: {t:?} {r:?} runs off the pane");
        }
        for (i, (a, ra)) in all.iter().enumerate() {
            for (b, rb) in &all[i + 1..] {
                assert!(
                    !ra.shrink(0.5).intersects(rb.shrink(0.5)),
                    "at {w}: {a:?} {ra:?} lies on {b:?} {rb:?}"
                );
            }
        }
    }
}

/// A chart's scale fits its lines on its own, so neither Ctrl and the
/// wheel (a zoom) nor a drag up or down moves it: they would only be put
/// back, and the chart jittered.
#[test]
fn a_charts_scale_neither_zooms_nor_pans() {
    let mut app = synthetic(60.0, 20.0);
    set_pane_visible(&mut app.panes.tiles, Pane::Charts, true);
    for p in [Pane::Strip, Pane::Sound, Pane::Scope, Pane::Distributions] {
        set_pane_visible(&mut app.panes.tiles, p, false);
    }
    let ctx = themed();
    let pass = |app: &mut TimegrapherApp, events: Vec<egui::Event>| {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                Vec2::new(1440.0, 1000.0),
            )),
            events,
            ..Default::default()
        };
        ctx.run(input, |ctx| app.panels(ctx))
    };
    let mut out = pass(&mut app, Vec::new());
    for _ in 0..3 {
        out = pass(&mut app, Vec::new());
    }
    // The pointer in the Rate chart, right of a scale label.
    let label = out
        .shapes
        .iter()
        .find_map(|c| match &c.shape {
            egui::Shape::Text(t) if t.galley.text() == "20.00 s/d" => Some(t.pos),
            _ => None,
        })
        .unwrap_or_else(|| panic!("{}", painted_text(&out)));
    let at = label + Vec2::new(300.0, 0.0);
    // The scale's labels in the frames after `events`: the same in each,
    // so nothing jitters.
    let settle = |app: &mut TimegrapherApp, events: Vec<egui::Event>| {
        let mut text = painted_text(&pass(app, events));
        let mut each = Vec::new();
        for _ in 0..3 {
            each.push(text);
            text = painted_text(&pass(app, vec![egui::Event::PointerMoved(at)]));
        }
        each.push(text.clone());
        // The scale's labels, not the values box's "name: value" lines.
        let labels = |t: &str| -> Vec<String> {
            t.lines()
                .filter(|l| l.ends_with(" s/d") && !l.contains(": "))
                .map(String::from)
                .collect()
        };
        assert!(
            each.iter().all(|t| labels(t) == labels(&text)),
            "the scale moved and came back"
        );
        let scale: Vec<String> = text
            .lines()
            .filter(|l| l.ends_with(" s/d") && !l.contains(": "))
            .map(String::from)
            .collect();
        assert!(scale.len() > 2, "{text}");
        scale
    };
    let before = settle(&mut app, vec![egui::Event::PointerMoved(at)]);
    let zoomed = settle(&mut app, vec![egui::Event::Zoom(2.0)]);
    assert_eq!(zoomed, before, "Ctrl and the wheel left the scale");
    let press = |pressed, pos| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    let below = at + Vec2::new(0.0, 80.0);
    pass(&mut app, vec![press(true, at)]);
    for k in 1..=8 {
        let p = at + Vec2::new(0.0, 10.0 * k as f32);
        pass(&mut app, vec![egui::Event::PointerMoved(p)]);
    }
    pass(&mut app, vec![press(false, below)]);
    let dragged = settle(&mut app, Vec::new());
    assert_eq!(dragged, before, "a drag up or down left the scale");
}

/// On the Charts, the wheel or a swipe pans time, and Cmd or Ctrl with
/// the wheel (a zoom) zooms it about the pointer (Play Head rule 10).
#[test]
fn the_charts_wheel_pans_time_and_a_zoom_zooms_it() {
    let mut app = synthetic(120.0, 20.0);
    set_pane_visible(&mut app.panes.tiles, Pane::Charts, true);
    for p in [Pane::Strip, Pane::Sound, Pane::Scope, Pane::Distributions] {
        set_pane_visible(&mut app.panes.tiles, p, false);
    }
    let ctx = themed();
    let pass_held = |app: &mut TimegrapherApp, events: Vec<egui::Event>, modifiers| {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                Vec2::new(1440.0, 1000.0),
            )),
            events,
            modifiers,
            ..Default::default()
        };
        ctx.run(input, |ctx| app.panels(ctx))
    };
    let pass = |app: &mut TimegrapherApp, events| pass_held(app, events, egui::Modifiers::NONE);
    let mut out = pass(&mut app, Vec::new());
    for _ in 0..3 {
        out = pass(&mut app, Vec::new());
    }
    let label = out
        .shapes
        .iter()
        .find_map(|c| match &c.shape {
            egui::Shape::Text(t) if t.galley.text() == "20.00 s/d" => Some(t.pos),
            _ => None,
        })
        .unwrap_or_else(|| panic!("{}", painted_text(&out)));
    let at = label + Vec2::new(500.0, 0.0);
    // The time axis's labels a few frames after `events`, the keys a
    // wheel turn in them was made with held throughout, as a hand holds
    // them while the turn plays out.
    let look = |app: &mut TimegrapherApp, events: Vec<egui::Event>| {
        let held = events
            .iter()
            .find_map(|e| match e {
                egui::Event::MouseWheel { modifiers, .. } => Some(*modifiers),
                _ => None,
            })
            .unwrap_or_default();
        pass_held(app, events, held);
        let mut text = String::new();
        for _ in 0..4 {
            text = painted_text(&pass_held(app, vec![egui::Event::PointerMoved(at)], held));
        }
        text
    };
    let before = look(&mut app, vec![egui::Event::PointerMoved(at)]);
    let wheel = |dy: f32, modifiers| egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: Vec2::new(0.0, dy),
        modifiers,
    };
    let span = |l: &[f64]| l[l.len() - 1] - l[0];
    // The whole session has nowhere to pan to.
    let whole = look(&mut app, vec![wheel(60.0, egui::Modifiers::NONE)]);
    assert_eq!(time_labels(&whole), time_labels(&before));
    // A zoom (Cmd or Ctrl and the wheel, or a pinch): a shorter time.
    let zoomed = look(&mut app, vec![egui::Event::Zoom(2.0)]);
    let (b, z) = (time_labels(&before), time_labels(&zoomed));
    assert!(span(&z) < span(&b) * 0.75, "{b:?} {z:?}");
    // Scrolled forward: later times, over the same length. A turn that
    // pans past a label's step (5 s) wherever the zoom left the chart.
    let panned = look(&mut app, vec![wheel(-200.0, egui::Modifiers::NONE)]);
    let p = time_labels(&panned);
    assert!(p[0] > z[0], "{z:?} {p:?}");
    assert!((span(&p) - span(&z)).abs() <= 10.0, "{z:?} {p:?}");
    // With Shift too, the same, back.
    let shifted = look(&mut app, vec![wheel(200.0, egui::Modifiers::SHIFT)]);
    assert!(time_labels(&shifted)[0] < p[0], "{p:?} {shifted:?}");
    // Alt and the wheel (the strip's width) does nothing here.
    let alt = egui::Modifiers {
        alt: true,
        ..Default::default()
    };
    // Once the Shift turn's smooth scroll has played out.
    look(&mut app, Vec::new());
    let settled = look(&mut app, Vec::new());
    let still = look(&mut app, vec![wheel(-60.0, alt)]);
    assert_eq!(time_labels(&still), time_labels(&settled));
    // Panned far past either end, the chart stops at the take's first or
    // last reading, its length kept.
    for dy in [-6000.0, 6000.0] {
        let end = time_labels(&look(&mut app, vec![wheel(dy, egui::Modifiers::NONE)]));
        assert!(end[0] >= 0.0 && end[end.len() - 1] <= 120.0, "{end:?}");
        assert!((span(&end) - span(&z)).abs() <= 10.0, "{z:?} {end:?}");
        look(&mut app, Vec::new());
        assert!(!app.charts_fit, "{end:?}");
    }
    // Zoomed out past the take, it shows the whole session again.
    let out = look(&mut app, vec![egui::Event::Zoom(0.1)]);
    assert_eq!(time_labels(&out), b);
    assert!(app.charts_fit);
    // The pane's one Fit, and no button on any chart (#1063).
    assert!(!out
        .lines()
        .any(|l| l == "Show Whole Session" || l == "Follow Live"));
}

fn time_labels(text: &str) -> Vec<f64> {
    text.lines()
        .skip_while(|l| *l != "Charts")
        .filter_map(|l| {
            let (m, s) = l.split_once(':')?;
            Some(m.parse::<f64>().ok()? * 60.0 + s.parse::<f64>().ok()?)
        })
        .collect()
}

/// The values box writes a point to the decimals its card does (Walter's
/// walk of #1057, #1063): an amplitude of 269.5° with a ± of 0.3° reads
/// 269.5, not 270; with no ± it takes the base decimals; a rate keeps
/// its sign and a true minus.
#[test]
fn the_values_box_writes_figures_as_the_cards_do() {
    use super::charts::line_value;
    use timegrapher_core::figure_text::base_digits;
    let point = |t, amp, se| TrendPoint {
        t,
        rate_s_per_day: Some(-1.234),
        amplitude_deg: Some(amp),
        amplitude_even_deg: None,
        amplitude_odd_deg: None,
        beat_error_unlock_ms: None,
        beat_error_ms: None,
        rate_error_s_per_day: Some(0.04),
        amplitude_error_deg: se,
        beat_error_unlock_error_ms: None,
        heard: true,
    };
    let trend = [point(0.0, 269.46, Some(0.3)), point(10.0, 269.46, None)];
    let amp: Vec<[f64; 2]> = trend.iter().map(|p| [p.t, 269.46]).collect();
    let se: fn(&TrendPoint) -> Option<f64> = |p| p.amplitude_error_deg;
    let at = |x| line_value(&amp, se, (x, &trend), base_digits::AMPLITUDE, false);
    assert_eq!(at(1.0).as_deref(), Some("269.5"));
    assert_eq!(at(9.0).as_deref(), Some("269"));
    let rate: Vec<[f64; 2]> = trend.iter().map(|p| [p.t, -1.234]).collect();
    let se: fn(&TrendPoint) -> Option<f64> = |p| p.rate_error_s_per_day;
    let r = line_value(&rate, se, (0.0, &trend), base_digits::RATE, true);
    assert_eq!(r.as_deref(), Some("\u{2212}1.23"));
}

/// The pointer over a chart reads every chart's lines in their lanes
/// (Hugh, #1063): no box over the plot or its time labels, the time once
/// in the lane of the chart under the pointer, and nothing once it
/// leaves.
#[test]
fn the_lanes_read_the_pointers_time_on_every_chart() {
    let mut app = synthetic(60.0, 20.0);
    set_pane_visible(&mut app.panes.tiles, Pane::Charts, true);
    for p in [Pane::Strip, Pane::Sound, Pane::Scope, Pane::Distributions] {
        set_pane_visible(&mut app.panes.tiles, p, false);
    }
    app.chart_series = [true, true, false];
    let ctx = themed();
    let pass = |app: &mut TimegrapherApp, events: Vec<egui::Event>| {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                Vec2::new(1440.0, 1000.0),
            )),
            events,
            ..Default::default()
        };
        ctx.run(input, |ctx| app.panels(ctx))
    };
    let mut out = pass(&mut app, Vec::new());
    for _ in 0..3 {
        out = pass(&mut app, Vec::new());
    }
    // The Rate chart's name, the lowest "Rate" (the card's is above).
    let title = out
        .shapes
        .iter()
        .filter_map(|c| match &c.shape {
            egui::Shape::Text(t) if t.galley.text() == "Rate" => Some(t.pos),
            _ => None,
        })
        .max_by(|a, b| a.y.total_cmp(&b.y))
        .unwrap_or_else(|| panic!("{}", painted_text(&out)));
    let at = title + Vec2::new(400.0, 60.0);
    for _ in 0..3 {
        out = pass(&mut app, vec![egui::Event::PointerMoved(at)]);
    }
    let text = painted_text(&out);
    let value = |unit: &str| {
        text.lines().any(|l| {
            l.ends_with(unit)
                && (l.starts_with('+') || l.starts_with('\u{2212}') || l.starts_with('2'))
        })
    };
    assert!(value(" s/d") && value("°"), "{text}");
    assert!(!text.contains("Scroll or drag"), "{text}");
    assert!(app.charts_over.is_some(), "{text}");
    for _ in 0..3 {
        out = pass(&mut app, vec![egui::Event::PointerGone]);
    }
    assert_eq!(app.charts_over, None);
    // The card's own figures stay; the lane's three amplitudes go.
    let degrees = |t: &str| t.lines().filter(|l| l.ends_with('°')).count();
    let gone = painted_text(&out);
    assert_eq!(degrees(&gone) + 3, degrees(&text), "{gone}");
}

/// Panned to the newest reading, the charts follow new ones at the length
/// they show, as the strip does; panned back, they stay put (Hugh,
/// #1070).
#[test]
fn the_charts_panned_to_the_newest_reading_follow_new_ones() {
    let mut app = synthetic(120.0, 20.0);
    set_pane_visible(&mut app.panes.tiles, Pane::Charts, true);
    for p in [Pane::Strip, Pane::Sound, Pane::Scope, Pane::Distributions] {
        set_pane_visible(&mut app.panes.tiles, p, false);
    }
    let ctx = themed();
    let pass = |app: &mut TimegrapherApp, events: Vec<egui::Event>| {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                Vec2::new(1440.0, 1000.0),
            )),
            events,
            ..Default::default()
        };
        painted_text(&ctx.run(input, |ctx| app.panels(ctx)))
    };
    let mut text = String::new();
    for _ in 0..4 {
        text = pass(&mut app, Vec::new());
    }
    assert!(text.lines().any(|l| l == "Charts"), "{text}");
    // The pointer well inside the Rate chart.
    let at = egui::pos2(720.0, 700.0);
    let look = |app: &mut TimegrapherApp, events: Vec<egui::Event>| {
        pass(app, events);
        // Long enough for a wheel turn's smooth scroll to play out.
        let mut text = String::new();
        for _ in 0..40 {
            text = pass(app, vec![egui::Event::PointerMoved(at)]);
        }
        // The axis's labels, in order: not the lane's time at the pointer.
        let all = time_labels(&text);
        let n = 1 + all.windows(2).take_while(|w| w[1] > w[0]).count();
        all[..n].to_vec()
    };
    let wheel = |dy: f32| egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: Vec2::new(0.0, dy),
        modifiers: egui::Modifiers::NONE,
    };
    look(&mut app, vec![egui::Event::PointerMoved(at)]);
    look(&mut app, vec![egui::Event::Zoom(3.0)]);
    let end = look(&mut app, vec![wheel(-6000.0)]);
    let span = |l: &[f64]| l[l.len() - 1] - l[0];
    assert!(end[end.len() - 1] > 90.0, "{end:?}");
    // A minute more heard, a reading a second, as listening adds them
    // (the take behind them unchanged, so nothing reads them again):
    // the charts move with it, the length kept.
    let more = |app: &mut TimegrapherApp, s: usize| {
        for _ in 0..s {
            let mut p = *app.trend.last().unwrap();
            p.t += 1.0;
            app.trend.push(p);
        }
    };
    more(&mut app, 60);
    let later = look(&mut app, Vec::new());
    assert!(later[0] > end[0] + 30.0, "{end:?} {later:?}");
    assert!(
        (span(&later) - span(&end)).abs() <= 10.0,
        "{end:?} {later:?}"
    );
    // Panned back, more heard leaves them where they are.
    let back = look(&mut app, vec![wheel(600.0)]);
    assert!(back[0] < later[0], "{later:?} {back:?}");
    more(&mut app, 30);
    assert_eq!(look(&mut app, Vec::new()), back);
}

/// The strip lying across and the charts share one left gutter, fitted to
/// the widest value label rather than a fixed 76 pt, so their time axes
/// still line up (#1225).
#[test]
fn the_strip_and_the_charts_share_a_fitted_gutter() {
    let mut app = synthetic(120.0, 20.0);
    app.strip.horizontal = true;
    set_pane_visible(&mut app.panes.tiles, Pane::Charts, true);
    set_pane_visible(&mut app.panes.tiles, Pane::Strip, true);
    for p in [Pane::Sound, Pane::Scope, Pane::Distributions] {
        set_pane_visible(&mut app.panes.tiles, p, false);
    }
    let ctx = themed();
    let mut out = None;
    for _ in 0..4 {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                Vec2::new(1440.0, 1000.0),
            )),
            ..Default::default()
        };
        out = Some(ctx.run(input, |ctx| app.panels(ctx)));
    }
    let out = out.unwrap();
    let texts: Vec<(String, egui::Rect)> = out
        .shapes
        .iter()
        .filter_map(|c| match &c.shape {
            egui::Shape::Text(t) => {
                Some((t.galley.text().to_string(), c.shape.visual_bounding_rect()))
            }
            _ => None,
        })
        .collect();
    let early = texts
        .iter()
        .find(|(t, _)| t == "Early")
        .map(|(_, r)| *r)
        .unwrap_or_else(|| panic!("{}", painted_text(&out)));
    // The rate chart's value labels, in its gutter: the leftmost "s/d".
    let rate = texts
        .iter()
        .filter(|(t, _)| t.ends_with(" s/d"))
        .map(|(_, r)| *r)
        .min_by(|a, b| a.right().total_cmp(&b.right()))
        .unwrap_or_else(|| panic!("{}", painted_text(&out)));
    // Each pane's plot starts where its labels end: the strip's 4 pt on,
    // the charts' 6 pt.
    assert!(
        (early.right() + 4.0 - (rate.right() + 6.0)).abs() < 1.0,
        "{early:?} {rate:?}"
    );
    let gutter = theme::time_gutter(&ctx);
    assert!(gutter < 76.0, "{gutter}");
    let widest = texts
        .iter()
        .filter(|(t, r)| t.ends_with(" s/d") && (r.right() - rate.right()).abs() < 0.5)
        .map(|(_, r)| r.width())
        .fold(0.0, f32::max);
    assert!(gutter >= widest, "{gutter} {widest}");
    // A wide label once keeps the gutter wide, until a take is started
    // or opened (Hugh on #1239).
    let wide = egui::Rect::from_min_size(egui::Pos2::ZERO, Vec2::new(200.0, 10.0));
    let _ = ctx.run(egui::RawInput::default(), |ctx| {
        theme::note_gutter_label(ctx, wide)
    });
    let _ = ctx.run(egui::RawInput::default(), |_| {});
    assert!(theme::time_gutter(&ctx) > 200.0);
    app.reset_session(48000, "Next".into());
    let _ = ctx.run(egui::RawInput::default(), |ctx| app.panels(ctx));
    let _ = ctx.run(egui::RawInput::default(), |_| {});
    assert!(
        theme::time_gutter(&ctx) < 76.0,
        "{}",
        theme::time_gutter(&ctx)
    );
}

/// A take opened after one whose labels widened the gutter fits it
/// afresh: File > Open as well as Listen (Walter's HOLD on #1239).
#[test]
fn a_take_opened_after_a_wide_one_fits_the_gutter_afresh() {
    let base = std::env::temp_dir().join(format!("tg-app-gutter-open-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();
    let audio = timegrapher_core::synth::generate(
        &timegrapher_core::synth::SynthConfig {
            duration_s: 60.0,
            ..Default::default()
        },
        |_| 280.0,
        |_| 0.0,
    );
    let wav = base.join("narrow.wav");
    timegrapher_core::audio::write_wav(&wav, &audio).unwrap();
    let mut app = synthetic(120.0, 20.0);
    set_pane_visible(&mut app.panes.tiles, Pane::Charts, true);
    let ctx = themed();
    let frame = |app: &mut TimegrapherApp| {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                Vec2::new(1440.0, 1000.0),
            )),
            ..Default::default()
        };
        let _ = ctx.run(input, |ctx| app.panels(ctx));
    };
    // The wide take on show, its labels drawn.
    frame(&mut app);
    let wide = egui::Rect::from_min_size(egui::Pos2::ZERO, Vec2::new(200.0, 10.0));
    let _ = ctx.run(egui::RawInput::default(), |ctx| {
        theme::note_gutter_label(ctx, wide)
    });
    frame(&mut app);
    assert!(theme::time_gutter(&ctx) > 200.0);
    // Opened as File > Open does.
    app.open_recording(&wav, false);
    let t = Instant::now();
    while app.batch.is_some() && t.elapsed().as_secs() < 120 {
        // What is drawn while it opens: wide labels, as the take before's.
        frame(&mut app);
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            theme::note_gutter_label(ctx, wide)
        });
        app.poll();
    }
    assert!(app.batch.is_none(), "{:?}", app.message);
    for _ in 0..3 {
        frame(&mut app);
    }
    let gutter = theme::time_gutter(&ctx);
    assert!(gutter < 76.0, "{gutter}");
    let _ = std::fs::remove_dir_all(&base);
}

/// The stacked charts share one time axis, the take's: two whose first
/// readings differ still put 0:00 at the same place (Hugh, #1252).
#[test]
fn stacked_charts_put_the_takes_start_at_one_place() {
    let mut app = synthetic(60.0, 20.0);
    // The strip across, showing the whole session as the charts do.
    app.strip.horizontal = true;
    app.strip_fit = true;
    set_pane_visible(&mut app.panes.tiles, Pane::Charts, true);
    set_pane_visible(&mut app.panes.tiles, Pane::Strip, true);
    for p in [Pane::Sound, Pane::Scope, Pane::Distributions] {
        set_pane_visible(&mut app.panes.tiles, p, false);
    }
    // The rate read from 20 s on, the amplitude from the start.
    for p in app.trend.iter_mut().filter(|p| p.t < 20.0) {
        p.rate_s_per_day = None;
    }
    assert!(app
        .trend
        .iter()
        .any(|p| p.t < 20.0 && p.amplitude_deg.is_some()));
    app.chart_series = [true, true, false];
    let ctx = themed();
    let mut out = None;
    for _ in 0..4 {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                Vec2::new(1440.0, 1000.0),
            )),
            ..Default::default()
        };
        out = Some(ctx.run(input, |ctx| app.panels(ctx)));
    }
    let out = out.unwrap();
    // Each time label's middle: the strip's and each chart's.
    let at = |label: &str| -> Vec<f32> {
        out.shapes
            .iter()
            .filter_map(|c| match &c.shape {
                egui::Shape::Text(t) if t.galley.text() == label => {
                    Some(c.shape.visual_bounding_rect().center().x)
                }
                _ => None,
            })
            .collect()
    };
    let starts = at("0:00");
    assert_eq!(starts.len(), 2, "one 0:00 on each chart: {starts:?}");
    assert!((starts[0] - starts[1]).abs() < 0.5, "{starts:?}");
    // The strip's time axis starts where theirs does (Hugh, #1252): its
    // 0:00, a label step before its 0:10, over the charts'. (Its plot
    // ends at the newest beat, theirs at the newest reading, so the two
    // scales may differ by a fraction of a second at the far end.)
    let (tens, twenties) = (at("0:10"), at("0:20"));
    assert_eq!(tens.len(), 3, "the strip and each chart: {tens:?}");
    let strip_start = tens[0] - (twenties[0] - tens[0]);
    assert!(
        (strip_start - starts[0]).abs() < 1.5,
        "{strip_start} {starts:?}"
    );
}

/// Readings Over Time lies in the time panes' one gutter and over the
/// take's whole time, from its start, whatever the width of each chart's
/// value labels (Hugh, #1252).
#[test]
fn readings_over_time_charts_share_one_gutter() {
    // Steadiness reads a take of five minutes or more.
    let mut app = synthetic(310.0, 20.0);
    app.strip.horizontal = true;
    app.strip_fit = true;
    app.steady_series
        .insert(steady::View::Readings, [true, true, true]);
    for p in [Pane::Strip, Pane::SteadyReadings] {
        set_pane_visible(&mut app.panes.tiles, p, true);
    }
    for p in [Pane::Charts, Pane::Sound, Pane::Scope, Pane::Distributions] {
        set_pane_visible(&mut app.panes.tiles, p, false);
    }
    let ctx = themed();
    let pass = |app: &mut TimegrapherApp| {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                Vec2::new(1440.0, 1400.0),
            )),
            ..Default::default()
        };
        ctx.run(input, |ctx| app.panels(ctx))
    };
    // Read here rather than on its own thread.
    let log = steady::log_of(app.live.as_ref().unwrap()).unwrap();
    app.steady = Some((app.rev, steady::compute(&log, None)));
    let mut out = pass(&mut app);
    for _ in 0..3 {
        out = pass(&mut app);
    }
    let at = |label: &str| -> Vec<f32> {
        out.shapes
            .iter()
            .filter_map(|c| match &c.shape {
                egui::Shape::Text(t) if t.galley.text() == label => {
                    Some(c.shape.visual_bounding_rect().center().x)
                }
                _ => None,
            })
            .collect()
    };
    // A minute in, on the strip and on each of Readings Over Time's
    // charts, at one x: one gutter, one time span (the strip ends at the
    // newest beat, a fraction of a second before the newest reading).
    // Its three charts side by side, each in a column of one width: each
    // 0:00 as far into its column as the next, whatever the width of its
    // value labels (19.94 s/d against 279.1°), and at the plot's start.
    let starts = at("0:00");
    assert_eq!(starts.len(), 3, "the three charts: {starts:?}");
    let (a, b) = (starts[1] - starts[0], starts[2] - starts[1]);
    assert!((a - b).abs() < 1.0, "{starts:?}");
}

/// The strip and a time pane under it put a late time at one x, not
/// only 0:00: both end their axes at the take's one end. Charts and
/// Readings Over Time each, under the strip (Walter and Trevor on #1256).
#[test]
fn time_panes_under_the_strip_put_a_late_time_at_one_place() {
    for pane in [Pane::Charts, Pane::SteadyReadings] {
        let mut app = synthetic(370.0, 20.0);
        app.strip.horizontal = true;
        app.strip_fit = true;
        app.chart_series = [true, false, false];
        app.steady_series
            .insert(steady::View::Readings, [true, false, false]);
        for p in [Pane::Strip, pane] {
            set_pane_visible(&mut app.panes.tiles, p, true);
        }
        for p in [
            Pane::Charts,
            Pane::SteadyReadings,
            Pane::Sound,
            Pane::Scope,
            Pane::Distributions,
        ] {
            if p != pane {
                set_pane_visible(&mut app.panes.tiles, p, false);
            }
        }
        let ctx = themed();
        let pass = |app: &mut TimegrapherApp| {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    Vec2::new(1440.0, 1400.0),
                )),
                ..Default::default()
            };
            ctx.run(input, |ctx| app.panels(ctx))
        };
        // The tests last read the take a minute short of its end, as while
        // listening they read again only every so often: Readings Over
        // Time still ends where the strip does, at the take's end.
        let short = synthetic(310.0, 20.0);
        let log = steady::log_of(short.live.as_ref().unwrap()).unwrap();
        app.steady = Some((app.rev, steady::compute(&log, None)));
        let mut out = pass(&mut app);
        for _ in 0..3 {
            out = pass(&mut app);
        }
        let at = |label: &str| -> Vec<f32> {
            out.shapes
                .iter()
                .filter_map(|c| match &c.shape {
                    egui::Shape::Text(t) if t.galley.text() == label => {
                        Some(c.shape.visual_bounding_rect().center().x)
                    }
                    _ => None,
                })
                .collect()
        };
        for label in ["1:00", "4:00", "6:00"] {
            let xs = at(label);
            assert_eq!(xs.len(), 2, "{pane:?} {label}, on it and the strip: {xs:?}");
            assert!((xs[0] - xs[1]).abs() < 1.0, "{pane:?} {label}: {xs:?}");
        }
    }
}

/// Readings Over Time ends at the take's end on its own clock; joined, at
/// the end of what the tests read, even past the last window, and never
/// short of that window (Trevor on #1256).
#[test]
fn readings_over_time_ends_where_its_clock_does() {
    let app = synthetic(310.0, 20.0);
    let log = steady::log_of(app.live.as_ref().unwrap()).unwrap();
    let mut s = steady::compute(&log, None);
    let joined = steady::Opts::default();
    s.upto_s = 0.0;
    let windows = steady::readings_end(&s, joined);
    assert!(windows > 200.0, "{windows}");
    s.upto_s = windows + 60.0;
    assert_eq!(steady::readings_end(&s, joined), windows + 60.0);
    let own = steady::Opts {
        take_span: Some((0.0, 370.0)),
        ..joined
    };
    assert_eq!(steady::readings_end(&s, own), 370.0);
}

/// With a position marked, Readings Over Time reads that position's
/// stretches joined end to end, on a clock of their own: its axis says
/// whose time it is, and none says so on the take's own (Hugh on #1256).
#[test]
fn joined_readings_over_time_says_whose_time_it_is() {
    for joined in [false, true] {
        let mut app = synthetic(310.0, 20.0);
        app.strip.horizontal = true;
        app.steady_series
            .insert(steady::View::Readings, [true, false, false]);
        for p in [Pane::Strip, Pane::SteadyReadings] {
            set_pane_visible(&mut app.panes.tiles, p, true);
        }
        for p in [Pane::Charts, Pane::Sound, Pane::Scope, Pane::Distributions] {
            set_pane_visible(&mut app.panes.tiles, p, false);
        }
        if joined {
            app.brush = Some(Position::DialDown);
            let end = app.latest_s();
            app.paint_position(0.0, end);
        }
        let log = steady::log_of(app.live.as_ref().unwrap()).unwrap();
        app.steady = Some((app.rev, steady::compute(&log, None)));
        let ctx = themed();
        let mut out = None;
        for _ in 0..3 {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    Vec2::new(1440.0, 1400.0),
                )),
                ..Default::default()
            };
            out = Some(ctx.run(input, |ctx| app.panels(ctx)));
        }
        let titled =
            out.unwrap().shapes.iter().any(
                |c| matches!(&c.shape, egui::Shape::Text(t) if t.galley.text() == "Time in DD"),
            );
        assert_eq!(
            titled,
            joined,
            "joined {joined}: {:?}",
            app.session.stretches()
        );
    }
}

/// Joined, Readings Over Time keeps its own span and width: neither the
/// take's span nor the strip's width, which would line a clock of its
/// own up with the take's (Trevor on #1256).
#[test]
fn joined_readings_over_time_keeps_its_own_span_and_width() {
    let mut app = synthetic(60.0, 0.0);
    let take = app.steady_opts(steady::View::Readings);
    assert_eq!(take.take_span, app.take_span());
    assert!(take.take_span.is_some() && take.joined_in.is_none());
    app.brush = Some(Position::DialDown);
    let end = app.latest_s();
    app.paint_position(0.0, end);
    let joined = app.steady_opts(steady::View::Readings);
    assert_eq!((joined.take_span, joined.joined_in), (None, Some("DD")));
    // Under the strip lying across, only the take's clock takes its width.
    let ctx = themed();
    let _ = ctx.run(egui::RawInput::default(), |ctx| {
        crate::theme::note_time_span(ctx, egui::Rangef::new(100.0, 900.0));
        let left = 100.0 - crate::theme::time_gutter(ctx);
        assert_eq!(steady::readings_width(ctx, left, take), Some(900.0 - left));
        assert_eq!(steady::readings_width(ctx, left, joined), None);
    });
}

/// A slow watch's Distributions line gives its figures with a true minus
/// and keeps "2-second" as it is written (#1416).
#[test]
fn the_distributions_line_gives_negative_figures_a_true_minus() {
    let spread = |t: f64| 0.7 * (((t / 10.0) as usize * 7) % 30) as f64 / 29.0;
    let mut app = synthetic_varying(300.0, -12.0, spread);
    app.update_dist_readings(None);
    let ctx = themed();
    let mut text = String::new();
    for _ in 0..3 {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                Vec2::new(1400.0, 300.0),
            )),
            ..Default::default()
        };
        let out = ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| app.histogram(ui, SeriesKind::Rate));
        });
        text = painted_text(&out);
    }
    let line = text
        .lines()
        .find(|l| l.contains(" · median "))
        .unwrap_or_else(|| panic!("{text}"));
    assert!(line.contains("median \u{2212}"), "{line}");
    assert!(line.contains(" to \u{2212}"), "{line}");
    assert!(!line.contains(" -"), "{line}");
}
