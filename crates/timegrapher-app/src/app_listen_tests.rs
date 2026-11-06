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

/// A Quick Listen kept in Unsorted Takes with 9 s heard, its save into
/// the take of the last 6 s held part way, as a long copy would be, until
/// the sender returned sends (or 10 s pass); 2 s more heard since.
struct HeldSave {
    app: TimegrapherApp,
    base: PathBuf,
    take: PathBuf,
    go: std::sync::mpsc::Sender<()>,
    _capture: std::sync::mpsc::Sender<Event>,
}

/// Hold the save being written part way, as a long copy would be, until
/// the sender returned sends (or 10 s pass).
fn hold_save(app: &mut TimegrapherApp) -> std::sync::mpsc::Sender<()> {
    let (go, held) = std::sync::mpsc::channel::<()>();
    let mut s = app.saving.take().expect("a save into the take");
    let copy = s.thread;
    s.thread = std::thread::spawn(move || {
        let _ = held.recv_timeout(std::time::Duration::from_secs(10));
        copy.join().unwrap()
    });
    app.saving = Some(s);
    go
}

/// A Quick Listen heard for 3 s, its sound not yet kept.
fn listening(name: &str) -> (TimegrapherApp, PathBuf, std::sync::mpsc::Sender<Event>) {
    let base = std::env::temp_dir().join(format!("tg-app-{name}-{}", std::process::id()));
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
    r.write(&silence(3, 0)).unwrap();
    app.session.recorder = Some(r);
    let capture = queued_capture(&mut app, &base);
    (app, base, capture)
}

/// `s` seconds of silence heard `at` seconds in.
fn silence(s: u64, at: u64) -> timegrapher_core::capture::Block {
    timegrapher_core::capture::Block {
        samples: vec![0.0; 48000 * s as usize],
        at: std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_760_000_000 + at),
    }
}

/// A watch named `name` added to the Watches folder, by id.
fn add_watch(app: &mut TimegrapherApp, name: &str) -> String {
    match &mut app.library {
        Ok(Some(l)) => {
            l.add(&library::Watch::new(name)).unwrap();
            l.find(name).unwrap().watch.id.clone()
        }
        _ => unreachable!(),
    }
}

/// The folder a watch's takes are kept in.
fn takes_of(app: &TimegrapherApp, name: &str) -> PathBuf {
    match &app.library {
        Ok(Some(l)) => l.find(name).unwrap().dir.join(library::TAKES),
        _ => unreachable!(),
    }
}

fn held_save(name: &str) -> HeldSave {
    let (mut app, base, capture) = listening(name);
    app.save_again();
    app.wait_for_save();
    let take = app.session.saved_to().expect("kept").to_path_buf();
    let rec = app.session.recorder.as_mut().unwrap();
    rec.write(&silence(6, 3)).unwrap();
    app.stream_tick();
    assert_eq!(app.saving.as_ref().map(|s| s.streamed), Some(Streamed::Yes));
    let go = hold_save(&mut app);
    let rec = app.session.recorder.as_mut().unwrap();
    rec.write(&silence(2, 9)).unwrap();
    HeldSave {
        app,
        base,
        take,
        go,
        _capture: capture,
    }
}

impl HeldSave {
    /// The seconds of sound in the take.
    fn take_s(&self) -> f64 {
        let flac = self.take.join("audio-001.flac");
        let wav = self.take.join("audio-001.wav");
        let kept = if flac.exists() { flac } else { wav };
        timegrapher_core::audio::load(&kept).unwrap().samples.len() as f64 / 48000.0
    }

    fn held(&self) -> bool {
        self.app
            .saving
            .as_ref()
            .is_some_and(|s| s.streamed == Streamed::Yes)
    }

    /// Let the held save go, and wait until it is written.
    fn release(&mut self) {
        self.go.send(()).unwrap();
        while self.held() {
            self.app.poll_saving();
        }
    }
}

impl Drop for HeldSave {
    fn drop(&mut self) {
        let _ = self.go.send(());
        self.app.wait_for_save();
        let _ = std::fs::remove_dir_all(&self.base);
    }
}

/// A Stop while a take's save into its folder is still being written (the
/// first can copy a whole hour) waits its turn, rather than the window
/// waiting on the copy (Vernon's point on #1201); once that save is
/// written, the Stop's save adds the rest, and an ask after it waits for
/// that too (Felix).
#[test]
fn a_stop_waits_its_turn_behind_a_save_into_its_take() {
    let mut h = held_save("stream-turn");
    h.app.stop_listening();
    assert!(h.held(), "the Stop didn't wait on the copy");
    assert_eq!(h.app.after_save.len(), 1);
    assert!(
        h.app.saving_busy(),
        "nothing jumps the Stop waiting its turn"
    );
    // Closing the window waits for it too.
    h.app.close_after_save = true;
    assert!(!h.app.closes_after_save());
    let opened = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let then = opened.clone();
    let open = move |_: &mut TimegrapherApp| then.store(true, std::sync::atomic::Ordering::SeqCst);
    assert!(h.app.behind_save(open));
    h.release();
    // Asks run on the window's frame, not the listening thread's.
    assert_eq!(h.app.after_save.len(), 2);
    h.app.run_after_save();
    assert!(
        h.app.saving.is_some(),
        "the Stop's save, once the other is written"
    );
    assert_eq!(h.app.after_save.len(), 1, "the next ask waits for it");
    assert!(!h.app.closes_after_save());
    h.app.wait_for_save();
    h.app.run_after_save();
    assert!(h.app.after_save.is_empty());
    assert!(opened.load(std::sync::atomic::Ordering::SeqCst));
    assert!(h.app.closes_after_save());
    assert!(!h.app.needs_save_confirmation());
    assert_eq!(h.take_s(), 11.0);
}

