//! The ERP in the app: its place beside Watches, Settings > ERP, the ERP
//! view, and what the ERP's answers do to the window. The calls go to a
//! stand-in ERP on this computer (`timegrapher_erp::testing`) that checks the
//! key, the PIN, the sign-in token and the station's signature, as the ERP
//! does.

use super::erp::{results_from_table, Reply, FIELD_W};
use super::tests::{painted_text, screen, themed};
use super::*;
use std::time::{Duration, Instant, SystemTime};
use timegrapher_erp::client::{Identity, LoginOutcome};
use timegrapher_erp::config::Config;
use timegrapher_erp::operator::OperatorSession;
use timegrapher_erp::testing::{start_erp, Erp, KEY, PIN, SECRET};
use zeroize::Zeroizing;

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tg-app-erp-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

/// An app whose ERP settings live in `dir`, with nothing saved there.
fn bare_app(dir: &Path) -> TimegrapherApp {
    let mut app = TimegrapherApp::with_devices(Vec::new(), None);
    app.erp.dir = Some(dir.to_path_buf());
    app
}

/// An app set up for the stand-in ERP, its settings saved and read back.
fn set_up(name: &str, erp: &Erp) -> (TimegrapherApp, PathBuf) {
    let dir = temp_dir(name);
    let config = Config {
        base_url: erp.url.clone(),
        api_key: KEY.into(),
        api_secret: SECRET.into(),
        station_id: "STATION-01".into(),
        verify_tls: true,
        enhanced: false,
    };
    config.save(&dir).unwrap();
    let mut app = bare_app(&dir);
    app.erp.ensure_loaded();
    assert!(app.erp.configured());
    // the first check of the connection, as the window makes it
    finish(&mut app);
    assert!(app.erp.live(), "{:?}", app.erp.status);
    (app, dir)
}

