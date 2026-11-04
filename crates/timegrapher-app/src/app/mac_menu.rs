//! On a Mac the menus go in the menu bar at the top of the screen, as a
//! Mac app's do ("Top of the Window", #1000, part 2): Timegrapher (About,
//! Settings…, Hide, Quit), File, Edit, View, Watches and Help, with their
//! ⌘ shortcuts. Each item runs the same [`MenuCommand`] as the window's
//! own menus elsewhere; this file builds the items, keeps them dimmed,
//! ticked and named as the app is, and passes on what was chosen.

use super::menus::{menu_shortcut, MenuCommand};
use super::{Pane, TimegrapherApp, VIEWS};
use eframe::egui;
use muda::accelerator::{Accelerator, Code, Modifiers};
use muda::{
    AboutMetadata, CheckMenuItem, Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem, Submenu,
};
use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;

/// Set by `main` before the window opens: tests draw the app off the
/// main thread, where there is no menu bar, so they keep the window's.
static WANTED: AtomicBool = AtomicBool::new(false);

/// Ask for the menus in the menu bar at the top of the screen. Also
/// turns off macOS's window tabs before the window opens: the app has one
/// window, and with tabs allowed macOS adds Show Tab Bar to the View
/// menu, which gives the window a second title bar.
pub(crate) fn want() {
    WANTED.store(true, Ordering::Relaxed);
    // SAFETY: a class method of NSWindow, taking a BOOL and returning
    // nothing, called on the main thread before any window exists.
    unsafe {
        let _: () = objc2::msg_send![
            objc2::class!(NSWindow),
            setAllowsAutomaticWindowTabbing: false
        ];
    }
    // The app puts nothing on a Touch Bar. Left on, AppKit's Touch Bar
    // finder watches the window's views and, once the window has gone
    // at quit, stops watching one it no longer holds and aborts (#1071).
    let key = objc2_foundation::NSString::from_str("NSFunctionBarAPIEnabled");
    // SAFETY: NSUserDefaults' shared defaults, and setBool:forKey: with
    // a BOOL and an NSString, on the main thread before NSApplication
    // starts.
    unsafe {
        let defaults: *mut objc2::runtime::AnyObject =
            objc2::msg_send![objc2::class!(NSUserDefaults), standardUserDefaults];
        if !defaults.is_null() {
            let _: () = objc2::msg_send![defaults, setBool: false, forKey: &*key];
        }
    }
}

/// Before the window goes at quit: each window's first responder is
/// the window itself, so nothing AppKit watches in its views (the Touch
/// Bar's finder) is left watching a view about to go (#1071).
pub(crate) fn before_close() {
    if !WANTED.load(Ordering::Relaxed) {
        return;
    }
    // SAFETY: NSApplication's shared instance and its key and main
    // windows, each nil or a live NSWindow, and makeFirstResponder: with
    // nil, which makes the window its own first responder; on the main
    // thread, where eframe calls `on_exit`.
    unsafe {
        let app: *mut objc2::runtime::AnyObject =
            objc2::msg_send![objc2::class!(NSApplication), sharedApplication];
        if app.is_null() {
            return;
        }
        let key: *mut objc2::runtime::AnyObject = objc2::msg_send![app, keyWindow];
        let main: *mut objc2::runtime::AnyObject = objc2::msg_send![app, mainWindow];
        for w in [key, main] {
            if !w.is_null() {
                let none: *mut objc2::runtime::AnyObject = std::ptr::null_mut();
                let _: bool = objc2::msg_send![w, makeFirstResponder: none];
            }
        }
    }
}

/// An item that runs a command: plain, or ticked when on.
enum Item {
    Plain(MenuItem),
    Check(CheckMenuItem),
}

impl Item {
    fn id(&self) -> &MenuId {
        match self {
            Item::Plain(i) => i.id(),
            Item::Check(i) => i.id(),
        }
    }
}

/// The menu bar, its items and the commands they run, and what has been
/// chosen since the last frame.
struct Bar {
    _menu: Menu,
    items: Vec<(Item, MenuCommand)>,
    chosen: mpsc::Receiver<MenuId>,
}

