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
    // Over the watch: what keeping it there does.
    let mut said = false;
    pass(&mut app, vec![egui::Event::PointerMoved(pepsi)]);
    for _ in 0..8 {
        let drawn = pass(&mut app, Vec::new());
        said |= drawn.iter().any(|t| t.0 == KEEP_HEARD_HINT);
    }
    assert!(said, "the hover says listening goes on");
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

/// Each text a frame draws, with its centre.
fn drawn_at(out: &egui::FullOutput) -> Vec<(String, egui::Pos2)> {
    out.shapes
        .iter()
        .filter_map(|c| match &c.shape {
            egui::Shape::Text(t) => Some((
                t.galley.text().to_string(),
                t.galley.rect.translate(t.pos.to_vec2()).center(),
            )),
            _ => None,
        })
        .collect()
}

/// A click at `at`, pressed and let go within one frame.
fn click_at(at: egui::Pos2) -> Vec<egui::Event> {
    let button = |pressed| egui::Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    };
    vec![egui::Event::PointerMoved(at), button(true), button(false)]
}

/// The Watch line's menu keeps a Quick Listen still heard in the watch
/// picked there (#743, Trevor's HOLD): listening goes on, and the take is
/// that watch's.
#[test]
fn a_quick_listen_heard_is_kept_in_a_watch_from_the_watch_line() {
    let base = std::env::temp_dir().join(format!("tg-app-keep-line-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();
    let root = base.join("Timegrapher Data");
    let mut lib = library::Library::create(&root).unwrap();
    lib.add(&library::Watch::new("Pepsi")).unwrap();
    let pepsi = lib.find("Pepsi").unwrap().watch.id.clone();
    drop(lib);
    let mut app = TimegrapherApp::with_devices(Vec::new(), None);
    app.use_library(Some(root.clone()));
    assert!(app.quick_listen_ready(true));
    app.session.listen();
    let _tx = queued_capture(&mut app, &base);
    assert!(app.running() && app.quick_listen);
    let ctx = theme_ctx();
    let pass = |app: &mut TimegrapherApp, events: Vec<egui::Event>| {
        let input = egui::RawInput {
            events,
            ..Default::default()
        };
        drawn_at(&ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| app.watch_line_menu(ui, false));
        }))
    };
    pass(&mut app, Vec::new());
    let drawn = pass(&mut app, Vec::new());
    let at = drawn
        .iter()
        .find(|t| t.0 == "Pepsi")
        .unwrap_or_else(|| panic!("{drawn:?}"))
        .1;
    pass(&mut app, click_at(at));
    pass(&mut app, Vec::new());
    assert!(app.running(), "listening goes on");
    assert!(!app.quick_listen && app.watch == "Pepsi");
    assert_eq!(app.watch_open.as_deref(), Some(pepsi.as_str()));
    app.capture = None;
    let _ = std::fs::remove_dir_all(&base);
}

/// A recording played and still heard goes into a watch only once it
/// ends: Save's Keep in a Watch items are greyed and say why, so a pick
/// is never a silent no-op (Trevor's HOLD: kept from before #743).
#[test]
fn keep_in_a_watch_waits_for_a_played_recording_to_end() {
    let base = std::env::temp_dir().join(format!("tg-app-keep-played-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();
    let root = base.join("Timegrapher Data");
    let mut lib = library::Library::create(&root).unwrap();
    lib.add(&library::Watch::new("Pepsi")).unwrap();
    drop(lib);
    let mut app = TimegrapherApp::with_devices(Vec::new(), None);
    app.use_library(Some(root.clone()));
    let _tx = queued_capture(&mut app, &base);
    app.input = Input::File;
    app.opened_path = base.join("empty-input.wav").display().to_string();
    assert!(app.running() && !app.quick_listen);
    assert!(app.addable_take().is_some());
    let ctx = theme_ctx();
    let mut time = 0.0;
    let mut pass = |app: &mut TimegrapherApp, events: Vec<egui::Event>| {
        time += 0.25;
        let input = egui::RawInput {
            time: Some(time),
            events,
            ..Default::default()
        };
        drawn_at(&ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| app.save_menu(ui, 32.0));
        }))
    };
    pass(&mut app, Vec::new());
    // The Save button at the panel's top left: its menu opens.
    pass(&mut app, click_at(egui::pos2(8.0 + 16.0, 8.0 + 16.0)));
    let drawn = pass(&mut app, Vec::new());
    let pepsi = drawn
        .iter()
        .find(|t| t.0 == "Pepsi")
        .unwrap_or_else(|| panic!("{drawn:?}"))
        .1;
    // Over the watch: greyed, it says why.
    let mut said = false;
    pass(&mut app, vec![egui::Event::PointerMoved(pepsi)]);
    for _ in 0..8 {
        let drawn = pass(&mut app, Vec::new());
        said |= drawn.iter().any(|t| t.0 == KEEP_AFTER_STOP);
    }
    assert!(said, "the hover says why");
    // A click on it keeps nothing and changes no watch.
    pass(&mut app, click_at(pepsi));
    pass(&mut app, Vec::new());
    assert!(app.watch_open.is_none() && app.keep_back.is_none());
    app.capture = None;
    let _ = std::fs::remove_dir_all(&base);
}