/// Lets the call under way finish, as the window's frames would.
fn finish(app: &mut TimegrapherApp) {
    let ctx = egui::Context::default();
    for _ in 0..1000 {
        app.erp_poll(&ctx);
        if !app.erp.busy() {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("the call to the ERP never answered");
}

fn frame_text(app: &mut TimegrapherApp) -> String {
    let ctx = themed();
    let mut text = String::new();
    for _ in 0..3 {
        text = painted_text(&ctx.run(screen(), |ctx| app.panels(ctx)));
    }
    text
}

fn identity(id: &str) -> Identity {
    Identity {
        found: true,
        identity_id: Some(id.to_string()),
        ..Identity::default()
    }
}

/// A Positions Test table with Dial Up and Crown Up read, Dial Down marked
/// but short, and the rest not marked.
fn table() -> guided::FreeTable {
    use core_session::Position;
    let label = |p: Position| guided::position_label(p);
    guided::FreeTable {
        rows: vec![
            (
                label(Position::CH),
                61.0,
                guided::FreeCell::Read(Some(12.3456789), Some(281.5), Some(0.31)),
            ),
            (label(Position::CB), 4.0, guided::FreeCell::Short),
            (label(Position::H9), 0.0, guided::FreeCell::NotMarked),
            (label(Position::H6), 0.0, guided::FreeCell::NotMarked),
            (
                label(Position::H3),
                45.0,
                guided::FreeCell::Read(Some(-3.5), None, Some(0.2)),
            ),
            (label(Position::H12), 0.0, guided::FreeCell::NotMarked),
        ],
        details: Vec::new(),
        indices: None,
        mean: [None; 3],
        largest: [None; 3],
        read: 2,
    }
}

#[test]
fn erp_is_the_third_view_after_watches_with_its_own_key() {
    let names: Vec<&str> = VIEWS.iter().map(|v| v.1).collect();
    assert_eq!(names, ["Bench", "Watches", "ERP"]);
    let c = menus::MenuCommand::View(View::Erp);
    assert!(menus::MenuCommand::WITH_KEYS.contains(&c));
    let key = menus::menu_shortcut(c).unwrap();
    assert_eq!(key.logical_key, egui::Key::Num3);
    assert_eq!(key.modifiers, egui::Modifiers::COMMAND);
}

#[test]
fn the_view_and_its_key_work_like_the_other_views() {
    let mut app = bare_app(&temp_dir("keys"));
    let ctx = themed();
    let _ = ctx.run(screen(), |ctx| app.panels(ctx));
    app.run_menu_command(&ctx, menus::MenuCommand::View(View::Erp));
    assert_eq!(app.view, View::Erp);
    // The panes are the Bench's: dimmed here, as in Watches.
    assert!(!app.menu_enabled(menus::MenuCommand::ResetPanes));
    assert!(app.menu_checked(menus::MenuCommand::View(View::Erp)));
}

#[test]
fn settings_has_an_erp_category_last() {
    assert_eq!(settings_window::TABS.last().unwrap().0, Tab::Erp);
    assert_eq!(Tab::from_key(Tab::Erp.key()), Tab::Erp);
    let keys: std::collections::BTreeSet<&str> =
        settings_window::TABS.iter().map(|t| t.0.key()).collect();
    assert_eq!(keys.len(), settings_window::TABS.len());
}

#[test]
fn an_erp_not_set_up_says_so_and_offers_its_settings() {
    let mut app = bare_app(&temp_dir("unset"));
    app.view = View::Erp;
    let text = frame_text(&mut app);
    assert!(text.contains("The ERP is not set up yet."), "{text}");
    assert!(text.contains("Open ERP Settings"), "{text}");
    assert!(!text.contains("Send to ERP"), "{text}");
}

#[test]
fn the_erp_settings_page_has_the_connection_the_key_and_the_operator() {
    let mut app = bare_app(&temp_dir("page"));
    app.open_settings(Tab::Erp);
    let text = frame_text(&mut app);
    for want in [
        "Connection",
        "ERP Address",
        "API Key",
        "API Secret",
        "Station ID",
        "Check Certificate",
        "Save",
        "Test Connection",
        "Station Key",
        "Operator",
        "Operator ID",
        "PIN",
        "Sign In",
    ] {
        assert!(text.contains(want), "{want}: {text}");
    }
}

#[test]
fn a_set_up_erp_view_has_search_identity_note_and_send() {
    let erp = start_erp();
    let (mut app, dir) = set_up("view", &erp);
    app.view = View::Erp;
    let text = frame_text(&mut app);
    for want in [
        "Matches",
        "Identity",
        "Note",
        "Send to ERP",
        "Send 0 Positions",
    ] {
        assert!(text.contains(want), "{want}: {text}");
    }
    assert!(!text.contains("not set up"), "{text}");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn settings_are_saved_only_when_complete_and_safe() {
    let dir = temp_dir("save");
    let mut app = bare_app(&dir);
    app.erp.ensure_loaded();
    app.erp.form.base_url = "http://erp.example.com".into();
    app.erp.form.api_key = "k".into();
    app.erp.form.api_secret = "s".into();
    app.erp.form.station_id = "S1".into();
    app.erp.save_settings();
    assert!(!app.erp.status.as_ref().unwrap().ok, "an http address");
    assert!(!app.erp.configured());
    app.erp.form.base_url = "https://erp.example.com/".into();
    app.erp.save_settings();
    assert!(app.erp.status.as_ref().unwrap().ok);
    assert!(app.erp.configured());
    assert_eq!(app.erp.config.base_url, "https://erp.example.com");
    // what the command line program reads
    assert_eq!(Config::load(&dir).unwrap().station_id, "S1");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn test_connection_says_who_the_erp_thinks_the_station_is() {
    let erp = start_erp();
    let (mut app, dir) = set_up("test", &erp);
    let ctx = egui::Context::default();
    app.erp.test_connection(&ctx);
    finish(&mut app);
    let note = app.erp.status.clone().unwrap();
    assert!(note.ok, "{note:?}");
    assert!(note.text.contains("station@example.test"), "{note:?}");
    assert!(note.text.contains("1.5.0"), "{note:?}");
    // a wrong secret is reported in words, with the ERP's page left out
    app.erp.form.api_secret = "wrong".into();
    app.erp.test_connection(&ctx);
    finish(&mut app);
    let note = app.erp.status.clone().unwrap();
    assert!(!note.ok && note.text.contains("API key"), "{note:?}");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_search_waits_for_the_field_to_rest_and_finds_what_the_erp_has() {
    let erp = start_erp();
    let (mut app, dir) = set_up("search", &erp);
    let ctx = egui::Context::default();
    // One character is not searched for; nothing is asked.
    app.erp.query = "I".into();
    app.erp.typed_at = Some(Instant::now() - Duration::from_secs(5));
    app.erp.search_when_rested(&ctx);
    assert!(!app.erp.busy());
    // Just typed: not yet.
    app.erp.query = "ID-1".into();
    app.erp.typed_at = Some(Instant::now());
    app.erp.search_when_rested(&ctx);
    assert!(!app.erp.busy());
    // Rested: asked.
    app.erp.typed_at = Some(Instant::now() - Duration::from_secs(5));
    app.erp.search_when_rested(&ctx);
    assert!(app.erp.busy());
    finish(&mut app);
    assert_eq!(app.erp.results.len(), 1);
    assert_eq!(app.erp.searched, "ID-1");
    // The same text is not asked again.
    app.erp.search_when_rested(&ctx);
    assert!(!app.erp.busy());
    // Picking a match asks for the whole identity and what the ERP recommends.
    app.erp.look_up(&ctx, "ID-1".into());
    finish(&mut app);
    let picked = app.erp.picked.clone().unwrap();
    assert_eq!(picked.ln_part.as_deref(), Some("ETA 2824-2"));
    assert_eq!(app.erp.measurement_type, "Follow-up Certification");
    assert_eq!(app.erp.qa_standard, "Workshop");
    // an identity the ERP doesn't know changes nothing and says so
    app.erp.look_up(&ctx, "nope".into());
    finish(&mut app);
    assert!(!app.erp.view_note.as_ref().unwrap().ok);
    assert_eq!(
        app.erp.picked.as_ref().unwrap().identity_id.as_deref(),
        Some("ID-1")
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn an_erp_that_is_not_set_up_is_never_asked() {
    let mut app = bare_app(&temp_dir("never"));
    app.erp.ensure_loaded();
    app.erp.query = "ID-1".into();
    app.erp.typed_at = Some(Instant::now() - Duration::from_secs(5));
    app.erp.search_when_rested(&egui::Context::default());
    assert!(!app.erp.busy());
}

#[test]
fn signing_in_wipes_the_pin_and_signing_out_forgets_the_token() {
    let erp = start_erp();
    let (mut app, dir) = set_up("signin", &erp);
    let ctx = egui::Context::default();
    app.erp.operator_id = " OP1 ".into();
    app.erp.pin = Zeroizing::new("111111".to_string());
    app.erp.sign_in(&ctx);
    assert!(app.erp.pin.is_empty(), "the PIN stayed in the window");
    finish(&mut app);
    // a wrong PIN: the ERP's own words, and nobody is signed in
    let note = app.erp.operator_note.clone().unwrap();
    assert!(
        !note.ok && note.text == "The Operator ID or PIN is wrong.",
        "{note:?}"
    );
    assert!(!app.erp.signed_in());
    app.erp.pin = Zeroizing::new(PIN.to_string());
    app.erp.sign_in(&ctx);
    assert!(app.erp.pin.is_empty());
    finish(&mut app);
    assert!(app.erp.signed_in());
    assert_eq!(
        app.erp.operator_note.as_ref().unwrap().text,
        "Signed in as Op One."
    );
    // the PIN went only to the sign-in call
    let log = erp.state.lock().unwrap().log.clone();
    for (name, _, body) in &log {
        if name != "operator_login" {
            assert!(!body.contains(PIN), "{name}");
        }
    }
    assert!(log
        .iter()
        .any(|(n, _, b)| n == "operator_login" && b.contains(PIN)));
    app.erp.sign_out(&ctx);
    assert!(app.erp.operator.is_none() && !app.erp.signed_in());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_sign_in_past_its_limits_is_not_signed_in() {
    let mut app = bare_app(&temp_dir("stale"));
    let long_ago = SystemTime::now() - Duration::from_secs(9 * 3600);
    app.erp.operator = Some(OperatorSession::new(
        "t".into(),
        "OP1".into(),
        "Op".into(),
        long_ago,
    ));
    assert!(!app.erp.signed_in());
    app.erp.operator = Some(OperatorSession::new(
        "t".into(),
        "OP1".into(),
        "Op".into(),
        SystemTime::now(),
    ));
    assert!(app.erp.signed_in());
}

#[test]
fn the_key_is_made_and_registered_once_from_settings() {
    let erp = start_erp();
    let (mut app, dir) = set_up("key", &erp);
    let ctx = egui::Context::default();
    app.erp.make_key(&ctx);
    finish(&mut app);
    let note = app.erp.key_note.clone().unwrap();
    assert!(
        note.ok && note.text.contains("made and registered"),
        "{note:?}"
    );
    assert!(timegrapher_erp::register::is_registered_as(
        &dir,
        "STATION-01-workstation"
    ));
    // again: not registered a second time
    app.erp.make_key(&ctx);
    finish(&mut app);
    let registrations = erp
        .state
        .lock()
        .unwrap()
        .log
        .iter()
        .filter(|(n, _, _)| n == "register_key")
        .count();
    assert_eq!(registrations, 1);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn the_positions_test_goes_to_the_erp_signed_attested_with_the_note() {
    let erp = start_erp();
    let (mut app, dir) = set_up("send", &erp);
    let ctx = egui::Context::default();
    // sign in, pick the watch, write a note
    app.erp.operator_id = "OP1".into();
    app.erp.pin = Zeroizing::new(PIN.to_string());
    app.erp.sign_in(&ctx);
    finish(&mut app);
    app.erp.look_up(&ctx, "ID-1".into());
    finish(&mut app);
    app.erp.note = "Check the rate again after regulating".into();
    let results = results_from_table(&table(), Some(28800), Some(52.0), &app.erp.note);
    assert_eq!(app.erp_send_blocker(&results), None);
    app.erp.send(&ctx, results);
    finish(&mut app);
    assert_eq!(
        app.erp.sent,
        [
            "Dial Up: sent, Attested",
            "Crown Up: sent, Attested",
            "Suggested grade: B - Serviceable. Confirm it in the ERP."
        ]
    );
    assert!(app.erp.view_note.as_ref().unwrap().ok);
    // the note went with the results, so it is gone; the sign-in came back
    assert!(app.erp.note.is_empty());
    assert!(app.erp.signed_in());
    // one ERP session for both; the note and the figures are in the body
    let state = erp.state.lock().unwrap();
    assert_eq!(state.sessions, 1);
    let bodies: Vec<serde_json::Value> = state
        .log
        .iter()
        .filter(|(n, _, _)| n == "submit_timing_result")
        .map(|(_, _, b)| serde_json::from_str(b).unwrap())
        .collect();
    assert_eq!(bodies.len(), 2);
    assert!(bodies[0]["notes"]
        .as_str()
        .unwrap()
        .starts_with("Check the rate again"));
    assert_eq!(bodies[0]["measurements"]["rate_s_per_day"], 12.345679);
    assert_eq!(bodies[0]["measurements"]["beat_frequency_bph"], 28800.0);
    assert_eq!(bodies[0]["operator_id"], "OP1");
    assert_eq!(bodies[1]["position"], "Crown Up");
    drop(state);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn without_a_sign_in_the_results_are_invalid_and_the_view_says_so() {
    let erp = start_erp();
    let (mut app, dir) = set_up("send-invalid", &erp);
    let ctx = egui::Context::default();
    app.erp.look_up(&ctx, "ID-1".into());
    finish(&mut app);
    app.view = View::Erp;
    let text = frame_text(&mut app);
    assert!(text.contains("record these as Invalid"), "{text}");
    let results = results_from_table(&table(), None, None, "");
    app.erp.send(&ctx, results);
    finish(&mut app);
    assert_eq!(app.erp.sent[0], "Dial Up: sent, Invalid");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_failed_send_keeps_the_note_and_says_why() {
    let erp = start_erp();
    let (mut app, dir) = set_up("send-fail", &erp);
    let ctx = egui::Context::default();
    app.erp.look_up(&ctx, "ID-1".into());
    finish(&mut app);
    // the ERP is gone
    app.erp.config.base_url = "http://127.0.0.1:1".into();
    app.erp.note = "keep me".into();
    app.erp
        .send(&ctx, results_from_table(&table(), None, None, "keep me"));
    finish(&mut app);
    assert!(!app.erp.view_note.as_ref().unwrap().ok);
    assert_eq!(app.erp.note, "keep me");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn only_the_positions_with_a_reading_are_sent_with_core_s_names_and_figures() {
    let results = results_from_table(&table(), Some(28800), Some(52.0), "");
    let names: Vec<&str> = results.iter().map(|r| r.position).collect();
    assert_eq!(names, ["Dial Up", "Crown Up"]);
    let first = &results[0];
    assert_eq!(first.rate_s_per_day, Some(12.3456789));
    assert_eq!(first.beat_error_ms, Some(0.31));
    assert_eq!(first.amplitude_deg, Some(281.5));
    assert_eq!(first.beat_frequency_bph, Some(28800.0));
    assert_eq!(first.lift_angle_deg, Some(52.0));
    assert_eq!(first.duration_seconds, Some(61.0));
    assert_eq!(results[1].amplitude_deg, None);
    assert!(
        first.notes.starts_with("Measured by timegrapher"),
        "{}",
        first.notes
    );
    // a note goes first; blank space is not a note
    let with = results_from_table(&table(), None, None, "  check the crown \n");
    assert!(
        with[0].notes.starts_with("check the crown\nMeasured by"),
        "{}",
        with[0].notes
    );
    assert!(with[0].beat_frequency_bph.is_none() && with[0].lift_angle_deg.is_none());
}

#[test]
fn send_says_why_it_cannot_yet() {
    let mut app = bare_app(&temp_dir("blocker"));
    let some = results_from_table(&table(), None, None, "");
    assert!(app
        .erp_send_blocker(&some)
        .unwrap()
        .contains("Pick the watch"));
    app.erp.picked = Some(identity("ID-1"));
    assert!(app
        .erp_send_blocker(&[])
        .unwrap()
        .contains("No position has a reading"));
    assert_eq!(app.erp_send_blocker(&some), None);
}

#[test]
fn what_the_erp_answers_reaches_the_window_in_words() {
    let mut app = bare_app(&temp_dir("replies"));
    app.erp.apply(Reply::Searched {
        query: "xx".into(),
        found: Ok(Vec::new()),
    });
    assert_eq!(
        app.erp.view_note.as_ref().unwrap().text,
        "No match in the ERP."
    );
    app.erp.apply(Reply::Searched {
        query: "xx".into(),
        found: Err("could not reach the ERP: down".into()),
    });
    assert!(!app.erp.view_note.as_ref().unwrap().ok);
    assert!(app.erp.results.is_empty());
    app.erp
        .apply(Reply::SignedIn(Ok(LoginOutcome::Refused("locked".into()))));
    assert_eq!(app.erp.operator_note.as_ref().unwrap().text, "locked");
    assert!(app.erp.operator.is_none());
}

/// The widths of the rectangles drawn in a frame.
fn rect_widths(shapes: &[egui::epaint::ClippedShape]) -> Vec<f32> {
    fn walk(shape: &egui::Shape, out: &mut Vec<f32>) {
        match shape {
            egui::Shape::Rect(r) => out.push(r.rect.width()),
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    shapes.iter().for_each(|c| walk(&c.shape, &mut out));
    out
}

/// A settings row gives its control the width it took last frame, which
/// squeezed the ERP page's text fields to a few points: each is its own
/// width, from the first frame on.
#[test]
fn the_erp_settings_text_fields_are_not_squeezed() {
    let mut app = bare_app(&temp_dir("widths"));
    app.open_settings(Tab::Erp);
    let ctx = themed();
    // A few frames: the window settles before it is looked at.
    let mut out = ctx.run(screen(), |ctx| app.panels(ctx));
    for _ in 0..4 {
        out = ctx.run(screen(), |ctx| app.panels(ctx));
    }
    // Address, key, secret, Station ID and Operator ID.
    let widths = rect_widths(&out.shapes);
    let fields = widths
        .iter()
        .filter(|w| (**w - FIELD_W).abs() < 16.0)
        .count();
    assert!(
        fields >= 5,
        "{fields} fields of about {FIELD_W}: {widths:?}"
    );
}

// ---- the Bench's ERP card ----

#[test]
fn the_bench_has_no_erp_card_until_the_erp_is_set_up() {
    let mut app = bare_app(&temp_dir("no-card"));
    app.erp.ensure_loaded();
    assert_eq!(app.view, View::Bench);
    let text = frame_text(&mut app);
    assert!(!text.contains("Quick search the ERP"), "{text}");
    assert!(!text.contains("Send 0 Positions"), "{text}");
}

#[test]
fn a_set_up_erp_puts_a_card_with_a_quick_search_at_the_top_of_the_bench() {
    let erp = start_erp();
    let (mut app, dir) = set_up("card", &erp);
    let text = frame_text(&mut app);
    for want in [
        "Quick search the ERP",
        "Not signed in",
        "Send 0 Positions",
        "ERP View",
        "Search above to pick the watch being measured.",
        "The Positions Test on the Bench shows here.",
    ] {
        assert!(text.contains(want), "{want}: {text}");
    }
    // The Bench itself is still there under it.
    assert!(text.contains("Listen"), "{text}");
    // Folded, the card keeps its one row.
    app.erp.card_closed = true;
    let text = frame_text(&mut app);
    assert!(text.contains("Quick search the ERP"), "{text}");
    assert!(!text.contains("Search above to pick"), "{text}");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn the_quick_search_on_the_bench_lists_matches_and_the_pick_fills_the_card() {
    let erp = start_erp();
    let (mut app, dir) = set_up("card-search", &erp);
    let ctx = egui::Context::default();
    // Typed on the Bench, asked for from there: no other view is needed.
    app.erp.query = "ID-1".into();
    app.erp.typed_at = Some(Instant::now() - Duration::from_secs(5));
    app.erp.search_open = true;
    app.erp_poll(&ctx);
    assert!(app.erp.busy());
    finish(&mut app);
    let text = frame_text(&mut app);
    assert!(text.contains("ID-1  |  ETA 2824-2  |  Movement"), "{text}");
    // Picking one asks for the whole identity, closes the list and fills the card.
    app.erp.look_up(&ctx, "ID-1".into());
    finish(&mut app);
    assert!(!app.erp.search_open);
    let text = frame_text(&mut app);
    assert!(!text.contains("ID-1  |  ETA 2824-2  |  Movement"), "{text}");
    for want in [
        "Part",
        "ETA 2824-2",
        "Recommended",
        "Follow-up Certification",
    ] {
        assert!(text.contains(want), "{want}: {text}");
    }
    assert!(!text.contains("Search above to pick"), "{text}");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn the_card_says_who_is_signed_in() {
    let erp = start_erp();
    let (mut app, dir) = set_up("card-who", &erp);
    let ctx = egui::Context::default();
    app.erp.operator_id = "OP1".into();
    app.erp.pin = Zeroizing::new(PIN.to_string());
    app.erp.sign_in(&ctx);
    finish(&mut app);
    let text = frame_text(&mut app);
    assert!(text.contains("Signed in: Op One"), "{text}");
    assert!(!text.contains("Not signed in"), "{text}");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn the_card_shares_its_state_with_the_erp_view() {
    let erp = start_erp();
    let (mut app, dir) = set_up("card-shared", &erp);
    let ctx = egui::Context::default();
    app.erp.look_up(&ctx, "ID-1".into());
    finish(&mut app);
    app.erp.note = "one note, two places".into();
    // The same identity and note on the ERP view, and what Send would send
    // is the same from either.
    app.view = View::Erp;
    let text = frame_text(&mut app);
    assert!(text.contains("ETA 2824-2"), "{text}");
    assert_eq!(
        app.erp_results_now(&ctx).len(),
        0,
        "no Positions Test is on the Bench"
    );
    let _ = std::fs::remove_dir_all(dir);
}

/// Where each text was painted, its top.
fn text_tops(out: &egui::FullOutput) -> Vec<(String, f32)> {
    out.shapes
        .iter()
        .filter_map(|c| match &c.shape {
            egui::Shape::Text(t) => Some((t.galley.text().to_string(), t.pos.y)),
            _ => None,
        })
        .collect()
}

/// The card is as tall as what it holds: the Bench's readings and panes stay
/// under it, in view, not pushed away by a card that fills the Bench.
#[test]
fn the_erp_card_leaves_the_bench_its_room() {
    let erp = start_erp();
    let (mut app, dir) = set_up("card-room", &erp);
    let ctx = themed();
    let mut out = ctx.run(screen(), |ctx| app.panels(ctx));
    for _ in 0..3 {
        out = ctx.run(screen(), |ctx| app.panels(ctx));
    }
    let tops = text_tops(&out);
    let top_of = |word: &str| {
        tops.iter()
            .find(|(t, _)| t == word)
            .unwrap_or_else(|| panic!("{word}: {tops:?}"))
            .1
    };
    // The window is 820 points tall. The card holds a row and six table
    // rows; the Bench's own cards begin well above the bottom.
    assert!(
        top_of("Listen") < 420.0,
        "the Bench starts at {}: the card took its room",
        top_of("Listen")
    );
    let _ = std::fs::remove_dir_all(dir);
}

// ---- the card waits for an ERP that answers ----

/// A web address nothing listens on.
fn dead_url() -> String {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    drop(listener);
    url
}

fn pings(erp: &Erp) -> usize {
    erp.state
        .lock()
        .unwrap()
        .log
        .iter()
        .filter(|(n, _, _)| n == "ping")
        .count()
}

#[test]
fn an_erp_that_is_set_up_but_not_checked_yet_shows_no_card() {
    let erp = start_erp();
    let dir = temp_dir("unchecked");
    Config {
        base_url: erp.url.clone(),
        api_key: KEY.into(),
        api_secret: SECRET.into(),
        station_id: "STATION-01".into(),
        ..Config::default()
    }
    .save(&dir)
    .unwrap();
    let mut app = bare_app(&dir);
    app.erp.ensure_loaded();
    assert!(app.erp.configured());
    assert!(!app.erp.live(), "defined, but nothing has answered yet");
    assert!(!frame_text_once(&mut app).contains("Quick search the ERP"));
    // The first frame asks, and the answer brings the card.
    finish(&mut app);
    assert!(app.erp.live());
    assert!(frame_text(&mut app).contains("Quick search the ERP"));
    let _ = std::fs::remove_dir_all(dir);
}

/// One frame, as the first the window draws.
fn frame_text_once(app: &mut TimegrapherApp) -> String {
    let ctx = themed();
    painted_text(&ctx.run(screen(), |ctx| app.panels(ctx)))
}

#[test]
fn an_erp_that_does_not_answer_has_no_card_and_the_view_says_why() {
    let dir = temp_dir("dead");
    Config {
        base_url: dead_url(),
        api_key: KEY.into(),
        api_secret: SECRET.into(),
        station_id: "STATION-01".into(),
        ..Config::default()
    }
    .save(&dir)
    .unwrap();
    let mut app = bare_app(&dir);
    app.erp.ensure_loaded();
    finish(&mut app);
    assert!(app.erp.configured());
    assert!(!app.erp.live());
    let text = frame_text(&mut app);
    assert!(!text.contains("Quick search the ERP"), "{text}");
    assert!(!text.contains("Send 0 Positions"), "{text}");
    // The ERP view is still there, and says the ERP is not answering.
    app.view = View::Erp;
    let text = frame_text(&mut app);
    assert!(text.contains("The ERP is not answering now"), "{text}");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn the_card_goes_when_the_erp_stops_answering_and_returns_when_it_does_again() {
    let erp = start_erp();
    let (mut app, dir) = set_up("comes-and-goes", &erp);
    assert!(frame_text(&mut app).contains("Quick search the ERP"));
    let good = app.erp.config.base_url.clone();
    app.erp.config.base_url = dead_url();
    // Not asked again until it is due.
    finish(&mut app);
    assert!(app.erp.live());
    app.erp.checked_at = Some(Instant::now() - Duration::from_secs(300));
    finish(&mut app);
    assert!(!app.erp.live());
    assert!(!frame_text(&mut app).contains("Quick search the ERP"));
    app.erp.config.base_url = good;
    app.erp.checked_at = Some(Instant::now() - Duration::from_secs(300));
    finish(&mut app);
    assert!(frame_text(&mut app).contains("Quick search the ERP"));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn the_connection_is_checked_once_and_again_only_when_due() {
    let erp = start_erp();
    let (mut app, dir) = set_up("rechecks", &erp);
    assert_eq!(pings(&erp), 1);
    for _ in 0..5 {
        finish(&mut app);
    }
    assert_eq!(pings(&erp), 1, "asked again before it was due");
    app.erp.checked_at = Some(Instant::now() - Duration::from_secs(121));
    finish(&mut app);
    assert_eq!(pings(&erp), 2);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn saving_a_changed_connection_checks_it_again_but_saving_the_rest_does_not() {
    let erp = start_erp();
    let (mut app, dir) = set_up("save-checks", &erp);
    app.erp.form.enhanced = true;
    app.erp.save_settings();
    assert!(app.erp.config.enhanced);
    assert!(app.erp.live(), "only enhanced data changed");
    assert!(app.erp.checked_at.is_some());
    app.erp.form.base_url = dead_url();
    app.erp.save_settings();
    assert!(!app.erp.live());
    assert!(
        app.erp.checked_at.is_none(),
        "a new address is checked at once"
    );
    finish(&mut app);
    assert!(!app.erp.live(), "nothing answers at the new address");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn the_enhanced_switch_is_in_the_settings_and_is_kept() {
    let erp = start_erp();
    let (mut app, dir) = set_up("enhanced-setting", &erp);
    assert!(!app.erp.config.enhanced, "off until it is turned on");
    app.open_settings(Tab::Erp);
    let ctx = themed();
    let mut text = String::new();
    for _ in 0..3 {
        text = painted_text(&ctx.run(screen(), |ctx| app.panels(ctx)));
    }
    assert!(text.contains("Send Enhanced Data"), "{text}");
    app.erp.form.enhanced = true;
    app.erp.save_settings();
    assert!(Config::load(&dir).unwrap().enhanced);
    let _ = std::fs::remove_dir_all(dir);
}

// ---- the quick sign-in on the card ----

/// Where `word` was painted, its middle.
fn text_middle(out: &egui::FullOutput, word: &str) -> Option<egui::Pos2> {
    out.shapes.iter().find_map(|c| match &c.shape {
        egui::Shape::Text(t) if t.galley.text() == word => {
            Some(t.galley.rect.translate(t.pos.to_vec2()).center())
        }
        _ => None,
    })
}

/// A click on the middle of `word` as painted now.
fn click_on(app: &mut TimegrapherApp, word: &str) {
    let ctx = themed();
    let mut out = ctx.run(screen(), |ctx| app.panels(ctx));
    for _ in 0..2 {
        out = ctx.run(screen(), |ctx| app.panels(ctx));
    }
    let pos = text_middle(&out, word).unwrap_or_else(|| panic!("no \"{word}\" on screen"));
    for event in [
        egui::Event::PointerMoved(pos),
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        },
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        },
    ] {
        let mut input = screen();
        input.events = vec![event];
        let _ = ctx.run(input, |ctx| app.panels(ctx));
    }
}

#[test]
fn the_card_offers_a_quick_sign_in_while_nobody_is_signed_in() {
    let erp = start_erp();
    let (mut app, dir) = set_up("quick-signin", &erp);
    let text = frame_text(&mut app);
    for want in [
        "Sign in so the ERP attests your results:",
        "Operator ID",
        "PIN",
        "Sign In",
    ] {
        assert!(text.contains(want), "{want}: {text}");
    }
    // It stays when the card is folded.
    app.erp.card_closed = true;
    assert!(frame_text(&mut app).contains("Sign In"));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn the_quick_sign_in_signs_in_with_what_is_typed_and_the_row_goes() {
    let erp = start_erp();
    let (mut app, dir) = set_up("quick-signin-go", &erp);
    app.erp.operator_id = "OP1".into();
    app.erp.pin = Zeroizing::new(PIN.to_string());
    click_on(&mut app, "Sign In");
    finish(&mut app);
    assert!(app.erp.signed_in(), "{:?}", app.erp.operator_note);
    assert!(app.erp.pin.is_empty(), "the PIN is wiped once sent");
    let text = frame_text(&mut app);
    assert!(text.contains("Signed in: Op One"), "{text}");
    for gone in [
        "Sign in so the ERP attests",
        "Sign-in ended",
        "Operator ID",
        "Sign In",
    ] {
        assert!(!text.contains(gone), "{gone}: {text}");
    }
    // The PIN reached the ERP once, in the login.
    let state = erp.state.lock().unwrap();
    let logins = state.log.iter().filter(|(n, _, _)| n == "operator_login");
    assert_eq!(logins.count(), 1);
    drop(state);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_wrong_pin_on_the_quick_sign_in_is_told_on_the_card_and_it_stays() {
    let erp = start_erp();
    let (mut app, dir) = set_up("quick-signin-wrong", &erp);
    app.erp.operator_id = "OP1".into();
    app.erp.pin = Zeroizing::new("111111".to_string());
    click_on(&mut app, "Sign In");
    finish(&mut app);
    assert!(!app.erp.signed_in());
    let text = frame_text(&mut app);
    assert!(text.contains("The Operator ID or PIN is wrong."), "{text}");
    assert!(text.contains("Sign In"), "{text}");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn the_quick_sign_in_will_not_send_what_cannot_be_a_pin() {
    let erp = start_erp();
    let (mut app, dir) = set_up("quick-signin-short", &erp);
    app.erp.operator_id = "OP1".into();
    app.erp.pin = Zeroizing::new("12".to_string());
    click_on(&mut app, "Sign In");
    finish(&mut app);
    let state = erp.state.lock().unwrap();
    assert!(!state.log.iter().any(|(n, _, _)| n == "operator_login"));
    drop(state);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_sign_in_that_has_ended_is_asked_for_again_on_the_card() {
    let erp = start_erp();
    let (mut app, dir) = set_up("quick-signin-ended", &erp);
    app.erp.operator = Some(OperatorSession::new(
        "tok".into(),
        "OP1".into(),
        "Op One".into(),
        SystemTime::now() - Duration::from_secs(9 * 3600),
    ));
    assert!(!app.erp.signed_in());
    let text = frame_text(&mut app);
    assert!(text.contains("Sign-in ended. Sign in again:"), "{text}");
    let _ = std::fs::remove_dir_all(dir);
}

// ---- enhanced data ----

/// A recording of `seconds` at 8 kHz that is not silence, and its samples.
fn tone_file(dir: &Path, seconds: usize) -> (PathBuf, Vec<i32>) {
    std::fs::create_dir_all(dir).unwrap();
    // Not a repeating pattern, so a cut from the wrong place cannot match.
    let mut x: u32 = 12345;
    let samples: Vec<i32> = (0..seconds * 8000)
        .map(|_| {
            x = x.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            ((x >> 16) % 2000) as i32 - 1000
        })
        .collect();
    let pcm = timegrapher_core::excerpt::Pcm {
        sample_rate: 8000,
        channels: 1,
        bits_per_sample: 16,
        samples: samples.clone(),
    };
    let path = dir.join("audio-001.flac");
    timegrapher_core::excerpt::write_flac(&path, &pcm).unwrap();
    (path, samples)
}

/// The fixture table with core's detail for the two positions read.
fn table_with_details() -> guided::FreeTable {
    let mut t = table();
    let detail = |rate: f64, spans: Vec<(f64, f64)>| guided::RowDetail {
        spans,
        measurement: Some(serde_json::json!({"rate_s_per_day": rate, "beats": 4100,
            "caveats": [], "clean_fraction": 0.987654321})),
    };
    t.details = t
        .rows
        .iter()
        .map(|r| match r.0.as_str() {
            l if l.starts_with("Dial Up") => detail(12.3456789, vec![(10.0, 70.0)]),
            l if l.starts_with("Crown Up") => detail(-3.5, vec![(80.0, 125.0)]),
            _ => guided::RowDetail::default(),
        })
        .collect();
    t.indices = Some(serde_json::json!({"x": 4.4, "d_rate": 15.8}));
    t
}

fn input_from(dir: &Path) -> (EnhancedInput, Vec<i32>) {
    let (file, samples) = tone_file(dir, 130);
    (
        EnhancedInput {
            table: table_with_details(),
            files: vec![file],
            about: serde_json::json!({"bph": 28800, "lift_deg": 52.0, "calibre": "ETA 2824-2"}),
        },
        samples,
    )
}

use super::erp_enhanced::{attach, EnhancedInput};

#[test]
fn each_position_gets_core_s_whole_measurement_and_the_sound_it_was_read_from() {
    let dir = temp_dir("attach");
    let (input, samples) = input_from(&dir);
    let mut results = results_from_table(&input.table, Some(28800), Some(52.0), "");
    let notes = attach(&mut results, &input);
    assert!(notes.is_empty(), "{notes:?}");
    let du = results[0].enhanced.as_ref().unwrap();
    assert_eq!(du.block["version"], 1);
    assert_eq!(du.block["position"]["code"], "CH");
    assert_eq!(
        du.block["position"]["spans"],
        serde_json::json!([[10.0, 70.0]])
    );
    assert_eq!(du.block["measurement"]["rate_s_per_day"], 12.3456789);
    assert_eq!(du.block["measurement"]["beats"], 4100);
    assert_eq!(du.block["settings"]["calibre"], "ETA 2824-2");
    assert_eq!(du.block["positions_test"]["indices"]["d_rate"], 15.8);
    assert_eq!(du.block["positions_test"]["positions_read"], 2);
    // the recording is the take's own samples over the stretch, untouched
    let rec = du.recording.as_ref().unwrap();
    assert_eq!((rec.from_s, rec.duration_s), (10.0, 60.0));
    assert_eq!(
        (rec.sample_rate, rec.channels, rec.bits_per_sample),
        (8000, 1, 16)
    );
    assert_eq!(rec.name, "Dial Up.flac");
    let cut = dir.join("cut.flac");
    std::fs::write(&cut, &rec.flac).unwrap();
    let back = timegrapher_core::excerpt::read(&cut).unwrap();
    assert_eq!(back.samples, samples[10 * 8000..70 * 8000]);
    // the second position is cut from its own stretch
    let cu = results[1].enhanced.as_ref().unwrap();
    assert_eq!(cu.block["position"]["code"], "3H");
    assert_eq!(cu.recording.as_ref().unwrap().from_s, 80.0);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn with_no_sound_file_the_measurement_still_goes_and_the_missing_recording_is_said() {
    let dir = temp_dir("attach-nosound");
    let (mut input, _) = input_from(&dir);
    input.files.clear();
    let mut results = results_from_table(&input.table, None, None, "");
    let notes = attach(&mut results, &input);
    assert!(results[0].enhanced.as_ref().unwrap().recording.is_none());
    assert_eq!(
        results[0].enhanced.as_ref().unwrap().block["measurement"]["rate_s_per_day"],
        12.3456789
    );
    assert!(
        notes.iter().any(|n| n.starts_with("Dial Up: no recording")),
        "{notes:?}"
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_recording_that_cannot_be_cut_does_not_lose_the_position() {
    let dir = temp_dir("attach-badfile");
    let (mut input, _) = input_from(&dir);
    input.files = vec![dir.join("missing.flac")];
    let mut results = results_from_table(&input.table, None, None, "");
    let notes = attach(&mut results, &input);
    let du = results[0].enhanced.as_ref().unwrap();
    assert!(du.recording.is_none());
    assert!(du.block["measurement"].is_object());
    assert!(
        notes.iter().any(|n| n.contains("no recording")),
        "{notes:?}"
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_position_the_table_holds_no_detail_for_is_sent_as_it_was() {
    let dir = temp_dir("attach-nodetail");
    let (mut input, _) = input_from(&dir);
    input.table.details.clear();
    let mut results = results_from_table(&input.table, None, None, "");
    let notes = attach(&mut results, &input);
    assert!(results.iter().all(|r| r.enhanced.is_none()));
    assert_eq!(notes.len(), 2, "{notes:?}");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn enhanced_data_reaches_the_erp_signed_with_the_recordings() {
    let erp = start_erp();
    let (mut app, dir) = set_up("send-enhanced", &erp);
    let ctx = egui::Context::default();
    app.erp.operator_id = "OP1".into();
    app.erp.pin = Zeroizing::new(PIN.to_string());
    app.erp.sign_in(&ctx);
    finish(&mut app);
    app.erp.look_up(&ctx, "ID-1".into());
    finish(&mut app);
    let (input, samples) = input_from(&dir.join("take"));
    let results = results_from_table(&input.table, Some(28800), Some(52.0), "");
    app.erp.send_with(&ctx, results, Some(input));
    finish(&mut app);
    // the ERP verified the station's signature over the digests and the sign-in
    assert!(
        app.erp.sent[0]
            .starts_with("Dial Up: sent, Attested, with its whole measurement and recording ("),
        "{:?}",
        app.erp.sent
    );
    assert!(
        app.erp.sent[1].starts_with("Crown Up: sent, Attested"),
        "{:?}",
        app.erp.sent
    );
    let state = erp.state.lock().unwrap();
    assert_eq!(state.enhanced.len(), 2);
    assert_eq!(
        state.enhanced[0].1["measurement"]["rate_s_per_day"],
        12.3456789
    );
    assert_eq!(state.recordings.len(), 2);
    let cut = dir.join("kept.flac");
    std::fs::write(&cut, &state.recordings[0].1).unwrap();
    let back = timegrapher_core::excerpt::read(&cut).unwrap();
    assert_eq!(back.samples, samples[10 * 8000..70 * 8000]);
    assert_eq!(state.results[0]["schema"], "ln_tg_timing_result_v2");
    drop(state);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn without_the_switch_the_positions_go_as_they_always_did() {
    let erp = start_erp();
    let (mut app, dir) = set_up("send-plain", &erp);
    let ctx = egui::Context::default();
    app.erp.look_up(&ctx, "ID-1".into());
    finish(&mut app);
    assert!(!app.erp.config.enhanced);
    app.erp_send_now(&ctx, results_from_table(&table(), None, None, ""));
    finish(&mut app);
    let state = erp.state.lock().unwrap();
    assert_eq!(state.results[0]["schema"], "ln_tg_timing_result_v1");
    assert!(state.enhanced.is_empty() && state.recordings.is_empty());
    assert!(!state.log.iter().any(|(n, _, _)| n == "submit_recording"));
    drop(state);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn an_erp_that_cannot_take_enhanced_data_gets_the_figures_and_the_window_says_so() {
    let erp = start_erp();
    erp.state.lock().unwrap().old_erp = true;
    let (mut app, dir) = set_up("send-old-erp", &erp);
    let ctx = egui::Context::default();
    app.erp.look_up(&ctx, "ID-1".into());
    finish(&mut app);
    let (input, _) = input_from(&dir.join("take"));
    let results = results_from_table(&input.table, None, None, "");
    app.erp.send_with(&ctx, results, Some(input));
    finish(&mut app);
    assert!(
        app.erp
            .sent
            .iter()
            .any(|l| l.contains("does not take enhanced data yet")),
        "{:?}",
        app.erp.sent
    );
    let state = erp.state.lock().unwrap();
    assert_eq!(state.results.len(), 2, "both positions still went");
    assert!(state.enhanced.is_empty());
    drop(state);
    let _ = std::fs::remove_dir_all(dir);
}

/// The table core reads from a take carries core's whole measurement and the
/// stretches for every position: what the enhanced data is made of.
fn app_with_positions() -> TimegrapherApp {
    use timegrapher_core::synth::{generate, SynthConfig};
    let mut app = super::tests::synthetic(40.0, 0.0);
    app.session.listen();
    app.session.set_state(session::State::Paused {
        since: Instant::now(),
    });
    let audio = generate(
        &SynthConfig {
            duration_s: 40.0,
            rate_s_per_day: 60.0,
            ..Default::default()
        },
        |_| 280.0,
        |_| 0.0,
    );
    app.session.set_state(session::State::Listening);
    app.set_position(Position::DialDown);
    app.session.set_state(session::State::Paused {
        since: Instant::now(),
    });
    let live = app.live.as_mut().unwrap();
    for b in audio.samples.chunks(960) {
        live.push(b);
    }
    app
}

#[test]
fn enhanced_data_is_made_only_when_the_switch_is_on() {
    let mut app = app_with_positions();
    let ctx = egui::Context::default();
    // The window has been drawing the table, which a take on show reads in
    // the background: wait for it as the frames would.
    for _ in 0..500 {
        if app.free_table_shown(&ctx).is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(app.free_table_shown(&ctx).is_some(), "the table never came");
    assert!(!app.erp.config.enhanced);
    assert!(app.erp_enhanced_input(&ctx).is_none(), "switched off");
    app.erp.config.enhanced = true;
    let input = app
        .erp_enhanced_input(&ctx)
        .expect("a Positions Test to take it from");
    assert!(input.table.read >= 2);
    assert_eq!(input.about["bph"], serde_json::json!(app.bph));
}

#[test]
fn the_positions_table_carries_core_s_measurement_and_the_stretches() {
    let app = app_with_positions();
    let t = app.free_table(&egui::Context::default()).expect("a table");
    assert_eq!(t.details.len(), t.rows.len());
    for ((label, _, cell), detail) in t.rows.iter().zip(&t.details) {
        if matches!(cell, guided::FreeCell::Read(..)) {
            let m = detail
                .measurement
                .as_ref()
                .unwrap_or_else(|| panic!("{label}"));
            // core's own fields, not a copy of the table's three
            for key in [
                "rate_s_per_day",
                "beats",
                "caveats",
                "confidence",
                "clean_fraction",
            ] {
                assert!(m.get(key).is_some(), "{label}: {key}");
            }
            assert!(!detail.spans.is_empty(), "{label}");
            assert!(
                detail
                    .spans
                    .iter()
                    .all(|s| s.0 >= 0.0 && s.1 <= 80.5 && s.0 < s.1),
                "{label}: {:?}",
                detail.spans
            );
        } else {
            assert!(
                detail.spans.is_empty() || detail.measurement.is_none(),
                "{label}"
            );
        }
    }
    let indices = t.indices.as_ref().unwrap();
    assert!(indices.get("x").is_some() && indices.get("d_rate").is_some());
}
