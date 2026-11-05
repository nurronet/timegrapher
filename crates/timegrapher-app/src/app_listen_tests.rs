//! The Listen card's Stop and Listen button, which share one place,
//! and the Save menu beside it.

use super::tests::{queued_capture, save_failure_fixture};
use super::*;

/// A Quick Listen still heard is kept in a watch from Save's Keep in a
/// Watch without stopping (#743): listening goes on, the take is the
/// watch's from then, with its calibre, and Stop saves it into the watch.
#[test]
fn a_quick_listen_heard_is_kept_in_a_watch_and_goes_on() {
    let base = std::env::temp_dir().join(format!("tg-app-keep-live-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();
    let root = base.join("Timegrapher Data");
    let mut lib = library::Library::create(&root).unwrap();
    let c = timegrapher_core::calibres::find("2824-2").expect("ETA 2824-2 in the table");
    let mut pepsi = library::Watch::new("Pepsi");
    pepsi.calibre = Some(c.calibre.clone());
    lib.add(&pepsi).unwrap();
    drop(lib);
    let takes = root.join("Pepsi").join(library::TAKES);
    let mut app = TimegrapherApp::with_devices(Vec::new(), None);
    app.use_library(Some(root.clone()));
    assert!(app.quick_listen_ready(true));
    assert!(app.calibre.is_empty());
    app.session.listen();
    let info = app.info("", 48000, 16);
    let mut r = Recorder::start(&base.join("unsaved"), info).unwrap();
    r.write(&timegrapher_core::capture::Block {
        samples: vec![0.0; 48000 * 5],
        at: std::time::SystemTime::now(),
    })
    .unwrap();
    app.session.recorder = Some(r);
    let _tx = queued_capture(&mut app, &base);
    assert!(app.running() && app.session.allows(Action::Save));
    let ctx = egui::Context::default();
    theme::install(&ctx);
    let mut time = 0.0;
    let mut pass = |app: &mut TimegrapherApp, events: Vec<egui::Event>| {
        time += 0.25;
        let input = egui::RawInput {
            time: Some(time),
            events,
            ..Default::default()
        };
        let out = ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| app.save_menu(ui, 32.0));
        });
        out.shapes
            .iter()
            .filter_map(|c| match &c.shape {
                egui::Shape::Text(t) => Some((
                    t.galley.text().to_string(),
                    t.galley.rect.translate(t.pos.to_vec2()).center(),
                )),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    let click = |at: egui::Pos2| {
        let button = |pressed| egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        vec![egui::Event::PointerMoved(at), button(true), button(false)]
    };
    pass(&mut app, Vec::new());
    // The Save button at the panel's top left: its menu opens.
    pass(&mut app, click(egui::pos2(8.0 + 16.0, 8.0 + 16.0)));
    let drawn = pass(&mut app, Vec::new());
    let pepsi = drawn
        .iter()
        .find(|t| t.0 == "Pepsi")
        .unwrap_or_else(|| panic!("{drawn:?}"))
        .1;
    assert!(drawn.iter().any(|t| t.0 == "New Watch…"), "{drawn:?}");
    // A click on it: the take is Pepsi's, still heard, nothing saved yet.
    pass(&mut app, click(pepsi));
    pass(&mut app, Vec::new());
    assert!(app.running(), "listening goes on");
    assert!(!app.quick_listen && app.watch == "Pepsi");
    assert_eq!(
        app.open_watch().map(|e| e.watch.name.as_str()),
        Some("Pepsi")
    );
    assert_eq!(app.calibre, c.calibre);
    assert!(app.session.saved_to().is_none());
    assert!(std::fs::read_dir(&takes).map_or(0, |d| d.count()) == 0);
    // Stop saves it into Pepsi, as a take of Pepsi.
    app.capture = None;
    app.stop_listening();
    app.wait_for_save();
    assert_eq!(std::fs::read_dir(&takes).map_or(0, |d| d.count()), 1);
    assert!(app
        .session
        .saved_to()
        .is_some_and(|p| p.starts_with(&takes)));
    let _ = std::fs::remove_dir_all(&base);
}

/// A take of a watch heard can't be moved to another watch: the Watch
/// line's other watches are greyed until Stop, and a pick there does
/// nothing.
#[test]
fn a_watchs_take_heard_keeps_its_watch() {
    let base = std::env::temp_dir().join(format!("tg-app-keep-watch-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();
    let root = base.join("Timegrapher Data");
    let mut lib = library::Library::create(&root).unwrap();
    lib.add(&library::Watch::new("Pepsi")).unwrap();
    lib.add(&library::Watch::new("Diver")).unwrap();
    let pepsi = lib.find("Pepsi").unwrap().watch.id.clone();
    let diver = lib.find("Diver").unwrap().watch.id.clone();
    drop(lib);
    let mut app = TimegrapherApp::with_devices(Vec::new(), None);
    app.use_library(Some(root.clone()));
    app.pick_watch(&pepsi);
    app.session.listen();
    let _tx = queued_capture(&mut app, &base);
    assert!(app.running() && !app.quick_listen);
    app.line_pick(Some(diver.clone()), false);
    app.keep_in_watch(&diver);
    assert_eq!(app.watch_open.as_deref(), Some(pepsi.as_str()));
    assert_eq!(app.watch, "Pepsi");
    app.capture = None;
    let _ = std::fs::remove_dir_all(&base);
}

/// The Listen button's count starts at the frame after it first shows,
/// and a press counts only from [`LISTEN_SETTLE_S`] after that (#725).
#[test]
fn listen_counts_a_press_only_once_settled() {
    // First drawn with a press in it: aimed at Stop.
    let tap = listen_tap(None, 7.0, true);
    assert_eq!(tap, (f64::INFINITY, false));
    // The next frame ran long, and delivers a press made while it ran,
    // with Stop still on screen: the count starts, the press is Stop's.
    let tap = listen_tap(Some(tap), 8.0, true);
    assert_eq!(tap, (8.0, false));
    // A press just after: still not counted.
    let tap = listen_tap(Some(tap), 8.0 + LISTEN_SETTLE_S / 2.0, true);
    assert_eq!(tap, (8.0, false));
    // A press once settled arms it, and it stays armed to the release.
    let tap = listen_tap(Some(tap), 8.0 + LISTEN_SETTLE_S, true);
    assert_eq!(tap, (8.0, true));
    assert_eq!(listen_tap(Some(tap), 10.0, false), (8.0, true));
}

/// Taps on Stop while the app lags: the first stops, and the next, landing
/// on Listen in a long frame or a moment later, start nothing (#725).
#[test]
fn a_second_tap_on_stop_starts_nothing() {
    let (mut app, base) = save_failure_fixture("second-stop-tap");
    // No Watches folder to keep a take in, on every system: a counted tap
    // on Listen then says so, and never opens a folder in the real home.
    app.library = Err("none in this test".into());
    let _tx = queued_capture(&mut app, &base);
    assert!(app.running() && app.session.mic());
    let ctx = egui::Context::default();
    theme::install(&ctx);
    let mut time = 0.0;
    let mut pass = |app: &mut TimegrapherApp, after: f64, events: Vec<egui::Event>| {
        time += after;
        let input = egui::RawInput {
            time: Some(time),
            events,
            ..Default::default()
        };
        let _ = ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| app.listen_buttons(ui, false));
        });
    };
    let at = egui::pos2(8.0 + LISTEN_BUTTON / 2.0, 8.0 + LISTEN_BUTTON / 2.0);
    let tap = || {
        let button = |pressed| egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        vec![egui::Event::PointerMoved(at), button(true), button(false)]
    };
    let quick = LISTEN_SETTLE_S / 5.0;
    pass(&mut app, quick, Vec::new());
    pass(&mut app, quick, tap());
    assert!(!app.running(), "the first tap stops");
    let stopped = (app.message.clone(), app.confirm.is_some());
    // Listen shows, and the frame after runs long, as a lagging app's
    // does: the second tap it delivers was made while Stop was on screen.
    pass(&mut app, quick, Vec::new());
    pass(&mut app, LISTEN_SETTLE_S * 2.0, tap());
    assert!(!app.running());
    assert_eq!((app.message.clone(), app.confirm.is_some()), stopped);
    // A third tap a moment later: still nothing.
    pass(&mut app, quick, tap());
    assert!(!app.running());
    assert_eq!((app.message.clone(), app.confirm.is_some()), stopped);
    // Once settled, a tap on Listen counts: it says why it can't listen.
    pass(&mut app, LISTEN_SETTLE_S, Vec::new());
    pass(&mut app, quick, tap());
    let said = app
        .message
        .as_ref()
        .map(|m| m.0.as_str())
        .unwrap_or_default();
    assert!(
        said.starts_with("Quick Listen can't keep its take"),
        "{said}"
    );
    app.capture = None;
    let _ = std::fs::remove_dir_all(&base);
}

/// A Quick Listen kept in Unsorted Takes records into it as it goes (Peter,
/// 5 Nov): each [`STREAM_EVERY_S`] more of sound is added to the take with
/// no Save Again, the Save menu says where it is saving and has no Save
/// Again, the status bar says it is saved, and Stop adds the last of it
/// with no question.
#[test]
fn a_kept_quick_listen_records_into_its_take() {
    let base = std::env::temp_dir().join(format!("tg-app-stream-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();
    let root = base.join("Timegrapher Data");
    drop(library::Library::create(&root).unwrap());
    let mut app = TimegrapherApp::with_devices(Vec::new(), None);
    app.use_library(Some(root.clone()));
    assert!(app.quick_listen_ready(true));
    app.session.listen();
    let info = app.info("", 48000, 16);
    let mut r = Recorder::start(&base.join("unsaved"), info).unwrap();
    let start = std::time::SystemTime::now();
    let mut heard = 0;
    let mut hear = |r: &mut Recorder, s: u64| {
        r.write(&timegrapher_core::capture::Block {
            samples: vec![0.0; 48000 * s as usize],
            at: start + std::time::Duration::from_secs(heard),
        })
        .unwrap();
        heard += s;
    };
    hear(&mut r, 3);
    app.session.recorder = Some(r);
    let _tx = queued_capture(&mut app, &base);
    // Not kept yet: nothing is saved as it goes.
    assert!(!app.streams());
    app.stream_tick();
    assert!(app.saving.is_none());
    // Kept in Unsorted Takes, as the Save menu's Unsorted Takes does.
    assert!(app.saves_into_unsorted());
    app.save_again();
    app.wait_for_save();
    let take = app.session.saved_to().expect("kept").to_path_buf();
    let wav = take.join("audio-001.wav");
    let samples = |p: &std::path::Path| timegrapher_core::audio::load(p).unwrap().samples.len();
    assert_eq!(samples(&wav), 3 * 48000);
    assert!(
        app.streams(),
        "a kept Quick Listen heard records into its take"
    );
    assert_eq!(app.kept_place(), "Unsorted Takes");
    // Under STREAM_EVERY_S more: not yet.
    hear(app.session.recorder.as_mut().unwrap(), 4);
    app.stream_tick();
    assert!(app.saving.is_none());
    // Past it: the sound since is added, saying nothing.
    hear(app.session.recorder.as_mut().unwrap(), 2);
    app.message = None;
    app.stream_tick();
    assert!(app.saving.is_some(), "a save of the sound since");
    app.wait_for_save();
    assert!(app.message.is_none(), "{:?}", app.message);
    assert_eq!(samples(&wav), 9 * 48000);
    assert_eq!(app.session.saved_to(), Some(take.as_path()));
    assert!(app.unsaved_s() < 1.0);
    // Nothing to save again: File > Save is dimmed, saying where it saves.
    assert!(!app.menu_enabled(menus::MenuCommand::Save));
    // The status bar: saved, its hover saying where to.
    {
        let pal = theme::LIGHT;
        let r = app.session.recorder.as_ref().unwrap();
        let (text, _, hover) = app.recording_state(r, &pal);
        assert_eq!(text, "Saved");
        assert!(hover.starts_with("Saving to Unsorted Takes:"), "{hover}");
    }
    // Stop adds the last of it, with no question.
    hear(app.session.recorder.as_mut().unwrap(), 2);
    app.stop_listening();
    assert!(app.confirm.is_none());
    app.wait_for_save();
    assert!(!app.streams());
    assert!(!app.needs_save_confirmation());
    let flac = take.join("audio-001.flac");
    let kept = if flac.exists() { flac } else { wav };
    assert_eq!(samples(&kept), 11 * 48000);
    let _ = std::fs::remove_dir_all(&base);
}

/// A watch's own take records into its folder as it goes, from its first
/// [`STREAM_EVERY_S`] of sound, so a crash costs only seconds; a quit adds
/// the last of it with no question.
#[test]
fn a_watchs_take_records_into_its_folder_as_it_goes() {
    let base = std::env::temp_dir().join(format!("tg-app-stream-watch-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();
    let root = base.join("Timegrapher Data");
    let mut lib = library::Library::create(&root).unwrap();
    lib.add(&library::Watch::new("Pepsi")).unwrap();
    let pepsi = lib.find("Pepsi").unwrap().watch.id.clone();
    drop(lib);
    let takes = root.join("Pepsi").join(library::TAKES);
    let mut app = TimegrapherApp::with_devices(Vec::new(), None);
    app.use_library(Some(root.clone()));
    app.pick_watch(&pepsi);
    app.session.listen();
    let info = app.info("", 48000, 16);
    let mut r = Recorder::start(&base.join("unsaved"), info).unwrap();
    r.write(&timegrapher_core::capture::Block {
        samples: vec![0.0; 48000 * 6],
        at: std::time::SystemTime::now(),
    })
    .unwrap();
    app.session.recorder = Some(r);
    let _tx = queued_capture(&mut app, &base);
    assert!(app.streams() && !app.quick_listen);
    assert_eq!(app.kept_place(), "Pepsi");
    app.stream_tick();
    app.wait_for_save();
    let take = app
        .session
        .saved_to()
        .expect("saved as it goes")
        .to_path_buf();
    assert!(take.starts_with(&takes), "{}", take.display());
    let wav = take.join("audio-001.wav");
    assert_eq!(
        timegrapher_core::audio::load(&wav).unwrap().samples.len(),
        6 * 48000
    );
    assert!(app.message.is_none(), "{:?}", app.message);
    assert!(!app.needs_save_confirmation());
    let _ = std::fs::remove_dir_all(&base);
}
