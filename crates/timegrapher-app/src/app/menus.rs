//! The menus: File, Edit, View, Watches and Help, what each item does
//! and its shortcut, and what File's Open asks before it opens. On a Mac
//! the same items go in the menu bar at the top of the screen
//! (`mac_menu`); elsewhere they are in the window's top row.

use super::*;

/// What a menu item does, whichever menu bar it is in. Its shortcut and
/// both menu bars run it through [`TimegrapherApp::run_menu_command`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum MenuCommand {
    OpenRecording,
    OpenFolder,
    Save,
    SaveAs,
    SaveAnalysisOnly,
    NewSession,
    Quit,
    RemoveMasks,
    Settings,
    View(View),
    Readings,
    CompactReadings,
    Pane(Pane),
    ResetPanes,
    NewWatch,
    FindInWatches,
    UserGuide,
    CheckForUpdates,
    ReportProblem,
}

impl MenuCommand {
    /// The items with a shortcut, in the order the keys are read: Save
    /// As before Save, which would also take Cmd+Shift+S.
    pub(super) const WITH_KEYS: [MenuCommand; 7] = [
        MenuCommand::OpenRecording,
        MenuCommand::SaveAs,
        MenuCommand::Save,
        MenuCommand::Quit,
        MenuCommand::View(View::Bench),
        MenuCommand::View(View::Watches),
        MenuCommand::Settings,
    ];
}

/// An item's shortcut: the platform's command key (Ctrl, or ⌘ on a Mac)
/// with O to open, S to save, Shift+S to save as, Q to quit, 1 and 2 for
/// the views, and the comma for Settings.
pub(super) fn menu_shortcut(c: MenuCommand) -> Option<egui::KeyboardShortcut> {
    let key = match c {
        MenuCommand::OpenRecording => egui::Key::O,
        MenuCommand::Save | MenuCommand::SaveAs => egui::Key::S,
        MenuCommand::Quit => egui::Key::Q,
        MenuCommand::View(View::Bench) => egui::Key::Num1,
        MenuCommand::View(View::Watches) => egui::Key::Num2,
        MenuCommand::Settings => egui::Key::Comma,
        _ => return None,
    };
    let shift = if c == MenuCommand::SaveAs {
        egui::Modifiers::SHIFT
    } else {
        egui::Modifiers::NONE
    };
    Some(egui::KeyboardShortcut::new(
        egui::Modifiers::COMMAND | shift,
        key,
    ))
}

/// Whether a Mac's menu bar at the top of the screen has the menus, so
/// the window leaves its own out.
#[cfg(target_os = "macos")]
use super::mac_menu::shown as mac_menu_shown;
#[cfg(not(target_os = "macos"))]
fn mac_menu_shown() -> bool {
    false
}

impl TimegrapherApp {
    /// The one row across the very top: the menus, File, Edit, View,
    /// Watches and Help, unless a Mac's menu bar has them; the Bench |
    /// Watches switch; and an icon for each pane that shows or hides it,
    /// then the Settings gear.
    pub(super) fn menu_bar(&mut self, ui: &mut egui::Ui) {
        #[cfg(target_os = "macos")]
        self.mac_menus(ui.ctx());
        let in_window = !mac_menu_shown();
        egui::MenuBar::new().ui(ui, |ui| {
            if in_window {
                ui.menu_button("File", |ui| self.file_menu(ui));
                ui.menu_button("Edit", |ui| self.edit_menu(ui));
                ui.menu_button("View", |ui| self.view_menu(ui));
                ui.menu_button("Watches", |ui| self.watches_menu(ui));
                ui.menu_button("Help", |ui| self.help_menu(ui));
                ui.add_space(6.0);
                ui.separator();
            }
            let mut view = self.view;
            theme::segmented(ui, &mut view, &VIEWS);
            self.view = view;
            ui.add_space(6.0);
            ui.separator();
            // The pane icons are the Bench's: in Watches they hide and
            // leave their room, so the gear stays where it is.
            self.pane_icons(ui, self.view == View::Bench);
        });
    }

