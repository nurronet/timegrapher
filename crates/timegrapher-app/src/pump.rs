//! Listening that carries on while the window isn't drawing.
//!
//! The desktop stops asking a hidden window to draw: a locked screen or a
//! covered window under Wayland, a minimised window on Windows, an app the
//! system has put to sleep on macOS. The window's frames are what save and
//! read the sound, so with no frames the sound piled up in memory, unsaved,
//! and the next frame froze the window while it caught up. A thread of its
//! own now does that work every few tenths of a second, whether the window
//! is drawing or not; the frames share the same app and read the sound
//! too while they draw. A file dialog waits until the frame has let the
//! app go ([`crate::pick`]), and a save copies the take on a thread of its
//! own, so neither holds this thread up. It also holds the computer awake
//! while listening, when Keep Awake is on.

use crate::app::TimegrapherApp;
use crate::applog;
use crate::awake::Awake;
use eframe::egui;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// How often the thread takes the sound that has come in.
const EVERY: Duration = Duration::from_millis(200);
/// How often the log notes that listening is still going.
const LOG_EVERY: Duration = Duration::from_secs(600);

/// The app, shared between the window's frames and the listening thread.
pub struct Shell {
    app: Arc<Mutex<TimegrapherApp>>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    /// Dropped last, after the app: on a Mac at quit, ends the process
    /// before eframe drops the window (#1071).
    _end: EndAfterApp,
}

/// See [`Shell`]'s last field.
struct EndAfterApp;

impl Drop for EndAfterApp {
    fn drop(&mut self) {
        #[cfg(target_os = "macos")]
        crate::app::mac_menu::end_process();
    }
}

impl Shell {
    pub fn new(app: TimegrapherApp) -> Self {
        let app = Arc::new(Mutex::new(app));
        let stop = Arc::new(AtomicBool::new(false));
        let spawned = {
            let (app, stop) = (app.clone(), stop.clone());
            std::thread::Builder::new()
                .name("listening".into())
                .spawn(move || run(&app, &stop))
        };
        let thread = match spawned {
            Ok(t) => Some(t),
            Err(e) => {
                applog::warn(&format!("the listening thread didn't start: {e}"));
                lock(&app).pump_fault = Some(e.to_string());
                None
            }
        };
        Shell {
            app,
            stop,
            thread,
            _end: EndAfterApp,
        }
    }

    pub(crate) fn app(&self) -> MutexGuard<'_, TimegrapherApp> {
        lock(&self.app)
    }

    fn finish(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl Drop for Shell {
    fn drop(&mut self) {
        self.finish();
    }
}

std::thread_local! {
    /// The parts of the frame being built and their times, for the log.
    static PARTS: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Run `f`, the part `name` of the frame being built, noting its time for
/// the frame log.
pub(crate) fn timed<R>(name: &str, f: impl FnOnce() -> R) -> R {
    let since = Instant::now();
    let r = f();
    part(name, since);
    r
}

/// Note the part `name` of the frame being built, begun at `since`, for
/// the frame log; nothing when it is off.
pub(crate) fn part(name: &str, since: Instant) {
    if frame_log() {
        let took = since.elapsed();
        PARTS.with(|p| p.borrow_mut().push(format!("{name} {took:?}")));
    }
}

/// Whether every frame's time goes in the log: with
/// `TIMEGRAPHER_FRAME_LOG` set, to see where a slow window spends it.
fn frame_log() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("TIMEGRAPHER_FRAME_LOG").is_some())
}

/// The app, even if a panic on the other thread left the lock poisoned:
/// the window should still show what was kept and let it be saved.
pub(crate) fn lock(app: &Mutex<TimegrapherApp>) -> MutexGuard<'_, TimegrapherApp> {
    app.lock().unwrap_or_else(|e| e.into_inner())
}

fn run(app: &Mutex<TimegrapherApp>, stop: &AtomicBool) {
    let mut log_at = Instant::now();
    let mut blocks = 0usize;
    let mut behind = false;
    let mut awake = Awake::default();
    while !stop.load(Ordering::Relaxed) {
        // Straight on while there is a backlog, letting the window in
        // between; otherwise a short rest.
        if behind {
            std::thread::yield_now();
        } else {
            std::thread::sleep(EVERY);
        }
        let started = Instant::now();
        let fault = awake.fault();
        let (taken, listening, heard, want_awake) = {
            let mut a = lock(app);
            a.awake_fault = fault;
            let taken = a.pump();
            (taken, a.running(), a.heard_s(), a.wants_awake())
        };
        awake.hold(want_awake);
        behind = taken > 0 && started.elapsed() >= crate::app::PUMP_BUDGET;
        blocks += taken;
        if !listening {
            log_at = Instant::now();
            blocks = 0;
        } else if log_at.elapsed() >= LOG_EVERY {
            applog::info(&format!(
                "still listening: {heard:.0} s heard, {blocks} blocks of sound taken in the last {} min",
                LOG_EVERY.as_secs() / 60
            ));
            log_at = Instant::now();
            blocks = 0;
        }
    }
}

impl eframe::App for Shell {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        let asked = Instant::now();
        let mut app = self.app();
        let waited = asked.elapsed();
        app.update(ctx, frame);
        drop(app);
        if frame_log() {
            let parts: Vec<String> = PARTS.with(|p| p.borrow_mut().drain(..).collect());
            applog::info(&format!(
                "frame: waited {waited:?} for the app, built in {:?}: {}",
                asked.elapsed() - waited,
                parts.join(", ")
            ));
        }
        if crate::pick::answer(&self.app, crate::pick::show) {
            ctx.request_repaint();
        }
    }

    fn clear_color(&self, visuals: &egui::Visuals) -> [f32; 4] {
        self.app().clear_color(visuals)
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        self.app().save(storage);
    }

    fn on_exit(&mut self, gl: Option<&eframe::glow::Context>) {
        #[cfg(target_os = "macos")]
        crate::app::mac_menu::before_close();
        self.finish();
        self.app().on_exit(gl);
    }
}