/// A theme installed on a fresh context.
fn theme_ctx() -> egui::Context {
    let ctx = egui::Context::default();
    theme::install(&ctx);
    ctx
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
    // Walter's walk of #1201: after a crash, the next start knows the
    // sound left behind is a copy of this take, not sound never saved.
    let scratch = app.session.recorder.as_ref().unwrap().dir().to_path_buf();
    let note = timegrapher_core::recorder::STREAMED_TO;
    assert!(scratch.join(note).is_file());
    // A second save, with the note there: it isn't copied into the take.
    app.session
        .recorder
        .as_mut()
        .unwrap()
        .write(&timegrapher_core::capture::Block {
            samples: vec![0.0; 48000 * 6],
            at: std::time::SystemTime::now(),
        })
        .unwrap();
    app.stream_tick();
    app.wait_for_save();
    assert_eq!(app.session.saved_to(), Some(take.as_path()));
    assert!(!take.join(note).exists(), "the note stays out of the take");
    let left = base.join("after a crash");
    let copy = left.join(scratch.file_name().unwrap());
    std::fs::create_dir_all(&copy).unwrap();
    for e in std::fs::read_dir(&scratch).unwrap().flatten() {
        if e.file_type().unwrap().is_file() {
            std::fs::copy(e.path(), copy.join(e.file_name())).unwrap();
        }
    }
    let kept = kept_sounds(&left);
    assert_eq!(kept.len(), 1);
    assert_eq!(kept[0].streamed_to.as_deref(), Some(take.as_path()));
    // Its notice offers the take, not the copy as sound never saved.
    app.kept = kept;
    let ctx = theme_ctx();
    let mut drawn = Vec::new();
    for _ in 0..2 {
        drawn = drawn_at(&ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| app.kept_notice(ui));
        }));
    }
    assert!(drawn.iter().any(|t| t.0 == "Open Take"), "{drawn:?}");
    assert!(
        !drawn.iter().any(|t| t.0.contains("wasn't saved")),
        "{drawn:?}"
    );
    app.capture = None;
    let _ = std::fs::remove_dir_all(&base);
}

/// A Power Reserve Test of a watch picked records into its folder as it
/// goes too, in the folder the test's end saves into, with no results file
/// until then: a crash 39 h in loses only seconds (Hugh, 5 Nov). Its end
/// saves into the same folder, with its results.
#[test]
fn a_power_reserve_test_records_into_its_folder_as_it_goes() {
    let base = std::env::temp_dir().join(format!("tg-app-stream-reserve-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();
    let root = base.join("Timegrapher Data");
    let mut lib = library::Library::create(&root).unwrap();
    lib.add(&library::Watch::new("Pepsi")).unwrap();
    let pepsi = lib.find("Pepsi").unwrap().watch.id.clone();
    drop(lib);
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
    let started = r.started_utc().to_string();
    app.session.recorder = Some(r);
    let _tx = queued_capture(&mut app, &base);
    app.reserve = Some(reserve::Test::new(app.position, true, started, 0.0));
    assert!(app.streams(), "a test of a watch picked records as it goes");
    app.stream_tick();
    app.wait_for_save();
    let take = app
        .session
        .saved_to()
        .expect("saved as it goes")
        .to_path_buf();
    let name = take.file_name().unwrap().to_string_lossy().into_owned();
    assert!(name.ends_with(" Power Reserve"), "{name}");
    assert!(!take.join(timegrapher_core::power_reserve::FILE).exists());
    assert!(app.message.is_none(), "{:?}", app.message);
    // With no watch picked, the end asks where it goes: nothing until then.
    app.watch_open = None;
    assert!(!app.streams());
    app.watch_open = Some(pepsi.clone());
    // Its end saves into the same folder, and nothing more streams.
    app.end_reserve();
    app.wait_for_save();
    assert_eq!(app.session.saved_to(), Some(take.as_path()));
    assert!(!app.streams());
    let _ = std::fs::remove_dir_all(&base);
}
