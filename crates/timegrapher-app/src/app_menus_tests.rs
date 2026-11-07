//! The window's menus: what each one lists, each item's shortcut and
//! name, and what running an item does.

use super::*;
use crate::app::tests::{
    append_save_test_second, painted_text, save_failure_fixture, screen, synthetic, themed,
};

/// Draw `menu` on its own in one frame, as its drop-down shows it, and
/// give back the text painted.
fn drawn(
    app: &mut TimegrapherApp,
    ctx: &egui::Context,
    menu: fn(&mut TimegrapherApp, &mut egui::Ui),
) -> String {
    let out = ctx.run(screen(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| menu(app, ui));
    });
    painted_text(&out)
}

/// Each item with a shortcut has its own, on the platform's command key;
/// only Save As adds Shift, so Save's key is not Save As's. Nothing else
/// has one.
#[test]
fn each_shortcut_is_one_command_key_of_its_own() {
    let keys: Vec<_> = MenuCommand::WITH_KEYS
        .iter()
        .map(|&c| menu_shortcut(c).unwrap_or_else(|| panic!("{c:?} has no shortcut")))
        .collect();
    for (i, k) in keys.iter().enumerate() {
        assert!(k.modifiers.command, "{k:?}");
        assert!(!keys[..i].contains(k), "{k:?} twice");
    }
    let save = menu_shortcut(MenuCommand::Save).unwrap();
    let save_as = menu_shortcut(MenuCommand::SaveAs).unwrap();
    assert_eq!(
        (save.logical_key, save_as.logical_key),
        (egui::Key::S, egui::Key::S)
    );
    assert!(!save.modifiers.shift && save_as.modifiers.shift);
    assert_eq!(
        menu_shortcut(MenuCommand::Settings).unwrap().logical_key,
        egui::Key::Comma
    );
    for c in [
        MenuCommand::OpenFolder,
        MenuCommand::SaveAnalysisOnly,
        MenuCommand::NewSession,
        MenuCommand::RemoveMasks,
        MenuCommand::Readings,
        MenuCommand::CompactReadings,
        MenuCommand::Pane(Pane::Scope),
        MenuCommand::ResetPanes,
        MenuCommand::NewWatch,
        MenuCommand::FindInWatches,
        MenuCommand::CheckForUpdates,
        MenuCommand::ReportProblem,
    ] {
        assert_eq!(menu_shortcut(c), None, "{c:?}");
    }
}

/// The File, Edit, Watches and Help menus list their items, each with
/// its shortcut; before anything is heard Save is "Save…", and Check for
/// Updates is not yet checking.
#[test]
fn the_menus_list_their_items_and_shortcuts() {
    let mut app = TimegrapherApp::with_devices(Vec::new(), None);
    let ctx = themed();
    let key = |c| ctx.format_shortcut(&menu_shortcut(c).unwrap());
    let file = drawn(&mut app, &ctx, TimegrapherApp::file_menu);
    for item in [
        "Open Recording…",
        "Open Folder…",
        "Save…",
        "Save As…",
        "Save Analysis Only…",
        "New Session",
        "Quit",
    ] {
        assert!(file.lines().any(|l| l == item), "{item} not in {file}");
    }
    for c in [
        MenuCommand::OpenRecording,
        MenuCommand::Save,
        MenuCommand::SaveAs,
        MenuCommand::Quit,
    ] {
        assert!(file.contains(&key(c)), "{c:?}'s key not in {file}");
    }
    let edit = drawn(&mut app, &ctx, TimegrapherApp::edit_menu);
    assert!(edit.contains("Remove All Masks…"), "{edit}");
    assert!(edit.contains("Settings…"), "{edit}");
    assert!(edit.contains(&key(MenuCommand::Settings)), "{edit}");
    let watches = drawn(&mut app, &ctx, TimegrapherApp::watches_menu);
    assert!(watches.contains("New Watch…"), "{watches}");
    assert!(watches.contains("Find in Watches…"), "{watches}");
    let help = drawn(&mut app, &ctx, TimegrapherApp::help_menu);
    assert!(help.contains("Check for Updates…"), "{help}");
    assert!(help.contains("Report a Problem…"), "{help}");
    assert!(!help.contains("Checking"), "{help}");
    assert_eq!(
        app.menu_name(MenuCommand::CheckForUpdates),
        Some("Check for Updates…")
    );
    assert_eq!(app.menu_name(MenuCommand::Quit), None);
}

/// The View menu has both views, the readings' two items, a box for
/// every pane in the icon bar's order and Reset Panes; the Analysis
/// panes are in a menu of their own.
#[test]
fn the_view_menu_has_both_views_and_every_pane() {
    let mut app = TimegrapherApp::with_devices(Vec::new(), None);
    let ctx = themed();
    let view = drawn(&mut app, &ctx, TimegrapherApp::view_menu);
    let lines: Vec<&str> = view.lines().collect();
    let at = |name: &str| {
        lines
            .iter()
            .position(|l| *l == name)
            .unwrap_or_else(|| panic!("{name} not in {view}"))
    };
    let order: Vec<usize> = ["Bench", "Watches", "Readings", "Compact Readings"]
        .into_iter()
        .chain(Pane::CORE.iter().map(|p| p.title()))
        .chain(["Analysis"])
        .chain(Pane::TESTS.iter().map(|p| p.title()))
        .chain(["Reset Panes"])
        .map(at)
        .collect();
    assert!(order.windows(2).all(|w| w[0] < w[1]), "{view}");
    for c in [
        MenuCommand::View(View::Bench),
        MenuCommand::View(View::Watches),
    ] {
        let k = ctx.format_shortcut(&menu_shortcut(c).unwrap());
        assert!(view.contains(&k), "{k} not in {view}");
    }
    // The Analysis panes wait in their own menu, closed.
    assert!(!lines.contains(&Pane::ANALYSIS[0].title()), "{view}");
}

