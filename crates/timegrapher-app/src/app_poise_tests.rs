//! The Dynamic Poise test and its pane (#1474): set up at 9H with the
//! card at 10, led through the eight upright stops, kept out of the
//! Positions Test's pane, and drawn with core's fit.

use super::tests::{queued_capture, synthetic_varying, themed};
use super::*;

/// The Watches view's Dynamic Poise Test sets the take up on the Bench:
/// its pane shows, the card turns to 10 at 9H, and nothing starts until
/// Start Test.
#[test]
fn a_dynamic_poise_test_is_set_up_at_9h_with_the_card_at_10() {
    let mut app = TimegrapherApp::with_devices(Vec::new(), None);
    app.view = View::Watches;
    assert!(!pane_visible(&app.panes.tiles, Pane::DynamicPoise));
    assert!(!app.ten_positions);
    app.new_take(NewTake::Poise, false);
    assert_eq!(app.view, View::Bench);
    assert_eq!(app.ready_test(), Some(NewTake::Poise));
    assert!(pane_visible(&app.panes.tiles, Pane::DynamicPoise));
    assert_eq!(app.position, Position::CrownDown);
    assert_eq!(app.position.witschi(), "9H");
    assert!(
        app.ten_positions,
        "Start was pressed: the card shows the eight"
    );
    assert!(app.guided.is_none() && !app.running());
    assert_eq!(NewTake::Poise.name(), "Dynamic Poise Test");
}

/// The test turns the watch 45° at a time, the same way round, through
/// the eight upright stops, and names them by the hour at the top.
#[test]
fn the_poise_order_is_the_eight_upright_stops_45_degrees_apart() {
    let order = guided::Kind::Poise.order();
    let hours: Vec<&str> = order.iter().map(|p| p.witschi()).collect();
    assert_eq!(
        hours,
        ["9H", "7H30", "6H", "4H30", "3H", "1H30", "12H", "10H30"]
    );
    for w in order.windows(2) {
        let (a, b) = (w[0].core().top_deg(), w[1].core().top_deg());
        let step = (a.unwrap() - b.unwrap()).rem_euclid(360.0);
        assert_eq!(step, 45.0, "{:?} to {:?}", w[0], w[1]);
    }
    let core: Vec<_> = order.iter().map(|p| p.core()).collect();
    assert_eq!(core, core_session::Position::UPRIGHT, "core's order");
}