    /// Whether `c` can be run now: what dims its item.
    pub(super) fn menu_enabled(&self, c: MenuCommand) -> bool {
        let can_save = self.session.allows(Action::Save);
        let saving = self.saving_asked();
        match c {
            // A take recording into its folder needs no Save Again.
            MenuCommand::Save => self.marks_unsaved() || (can_save && !saving && !self.streams()),
            MenuCommand::SaveAs => can_save && self.session.saved_to().is_some() && !saving,
            MenuCommand::SaveAnalysisOnly => self.can_save_analysis(),
            MenuCommand::NewSession => self.live.is_some(),
            MenuCommand::RemoveMasks => !self.masked.is_empty(),
            // The panes are the Bench's: dimmed in Watches.
            MenuCommand::Readings
            | MenuCommand::CompactReadings
            | MenuCommand::Pane(_)
            | MenuCommand::ResetPanes => self.view == View::Bench,
            _ => true,
        }
    }

    /// Whether `c`'s item is ticked: the view on show, and what is shown.
    pub(super) fn menu_checked(&self, c: MenuCommand) -> bool {
        match c {
            MenuCommand::View(v) => self.view == v,
            MenuCommand::Readings => self.show_readings,
            MenuCommand::CompactReadings => !self.readings_pinned,
            MenuCommand::Pane(p) => pane_visible(&self.panes.tiles, p),
            _ => false,
        }
    }