enum State {
    Unbuilt,
    /// The menu bar couldn't be made: the window keeps its menus.
    Failed,
    Built(Bar),
}

thread_local! {
    static BAR: RefCell<State> = const { RefCell::new(State::Unbuilt) };
}

/// Whether the menus are in the menu bar at the top of the screen.
pub(crate) fn shown() -> bool {
    BAR.with_borrow(|s| matches!(s, State::Built(_)))
}

impl TimegrapherApp {
    /// Make the menu bar the first time; then, each frame, dim, tick and
    /// name its items as the app is, and run what was chosen.
    pub(super) fn mac_menus(&mut self, ctx: &egui::Context) {
        if !WANTED.load(Ordering::Relaxed) {
            return;
        }
        let chosen: Vec<MenuCommand> = BAR.with_borrow_mut(|state| {
            if matches!(state, State::Unbuilt) {
                *state = match build(ctx) {
                    Ok(bar) => State::Built(bar),
                    Err(e) => {
                        crate::applog::warn(&format!("No Mac menu bar, menus in the window: {e}"));
                        State::Failed
                    }
                };
            }
            let State::Built(bar) = state else {
                return Vec::new();
            };
            for (item, c) in &bar.items {
                self.keep(item, *c);
            }
            bar.chosen
                .try_iter()
                .filter_map(|id| bar.items.iter().find(|(i, _)| i.id() == &id))
                .map(|(_, c)| *c)
                .collect()
        });
        for c in chosen {
            self.run_menu_command(ctx, c);
        }
    }

    /// Bring `item` in line with the app: dimmed, ticked and named as
    /// the window's menus would draw it, touching only what changed.
    fn keep(&self, item: &Item, c: MenuCommand) {
        let enabled = self.menu_enabled(c);
        let name = self.menu_name(c);
        match item {
            Item::Plain(i) => {
                if i.is_enabled() != enabled {
                    i.set_enabled(enabled);
                }
                if let Some(n) = name.filter(|n| i.text() != *n) {
                    i.set_text(n);
                }
            }
            Item::Check(i) => {
                if i.is_enabled() != enabled {
                    i.set_enabled(enabled);
                }
                let on = self.menu_checked(c);
                if i.is_checked() != on {
                    i.set_checked(on);
                }
            }
        }
    }
}

/// `c`'s shortcut as the menu bar writes it: the same keys the window
/// reads, with ⌘ for the command key.
fn accelerator(c: MenuCommand) -> Option<Accelerator> {
    let k = menu_shortcut(c)?;
    let code = match k.logical_key {
        egui::Key::O => Code::KeyO,
        egui::Key::S => Code::KeyS,
        egui::Key::Q => Code::KeyQ,
        egui::Key::Num1 => Code::Digit1,
        egui::Key::Num2 => Code::Digit2,
        egui::Key::Comma => Code::Comma,
        _ => return None,
    };
    let mut mods = Modifiers::SUPER;
    if k.modifiers.shift {
        mods |= Modifiers::SHIFT;
    }
    Some(Accelerator::new(Some(mods), code))
}

/// The items as they are made, each with the command it runs.
#[derive(Default)]
struct Items(Vec<(Item, MenuCommand)>);

impl Items {
    fn plain(&mut self, into: &Submenu, name: &str, c: MenuCommand) -> muda::Result<()> {
        let i = MenuItem::new(name, true, accelerator(c));
        into.append(&i)?;
        self.0.push((Item::Plain(i), c));
        Ok(())
    }

    fn check(&mut self, into: &Submenu, name: &str, c: MenuCommand) -> muda::Result<()> {
        let i = CheckMenuItem::new(name, true, false, accelerator(c));
        into.append(&i)?;
        self.0.push((Item::Check(i), c));
        Ok(())
    }
}