/// A rate that goes once round as the watch turns, fastest with 6H at the
/// top: the eight stops are measured, the test ends by itself after the
/// eighth, core's fit names 6H, and the Positions Test's pane and its
/// 24 hour results take no part in it.
#[test]
fn a_poise_test_measures_eight_stops_and_core_fits_the_wave() {
    let base = std::env::temp_dir().join(format!("tg-app-poise-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();
    let (t0, each, settle) = (5.0, 62.0, 2.0);
    let order = guided::Kind::Poise.order();
    // 30 s/d each way, fastest at 180° (6H at the top).
    let extra = move |t: f64| {
        let k = ((t - t0) / each).floor();
        if !(0.0..8.0).contains(&k) {
            return 0.0;
        }
        let deg = order[k as usize].core().top_deg().unwrap();
        30.0 * (deg - 180.0).to_radians().cos()
    };
    let mut app = synthetic_varying(t0 + 8.0 * each + 5.0, 4.0, extra);
    app.session.listen();
    let tx = queued_capture(&mut app, &base);
    let mut test = guided::Test::poise().timed(settle, each - settle);
    let mut t = t0;
    test.tick(t);
    for i in 0..order.len() {
        t += each;
        assert_eq!(test.tick(t), guided::Phase::Turn, "stop {i}");
        if i + 1 < order.len() {
            assert_eq!(test.advance(t), Some(order[i + 1]));
        }
    }
    app.guided = Some(test);
    assert!(app.test_listening());
    assert!(app.positions_test().is_none(), "not the Positions Test's");
    let ctx = egui::Context::default();
    app.guided_tick(&ctx);
    assert!(!app.guided_running(), "ended after the eighth");
    let kept = app.poise_test().expect("the table stays");
    let readings = kept.readings.as_ref().unwrap();
    assert_eq!(readings.len(), 8);
    // The Mean row is over all eight, the 45° stops too.
    let table = guided::table(readings, guided::Kind::Poise);
    let rates: Vec<f64> = table.rows.iter().filter_map(|r| r.1).collect();
    assert_eq!(rates.len(), 8);
    let mean = rates.iter().sum::<f64>() / 8.0;
    assert!(
        (table.mean_rate.unwrap() - mean).abs() < 1e-9,
        "{:?}",
        table.mean_rate
    );
    assert!(kept.kept(&app.guided_about()).is_none(), "no 24 hour half");
    let fit = guided::poise_fit(readings).expect("fitted from eight");
    assert_eq!(fit.positions, 8);
    assert!(fit.wave_clear, "{}", fit.text);
    assert!(
        fit.short_text.contains("fastest near 6H at the top"),
        "{}",
        fit.short_text
    );
    assert!((fit.wave.size.value - 30.0).abs() < 3.0, "{}", fit.text);
    // Copy: the table in the pane's order, Mean, no Largest Difference,
    // then core's sentence.
    let text = guided::export_text(kept, &app.guided_about(), '\t');
    assert!(text.contains("Crown Down (CD, 9H)\t"), "{text}");
    assert!(
        text.contains("\nMean\t") && text.contains("From Mean (s/d)"),
        "{text}"
    );
    assert!(!text.contains("Largest Difference") && !text.contains("Vertical less"));
    assert!(text.trim_end().ends_with(&fit.text), "{text}");
    drop(tx);
    let _ = std::fs::remove_dir_all(&base);
}

/// The pane draws a test part way, before five stops are in: the table's
/// rows in the stand's order, core's words for the fit to come, and in
/// either view no wave yet. The Positions Test's pane keeps to its own.
#[test]
fn the_pane_says_the_fit_waits_for_five_stops_in_both_views() {
    let mut app = synthetic_varying(30.0, 0.0, |_| 0.0);
    app.guided = Some(guided::Test::poise().timed(1.0, 5.0));
    for view in [PoiseView::Plot, PoiseView::Dial] {
        app.poise_view = view;
        let drawn = draw(&mut app, Pane::DynamicPoise);
        assert!(drawn.contains(&poise_pending()), "{view:?}: {drawn}");
        for label in ["Crown Down (CD, 9H)", "7:30 Up (7H30)", "10:30 Up (10H30)"] {
            assert!(drawn.contains(label), "{label}");
        }
        assert!(drawn.contains("Hour at the top") == (view == PoiseView::Plot));
        assert!(!drawn.contains("Fastest"), "no wave yet");
    }
    let positions = draw(&mut app, Pane::PositionsTest);
    assert!(!positions.contains("Crown Down (CD, 9H)"), "{positions}");
}

/// Info opens core's note on the band near 220°.
#[test]
fn info_shows_cores_note_on_the_band() {
    let mut app = TimegrapherApp::with_devices(Vec::new(), None);
    let note = timegrapher_core::poise::null_band_note();
    assert!(!draw(&mut app, Pane::DynamicPoise).contains(&note));
    app.poise_info = true;
    assert!(draw(&mut app, Pane::DynamicPoise).contains(&note));
}

fn poise_pending() -> String {
    timegrapher_core::poise::pending_text()
}

/// The pane's toolbar and body drawn twice at 1280 by 820, its text.
fn draw(app: &mut TimegrapherApp, pane: Pane) -> String {
    let ctx = themed();
    let mut drawn = String::new();
    for _ in 0..2 {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                Vec2::new(1280.0, 820.0),
            )),
            ..Default::default()
        };
        let out = ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                app.pane_tools(ui, pane);
                match pane {
                    Pane::DynamicPoise => app.poise_pane(ui),
                    _ => app.positions_pane(ui),
                }
            });
        });
        drawn = out
            .shapes
            .iter()
            .filter_map(|c| match &c.shape {
                egui::Shape::Text(t) => Some(t.galley.text().to_string()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n");
    }
    drawn
}