    /// The current label for items named by the app on both menu bars.
    pub(super) fn menu_name(&self, c: MenuCommand) -> Option<&'static str> {
        match c {
            MenuCommand::Save if self.marks_unsaved() => Some("Save"),
            MenuCommand::Save if self.session.saved_to().is_some() => Some("Save Again"),
            MenuCommand::Save => Some("Save…"),
            MenuCommand::CheckForUpdates => Some("Check for Updates…"),
            _ => None,
        }
    }

    /// Run `c`, from either menu bar or its shortcut, if it can be run
    /// now.
    pub(super) fn run_menu_command(&mut self, ctx: &egui::Context, c: MenuCommand) {
        if !self.menu_enabled(c) {
            return;
        }
        match c {
            MenuCommand::OpenRecording => self.open_from_menu(false),
            MenuCommand::OpenFolder => self.open_from_menu(true),
            MenuCommand::Save if self.marks_unsaved() => self.save_marks(),
            MenuCommand::Save => self.save_again(),
            MenuCommand::SaveAs => self.save_recording(),
            MenuCommand::SaveAnalysisOnly => self.save_analysis_only(),
            // Anything shown, from the microphone or a recording, can be
            // cleared, listening or not.
            MenuCommand::NewSession if self.needs_save_confirmation() => {
                self.confirm = Some(Confirm::NewSession);
            }
            MenuCommand::NewSession => self.new_session(),
            MenuCommand::Quit => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            MenuCommand::RemoveMasks => self.remove_masks_asked = true,
            MenuCommand::Settings => self.open_settings(self.settings_last),
            MenuCommand::View(v) => self.view = v,
            MenuCommand::Readings => self.show_readings = !self.show_readings,
            MenuCommand::CompactReadings => self.readings_pinned = !self.readings_pinned,
            MenuCommand::Pane(p) => {
                let on = pane_visible(&self.panes.tiles, p);
                self.show_pane(p, !on);
            }
            MenuCommand::ResetPanes => {
                self.show_readings = true;
                self.panes = default_layout(self.strip.horizontal);
                self.panes_dragged.clear();
            }
            MenuCommand::NewWatch => self.open_new_watch_form(),
            MenuCommand::FindInWatches => self.find_in_watches(),
            MenuCommand::UserGuide => ctx.open_url(egui::OpenUrl::new_tab(crate::guide::url())),
            MenuCommand::CheckForUpdates => {
                self.updates.start(ctx, true);
                self.update_dialog = Some(dialogs::UpdateDialog::Checking);
            }
            MenuCommand::ReportProblem => self.open_report(),
        }
    }

    /// An item of the window's menus that runs `c`: its name (unless it
    /// names itself), its shortcut, its hover, and `off`, why it is
    /// dimmed.
    fn menu_item(&mut self, ui: &mut egui::Ui, c: MenuCommand, name: &str, hint: &str, off: &str) {
        let mut button = egui::Button::new(self.menu_name(c).unwrap_or(name));
        if let Some(k) = menu_shortcut(c) {
            button = button.shortcut_text(ui.ctx().format_shortcut(&k));
        }
        let mut r = ui.add_enabled(self.menu_enabled(c), button);
        if !hint.is_empty() {
            r = r.on_hover_text(hint);
        }
        if !off.is_empty() {
            r = r.on_disabled_hover_text(off);
        }
        if r.clicked() {
            ui.close();
            self.run_menu_command(ui.ctx(), c);
        }
    }

    /// The Watches menu: New Watch… and Find in Watches…. Going to the
    /// Watches view is View ▸ Watches (Cmd+2); No Watch (Quick Listen) is
    /// on the Calibre card's Watch line.
    pub(super) fn watches_menu(&mut self, ui: &mut egui::Ui) {
        self.menu_item(
            ui,
            MenuCommand::NewWatch,
            "New Watch…",
            "Add a watch: its name, calibre, lift angle and beat rate",
            "",
        );
        self.menu_item(
            ui,
            MenuCommand::FindInWatches,
            "Find in Watches…",
            "Show the Watches view with its search field ready to type in",
            "",
        );
    }

    /// The Watches view's list, its search field ready to type in.
    pub(super) fn find_in_watches(&mut self) {
        self.view = View::Watches;
        if !self.list_shown() {
            self.list_for = Some((self.watch_open.clone(), self.unsorted_open));
        }
        self.watch_search_focus = true;
    }

    fn file_menu(&mut self, ui: &mut egui::Ui) {
        self.menu_item(
            ui,
            MenuCommand::OpenRecording,
            "Open Recording…",
            "Choose a WAV or FLAC recording and analyse it all. From outside your watches, it \
             asks first whether to add it to one of them.",
            "",
        );
        self.menu_item(ui, MenuCommand::OpenFolder, "Open Folder…", FOLDER_RULE, "");
        ui.separator();
        self.file_menu_save(ui);
        self.menu_item(
            ui,
            MenuCommand::NewSession,
            "New Session",
            NEW_SESSION_HINT,
            "Nothing to clear yet: listen to a watch or open a recording first",
        );
        ui.separator();
        self.menu_item(ui, MenuCommand::Quit, "Quit", "", "");
    }

    /// The File menu's Save, Save Again, Save As and Save Analysis Only
    /// items.
    fn file_menu_save(&mut self, ui: &mut egui::Ui) {
        let can_save = self.session.allows(Action::Save);
        let saving = self.saving_asked();
        let nothing = "Nothing to save yet: listen to a watch first";
        let recording_in = format!(
            "Saving to {} as it records: nothing to save again",
            self.kept_place()
        );
        let save_hint = if self.marks_unsaved() {
            SAVE_MARKS_HINT
        } else {
            SAVE_HINT
        };
        let off = if saving {
            SAVING_HINT
        } else if self.streams() {
            &recording_in
        } else {
            nothing
        };
        self.menu_item(ui, MenuCommand::Save, "Save…", save_hint, off);
        let off = if saving {
            SAVING_HINT
        } else {
            "Save once first; Save… asks where"
        };
        self.menu_item(ui, MenuCommand::SaveAs, "Save As…", SAVE_AS_HINT, off);
        let off = if self.analysis_save.is_some() {
            "The analysis is being kept now"
        } else if can_save {
            "Stop listening first: the whole take's sound is read again"
        } else {
            nothing
        };
        self.menu_item(
            ui,
            MenuCommand::SaveAnalysisOnly,
            "Save Analysis Only…",
            ANALYSIS_ONLY_HINT,
            off,
        );
    }

    fn edit_menu(&mut self, ui: &mut egui::Ui) {
        self.menu_item(
            ui,
            MenuCommand::RemoveMasks,
            "Remove All Masks…",
            "Bring every masked stretch back into the readings, after asking",
            "Nothing is masked",
        );
        ui.separator();
        self.menu_item(
            ui,
            MenuCommand::Settings,
            "Settings…",
            "How the app looks, the calibres and each input's kept clock, in a window of their \
             own",
            "",
        );
    }

    fn view_menu(&mut self, ui: &mut egui::Ui) {
        for (view, name, hint) in VIEWS {
            let c = MenuCommand::View(view);
            let shortcut = menu_shortcut(c).map(|k| ui.ctx().format_shortcut(&k));
            if ui
                .add(
                    egui::Button::selectable(self.menu_checked(c), name)
                        .shortcut_text(shortcut.unwrap_or_default()),
                )
                .on_hover_text(hint)
                .clicked()
            {
                ui.close();
                self.run_menu_command(ui.ctx(), c);
            }
        }
        ui.separator();
        // The panes are the Bench's: dimmed in Watches.
        let bench = self.menu_enabled(MenuCommand::ResetPanes);
        ui.add_enabled_ui(bench, |ui| self.view_menu_panes(ui));
    }

    fn view_menu_panes(&mut self, ui: &mut egui::Ui) {
        let off = "The panes are on the Bench: View > Bench first";
        let item = |app: &mut Self, ui: &mut egui::Ui, c: MenuCommand, name: &str, hint| {
            let mut on = app.menu_checked(c);
            if ui
                .checkbox(&mut on, name)
                .on_hover_text(hint)
                .on_disabled_hover_text(off)
                .changed()
            {
                app.run_menu_command(ui.ctx(), c);
            }
        };
        item(
            self,
            ui,
            MenuCommand::Readings,
            "Readings",
            "The rate, amplitude and beat error cards",
        );
        item(
            self,
            ui,
            MenuCommand::CompactReadings,
            "Compact Readings",
            "The readings as a strip across the top, their cards opening over the panes on a \
             click; off, the cards are pinned open in the layout",
        );
        // In the icon bar's order: its panes, Analysis, then Tests.
        for pane in Pane::CORE {
            item(self, ui, MenuCommand::Pane(pane), pane.title(), pane.hint());
        }
        ui.menu_button("Analysis", |ui| {
            for pane in Pane::ANALYSIS {
                item(self, ui, MenuCommand::Pane(pane), pane.title(), pane.hint());
            }
        });
        ui.separator();
        for pane in Pane::TESTS {
            item(self, ui, MenuCommand::Pane(pane), pane.title(), pane.hint());
        }
        ui.separator();
        self.menu_item(
            ui,
            MenuCommand::ResetPanes,
            "Reset Panes",
            "Put the panes back as the app starts: the readings, the Paper Strip and the Tick \
             Tock Profile, every other pane hidden",
            off,
        );
    }

    fn help_menu(&mut self, ui: &mut egui::Ui) {
        self.menu_item(
            ui,
            MenuCommand::UserGuide,
            "User Guide",
            "Open the manual in your browser: every button, pane and reading, what it does and \
             when to use it. It comes with the app, so it opens without an internet connection.",
            "",
        );
        ui.separator();
        self.menu_item(
            ui,
            MenuCommand::CheckForUpdates,
            "Check for Updates…",
            "Ask GitHub whether a newer Timegrapher is published. Nothing downloads or installs \
             itself.",
            "",
        );
        ui.separator();
        self.menu_item(
            ui,
            MenuCommand::ReportProblem,
            "Report a Problem…",
            "Make a zip of what went wrong, with up to five minutes of the sound, to attach to a \
             GitHub issue. The app sends nothing itself.",
            "",
        );
        if cfg!(target_os = "linux") {
            ui.separator();
            if ui
                .button("Add to Applications Menu")
                .on_hover_text(
                    "Add Timegrapher to this computer's applications menu, with its icon, \
                     so the desktop shows the icon on the window and in the task bar. It \
                     writes one entry and one icon in your home folder.",
                )
                .clicked()
            {
                ui.close();
                match crate::desktop::install() {
                    Ok(p) => self.info_msg(format!(
                        "Added to the applications menu ({}). The icon shows from the next \
                         start.",
                        p.display()
                    )),
                    Err(e) => self.error(format!("Couldn't add it to the menu: {e}")),
                }
            }
        }
    }

    /// The question Open asks for `path`, when there are watches to add it
    /// to and it isn't in them already. None means open it straight away
    /// (a recording that can't be read says why when opened).
    pub(super) fn open_ask_for(&self, path: &Path) -> Option<OpenAsk> {
        let Ok(Some(l)) = &self.library else {
            return None;
        };
        let canon = |p: &Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
        if l.read_only.is_some() || canon(path).starts_with(canon(&l.root)) {
            return None;
        }
        let inner = take_folder(path).ok()?;
        // A folder's session file speaks for all of it, each recording
        // listed or not; a lone file's only through an entry listing it.
        let manifest = if inner.is_dir() && inner.join(MANIFEST).is_file() {
            let text = std::fs::read_to_string(inner.join(MANIFEST)).ok()?;
            let m: Manifest = toml::from_str(&text).ok()?;
            let entries = m.recordings.clone();
            Some((m, entries))
        } else {
            read_manifest(&inner).ok()?
        };
        let files = recordings_in(&inner).unwrap_or_default();
        let named =
            position_in_name(&inner).or_else(|| files.first().and_then(|f| position_in_name(f)));
        let dir = if inner.is_dir() {
            inner.clone()
        } else {
            inner.parent()?.to_path_buf()
        };
        // A lone file's folder speaks for it only through a session file
        // listing it.
        let started = (manifest.is_some() || inner.is_dir()) && started_in(&dir).is_some();
        let has = match &manifest {
            Some((m, entries)) => OpenHas {
                manifest: true,
                bph: m.bph,
                lift: m.lift,
                calibre: m.calibre.clone().filter(|c| !c.is_empty()),
                position: named.is_some() || entries.iter().all(|e| e.position.is_some()),
                started,
            },
            None => OpenHas {
                position: named.is_some(),
                started,
                ..Default::default()
            },
        };
        let watch = self
            .watch_open
            .clone()
            .filter(|id| l.watches.iter().any(|e| &e.watch.id == id));
        Some(OpenAsk {
            path: path.to_path_buf(),
            add: watch.is_some(),
            new_watch: l.watches.is_empty(),
            watch,
            new_name: String::new(),
            has,
            bph: None,
            lift: DEFAULT_LIFT_DEG,
            lift_set: false,
            position: named,
            started: started_guess(&files)
                .map(|t| t.format("%Y-%m-%d %H:%M").to_string())
                .unwrap_or_default(),
            error: None,
            keeping: false,
        })
    }

    /// What adding `a` to its watch asks to be filled in: what neither the
    /// recording's files nor the watch and its calibre say.
    pub(super) fn open_need(&self, a: &OpenAsk) -> OpenNeed {
        let w = match (&self.library, &a.watch) {
            (Ok(Some(l)), Some(id)) if !a.new_watch => l
                .watches
                .iter()
                .find(|e| &e.watch.id == id)
                .map(|e| &e.watch),
            _ => None,
        };
        let calibre = a
            .has
            .calibre
            .clone()
            .or_else(|| w.and_then(|w| w.calibre.clone()))
            .filter(|c| !c.is_empty());
        let of_calibre = |f: &dyn Fn(&str) -> bool| calibre.as_deref().is_some_and(f);
        OpenNeed {
            bph: a.has.bph.is_none()
                && w.and_then(|w| w.bph).is_none()
                && !of_calibre(&|c| self.cal.bph(c).is_some()),
            lift: a.has.lift.is_none()
                && w.and_then(|w| w.lift).is_none()
                && !of_calibre(&|c| self.cal.lift(c).is_some()),
            position: !a.has.position,
            started: !a.has.started,
        }
    }

    /// Of what `need` says is missing, what the window asks for: all of
    /// it, except, when keeping the recording on show, what the take on
    /// show already says. It is still written with the take.
    pub(super) fn open_asks(a: &OpenAsk, need: OpenNeed) -> OpenNeed {
        if !a.keeping {
            return need;
        }
        OpenNeed {
            bph: need.bph && a.bph.is_none(),
            lift: need.lift && !a.lift_set,
            position: need.position && a.position.is_none(),
            started: need.started,
        }
    }
}

#[cfg(test)]
#[path = "../app_menus_tests.rs"]
mod tests;
