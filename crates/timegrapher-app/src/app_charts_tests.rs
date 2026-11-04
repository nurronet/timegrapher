//! The Charts and Distributions panes, headless: what each plots and
//! where its values come from.

use super::tests::{painted_text, synthetic, themed};
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
    let mut app = synthetic(300.0, 12.0);
    // Readings spread across a second a day, as a watch's are.
    for (i, p) in app.trend.iter_mut().enumerate() {
        p.rate_s_per_day = Some(-0.45 + 0.7 * ((i * 7) % 30) as f64 / 29.0);
    }
    app.hist_readings = true;
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
        let labels = |t: &str| -> Vec<String> {
            t.lines()
                .filter(|l| l.ends_with(" s/d"))
                .map(String::from)
                .collect()
        };
        assert!(
            each.iter().all(|t| labels(t) == labels(&text)),
            "the scale moved and came back"
        );
        let scale: Vec<String> = text
            .lines()
            .filter(|l| l.ends_with(" s/d"))
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
    // Scrolled back: earlier times, over the same length.
    let panned = look(&mut app, vec![wheel(60.0, egui::Modifiers::NONE)]);
    let (b, p) = (time_labels(&before), time_labels(&panned));
    assert!(p[0] < b[0], "{b:?} {p:?}");
    let span = |l: &[f64]| l[l.len() - 1] - l[0];
    assert!((span(&p) - span(&b)).abs() <= 10.0, "{b:?} {p:?}");
    // With Shift too, the same.
    let shifted = look(&mut app, vec![wheel(-60.0, egui::Modifiers::SHIFT)]);
    assert!(time_labels(&shifted)[0] > p[0], "{p:?} {shifted:?}");
    // Alt and the wheel (the strip's width) does nothing here.
    let alt = egui::Modifiers {
        alt: true,
        ..Default::default()
    };
    let still = look(&mut app, vec![wheel(60.0, alt)]);
    assert_eq!(time_labels(&still), time_labels(&shifted));
    // A zoom (Cmd or Ctrl and the wheel, or a pinch): a shorter time.
    let zoomed = look(&mut app, vec![egui::Event::Zoom(2.0)]);
    let z = time_labels(&zoomed);
    assert!(span(&z) < span(&p) * 0.75, "{p:?} {z:?}");
}

/// The Charts' time axis labels, in seconds.
fn time_labels(text: &str) -> Vec<f64> {
    text.lines()
        .skip_while(|l| *l != "Charts")
        .filter_map(|l| {
            let (m, s) = l.split_once(':')?;
            Some(m.parse::<f64>().ok()? * 60.0 + s.parse::<f64>().ok()?)
        })
        .collect()
}