/// A Stop while a Quick Listen's first save into Unsorted Takes is still
/// written waits for it, then saves the rest into the take (#1216, found
/// by Walter): it was refused, leaving the take short.
#[test]
fn a_stop_during_a_takes_first_save_saves_the_rest_after_it() {
    let base = std::env::temp_dir().join(format!("tg-app-first-save-{}", std::process::id()));
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
    let block = |s: u64, at: u64| timegrapher_core::capture::Block {
        samples: vec![0.0; 48000 * s as usize],
        at: start + std::time::Duration::from_secs(at),
    };
    r.write(&block(3, 0)).unwrap();
    app.session.recorder = Some(r);
    let _tx = queued_capture(&mut app, &base);
    // Kept in Unsorted Takes: the user's own save, held part way.
    app.save_again();
    assert_eq!(app.saving.as_ref().map(|s| s.streamed), Some(Streamed::No));
    let go = hold_save(&mut app);
    let rec = app.session.recorder.as_mut().unwrap();
    rec.write(&block(2, 3)).unwrap();
    app.stop_listening();
    assert_eq!(app.after_save.len(), 1, "the Stop waits for the first save");
    go.send(()).unwrap();
    while app.saving.is_some() {
        app.poll_saving();
    }
    app.run_after_save();
    assert!(
        app.saving.is_some(),
        "the Stop's save, once the first is written"
    );
    app.wait_for_save();
    assert!(app.unsaved_s() < 1.0, "{}", app.unsaved_s());
    assert!(!app.needs_save_confirmation());
    let take = app.session.saved_to().expect("kept").to_path_buf();
    let flac = take.join("audio-001.flac");
    let kept = if flac.exists() {
        flac
    } else {
        take.join("audio-001.wav")
    };
    let samples = timegrapher_core::audio::load(&kept).unwrap().samples.len();
    assert_eq!(samples, 5 * 48000);
    let _ = std::fs::remove_dir_all(&base);
}

/// A Quick Listen kept in Unsorted Takes and then picked into a watch on
/// the Watch line is filed under it, after the save being written, and
/// records into its folder there; Unsorted Takes keeps nothing (#1217,
/// found by Walter).
#[test]
fn a_kept_quick_listen_picked_into_a_watch_is_filed_under_it() {
    let mut h = held_save("file-kept");
    let alpha = add_watch(&mut h.app, "Alpha");
    let unsorted = h.take.parent().unwrap().to_path_buf();
    // Picked while its save into Unsorted Takes is still written.
    h.app.line_pick(Some(alpha.clone()), false);
    assert!(h.app.open_watch().is_some_and(|w| w.watch.id == alpha));
    assert_eq!(h.app.after_save.len(), 1, "filed once the save is written");
    h.release();
    h.app.run_after_save();
    let filed = h.app.session.saved_to().expect("kept").to_path_buf();
    let watch_takes = takes_of(&h.app, "Alpha");
    assert_eq!(filed.parent(), Some(watch_takes.as_path()), "{filed:?}");
    assert!(!h.take.exists());
    assert_eq!(std::fs::read_dir(&unsorted).unwrap().count(), 0);
    // Its next saves go into it there, not into a new folder.
    h.take = filed.clone();
    h.app.save_again();
    h.app.wait_for_save();
    assert_eq!(h.app.session.saved_to(), Some(filed.as_path()));
    assert_eq!(std::fs::read_dir(&watch_takes).unwrap().count(), 1);
    assert_eq!(h.take_s(), 11.0);
}

/// A watch picked while a Quick Listen's first save is still copying,
/// before the take has a folder, files it once the copy is written
/// (Walter's HOLD on #1229).
#[test]
fn a_quick_listen_picked_into_a_watch_during_its_first_save_is_filed_under_it() {
    let (mut app, base, _capture) = listening("file-first");
    let alpha = add_watch(&mut app, "Alpha");
    app.save_again();
    let go = hold_save(&mut app);
    assert_eq!(app.session.saved_to(), None, "the first copy still written");
    app.line_pick(Some(alpha), false);
    assert_eq!(app.after_save.len(), 1, "filed once the copy is written");
    go.send(()).unwrap();
    app.wait_for_save();
    app.run_after_save();
    let filed = app.session.saved_to().expect("kept").to_path_buf();
    assert_eq!(filed.parent(), Some(takes_of(&app, "Alpha").as_path()));
    let unsorted = match &app.library {
        Ok(Some(l)) => l.unsorted_dir().unwrap(),
        _ => unreachable!(),
    };
    assert_eq!(std::fs::read_dir(&unsorted).unwrap().count(), 0);
    let _ = std::fs::remove_dir_all(&base);
}