/// Make the menu bar and put it at the top of the screen; what is chosen
/// in it wakes the app.
fn build(ctx: &egui::Context) -> muda::Result<Bar> {
    let mut items = Items::default();
    let line = PredefinedMenuItem::separator;
    let app = Submenu::new("Timegrapher", true);
    let about = AboutMetadata {
        name: Some("Timegrapher".into()),
        version: Some(env!("TIMEGRAPHER_LONG_VERSION").into()),
        ..Default::default()
    };
    app.append(&PredefinedMenuItem::about(
        Some("About Timegrapher"),
        Some(about),
    ))?;
    app.append(&line())?;
    items.plain(&app, "Settings…", MenuCommand::Settings)?;
    app.append(&line())?;
    app.append(&PredefinedMenuItem::hide(Some("Hide Timegrapher")))?;
    app.append(&PredefinedMenuItem::hide_others(None))?;
    app.append(&PredefinedMenuItem::show_all(None))?;
    app.append(&line())?;
    // The app's own Quit, not the system's: it asks about an unsaved
    // take first, as closing the window does.
    items.plain(&app, "Quit Timegrapher", MenuCommand::Quit)?;

    let file = Submenu::new("File", true);
    items.plain(&file, "Open Recording…", MenuCommand::OpenRecording)?;
    items.plain(&file, "Open Folder…", MenuCommand::OpenFolder)?;
    file.append(&line())?;
    items.plain(&file, "Save…", MenuCommand::Save)?;
    items.plain(&file, "Save As…", MenuCommand::SaveAs)?;
    items.plain(&file, "Save Analysis Only…", MenuCommand::SaveAnalysisOnly)?;
    items.plain(&file, "New Session", MenuCommand::NewSession)?;

    let edit = Submenu::new("Edit", true);
    items.plain(&edit, "Remove All Masks…", MenuCommand::RemoveMasks)?;

    let view = Submenu::new("View", true);
    for (v, name, _) in VIEWS {
        items.check(&view, name, MenuCommand::View(v))?;
    }
    view.append(&line())?;
    items.check(&view, "Readings", MenuCommand::Readings)?;
    items.check(&view, "Compact Readings", MenuCommand::CompactReadings)?;
    for p in Pane::CORE {
        items.check(&view, p.title(), MenuCommand::Pane(p))?;
    }
    let analysis = Submenu::new("Analysis", true);
    for p in Pane::ANALYSIS {
        items.check(&analysis, p.title(), MenuCommand::Pane(p))?;
    }
    view.append(&analysis)?;
    view.append(&line())?;
    for p in Pane::TESTS {
        items.check(&view, p.title(), MenuCommand::Pane(p))?;
    }
    view.append(&line())?;
    items.plain(&view, "Reset Panes", MenuCommand::ResetPanes)?;

    let watches = Submenu::new("Watches", true);
    items.plain(&watches, "New Watch…", MenuCommand::NewWatch)?;
    items.plain(&watches, "Find in Watches…", MenuCommand::FindInWatches)?;

    let help = Submenu::new("Help", true);
    items.plain(&help, "Check for Updates…", MenuCommand::CheckForUpdates)?;
    help.append(&line())?;
    items.plain(&help, "Report a Problem…", MenuCommand::ReportProblem)?;

    let menu = Menu::new();
    menu.append_items(&[&app, &file, &edit, &view, &watches, &help])?;
    menu.init_for_nsapp();
    help.set_as_help_menu_for_nsapp();
    let (tx, chosen) = mpsc::channel();
    let ctx = ctx.clone();
    MenuEvent::set_event_handler(Some(move |e: MenuEvent| {
        let _ = tx.send(e.id);
        ctx.request_repaint();
    }));
    Ok(Bar {
        _menu: menu,
        items: items.0,
        chosen,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The menu bar's shortcuts are the window's: the same keys, with ⌘.
    #[test]
    fn the_menu_bars_shortcuts_are_the_windows() {
        let save_as = accelerator(MenuCommand::SaveAs).unwrap();
        assert_eq!(
            save_as,
            Accelerator::new(Some(Modifiers::SUPER | Modifiers::SHIFT), Code::KeyS)
        );
        for c in MenuCommand::WITH_KEYS {
            assert!(accelerator(c).is_some(), "{c:?}");
        }
        assert_eq!(accelerator(MenuCommand::NewSession), None);
    }
}