/// A view's item picks it and is ticked while it shows; Find in Watches
/// goes to Watches with the search ready to type in; New Watch… opens
/// the form.
#[test]
fn the_view_and_watches_items_go_where_they_say() {
    let mut app = TimegrapherApp::with_devices(Vec::new(), None);
    let ctx = themed();
    assert!(app.menu_checked(MenuCommand::View(View::Bench)));
    app.run_menu_command(&ctx, MenuCommand::View(View::Watches));
    assert_eq!(app.view, View::Watches);
    assert!(app.menu_checked(MenuCommand::View(View::Watches)));
    assert!(!app.menu_checked(MenuCommand::View(View::Bench)));
    app.run_menu_command(&ctx, MenuCommand::View(View::Bench));
    assert_eq!(app.view, View::Bench);
    assert!(!app.watch_search_focus);
    app.run_menu_command(&ctx, MenuCommand::FindInWatches);
    assert_eq!(app.view, View::Watches);
    assert!(app.watch_search_focus);
    assert!(app.watch_form.is_none());
    app.run_menu_command(&ctx, MenuCommand::NewWatch);
    assert!(app.watch_form.is_some());
}

/// A pane's item shows a hidden pane and hides a shown one, its tick
/// following; Reset Panes puts back the readings and the panes the app
/// starts with. Both are the Bench's, dimmed in Watches.
#[test]
fn a_panes_item_toggles_it_and_reset_panes_puts_them_back() {
    let mut app = TimegrapherApp::with_devices(Vec::new(), None);
    let ctx = themed();
    let charts = MenuCommand::Pane(Pane::Charts);
    let strip = MenuCommand::Pane(Pane::Strip);
    assert!(!app.menu_checked(charts));
    assert!(app.menu_checked(strip));
    app.run_menu_command(&ctx, charts);
    assert!(app.menu_checked(charts));
    app.run_menu_command(&ctx, strip);
    assert!(!app.menu_checked(strip));
    app.run_menu_command(&ctx, MenuCommand::Readings);
    assert!(!app.show_readings);
    app.view = View::Watches;
    assert!(!app.menu_enabled(MenuCommand::ResetPanes));
    assert!(!app.menu_enabled(charts));
    app.run_menu_command(&ctx, MenuCommand::ResetPanes);
    assert!(!app.menu_checked(strip), "Reset Panes ran in Watches");
    app.view = View::Bench;
    app.run_menu_command(&ctx, MenuCommand::ResetPanes);
    assert!(app.show_readings);
    assert!(app.menu_checked(strip));
    assert!(!app.menu_checked(charts));
}

/// New Session is dimmed before anything is heard and does nothing.
#[test]
fn new_session_is_dimmed_before_anything_is_heard() {
    let ctx = themed();
    let mut fresh = TimegrapherApp::with_devices(Vec::new(), None);
    assert!(!fresh.menu_enabled(MenuCommand::NewSession));
    fresh.run_menu_command(&ctx, MenuCommand::NewSession);
    assert_eq!(fresh.confirm, None);
}

/// A take with nothing to save is cleared at once, nothing asked.
#[test]
fn new_session_clears_a_take_with_nothing_to_save() {
    let ctx = themed();
    let mut app = synthetic(30.0, 5.0);
    assert!(!app.needs_save_confirmation());
    assert!(app.menu_enabled(MenuCommand::NewSession));
    app.run_menu_command(&ctx, MenuCommand::NewSession);
    assert_eq!(app.confirm, None);
    assert!(app.live.is_none(), "the take is still shown");
}

/// A take with sound not yet saved is kept, and New Session asks first.
#[test]
fn new_session_asks_before_clearing_unsaved_sound() {
    let ctx = themed();
    let (mut app, base) = save_failure_fixture("menu-new-session");
    app.save_into(&base.join("saved"));
    app.wait_for_save();
    for second in 2..=8 {
        append_save_test_second(&mut app, second);
    }
    assert!(app.needs_save_confirmation());
    app.run_menu_command(&ctx, MenuCommand::NewSession);
    assert_eq!(app.confirm, Some(Confirm::NewSession));
    assert!(app.live.is_some(), "the take was cleared before asking");
    let _ = std::fs::remove_dir_all(&base);
}

/// Report a Problem… opens its window over the app.
#[test]
fn report_a_problem_opens_its_window() {
    let mut app = synthetic(10.0, 5.0);
    let ctx = themed();
    // A window's first frame only sizes it: the second paints it.
    let shown = |app: &mut TimegrapherApp| {
        let mut out = None;
        for _ in 0..2 {
            out = Some(ctx.run(screen(), |ctx| {
                app.report.ui(ctx);
            }));
        }
        painted_text(&out.unwrap()).contains("Report a Problem")
    };
    assert!(!shown(&mut app));
    app.run_menu_command(&ctx, MenuCommand::ReportProblem);
    assert!(shown(&mut app));
}