/// A take kept in one watch then another while a save into it is written
/// is filed under the last, with no error (Vernon's HOLD on #1229).
#[test]
fn two_watches_picked_during_a_save_file_the_take_under_the_last() {
    let mut h = held_save("file-twice");
    let alpha = add_watch(&mut h.app, "Alpha");
    let beta = add_watch(&mut h.app, "Beta");
    // Paused, so each Keep in a Watch picks again (one still heard keeps
    // the first watch picked).
    h.app.pause();
    h.app.keep_in_watch(&alpha);
    h.app.keep_in_watch(&beta);
    assert!(h.app.after_save.len() >= 2, "both wait on the save");
    h.release();
    while h.app.saving.is_some() || !h.app.after_save.is_empty() {
        h.app.wait_for_save();
        h.app.run_after_save();
    }
    assert!(
        !h.app.message.as_ref().is_some_and(|m| m.1),
        "{:?}",
        h.app.message
    );
    let filed = h.app.session.saved_to().expect("kept").to_path_buf();
    assert_eq!(filed.parent(), Some(takes_of(&h.app, "Beta").as_path()));
    assert_eq!(h.app.watch, "Beta");
    assert!(
        !takes_of(&h.app, "Alpha").exists()
            || std::fs::read_dir(takes_of(&h.app, "Alpha"))
                .unwrap()
                .count()
                == 0
    );
    h.take = filed;
    assert_eq!(h.take_s(), 11.0);
}

/// Quitting with a Stop waiting on a save into the take still saves the
/// take's last seconds (Trevor's HOLD on #1214).
#[test]
fn a_quit_with_a_stop_waiting_saves_the_last_seconds() {
    let mut h = held_save("stream-quit");
    h.app.stop_listening();
    assert_eq!(h.app.after_save.len(), 1);
    let _ = h.go.send(());
    eframe::App::on_exit(&mut h.app, None);
    assert!(h.app.after_save.is_empty());
    assert!(h.app.saving.is_none());
    assert_eq!(h.take_s(), 11.0);
}

/// Each ask that would wait on a save into the take waits its turn
/// instead (Trevor's HOLD on #1214).
#[test]
fn each_ask_waits_its_turn_behind_a_save_into_its_take() {
    let mut h = held_save("stream-asks");
    h.app.pause();
    let elsewhere = h.base.join("elsewhere.wav");
    type Ask = Box<dyn Fn(&mut TimegrapherApp)>;
    let asks: [(&str, Ask); 7] = [
        ("Save", Box::new(|a| a.save_again())),
        ("Save As", Box::new(|a| a.save_recording())),
        (
            "Open",
            Box::new(move |a| a.open_recording(&elsewhere, false)),
        ),
        ("Watch line", Box::new(|a| a.line_pick(None, true))),
        ("New Take", Box::new(|a| a.new_take(NewTake::Listen, true))),
        ("Watches list", Box::new(|a| a.list_pick("w".into()))),
        ("Quick Listen", Box::new(|a| a.quick_listen(true))),
    ];
    for (name, ask) in asks {
        h.app.after_save.clear();
        let at = std::time::Instant::now();
        ask(&mut h.app);
        assert!(h.held(), "{name} waited on the copy");
        assert!(at.elapsed() < std::time::Duration::from_secs(5), "{name}");
        assert_eq!(h.app.after_save.len(), 1, "{name} waits its turn");
    }
    h.app.after_save.clear();
    h.release();
}

/// What the delete dialog for the first kept sound says.
fn delete_says(ctx: &egui::Context, app: &mut TimegrapherApp) -> Vec<(String, egui::Pos2)> {
    app.kept_delete_ask = Some(app.kept[0].dir.clone());
    let mut drawn = Vec::new();
    for _ in 0..2 {
        drawn = drawn_at(&ctx.run(egui::RawInput::default(), |ctx| app.kept_delete_dialog(ctx)));
    }
    app.kept_delete_ask = None;
    drawn
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
    let said = delete_says(&ctx, &mut app);
    assert!(
        said.iter().any(|t| t.0.starts_with("Its take keeps")),
        "{said:?}"
    );
    // Vernon's HOLD: with the take gone, the copy is the only one, and
    // everything says so; the note goes, so a move takes none with it.
    std::fs::remove_dir_all(&take).unwrap();
    app.kept = kept_sounds(&left);
    assert_eq!(app.kept[0].streamed_to, None);
    assert!(!copy.join(note).exists());
    let said = delete_says(&ctx, &mut app);
    assert!(
        said.iter()
            .any(|t| t.0.contains("never saved anywhere else")),
        "{said:?}"
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
