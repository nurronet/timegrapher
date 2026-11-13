//! The file dialogs the app asks for. Each is shown once the frame has let
//! the app go, so listening carries on while it is open ([`crate::pick`]).

use super::*;

impl TimegrapherApp {
    /// Ask for `picker`'s dialog, shown once this frame ends. While one is
    /// waiting to be shown, another is not asked for.
    pub(crate) fn pick(&mut self, picker: Picker) {
        if self.picker.is_none() {
            self.picker = Some(picker);
        }
    }

    /// The dialog asked for, to show now.
    pub(crate) fn take_picker(&mut self) -> Option<Picker> {
        self.picker.take()
    }

    /// Whether a dialog is waiting to be shown.
    #[cfg(test)]
    pub(super) fn picking(&self) -> bool {
        self.picker.is_some()
    }

    /// Where a save or load dialog starts: the folder the session was
    /// saved in, else the save folder.
    pub(super) fn save_start(&self) -> PathBuf {
        self.session
            .saved_to()
            .and_then(|p| p.parent().map(Path::to_path_buf))
            .unwrap_or_else(|| PathBuf::from(&self.save_dir))
    }

    /// Open a recording, or a folder of segments, chosen in a dialog.
    pub(super) fn open_from_menu(&mut self, folder: bool) {
        let here = Path::new(self.file_path.trim());
        let picker = if folder {
            let mut d =
                rfd::FileDialog::new().set_title("Open a Saved Take or a Folder of Segments");
            let start = if here.is_dir() {
                Some(here)
            } else {
                here.parent()
            };
            if let Some(dir) = start.filter(|d| d.is_dir()) {
                d = d.set_directory(dir);
            }
            Picker::one(d, Pick::Folder, Self::open_picked)
        } else {
            let mut d = rfd::FileDialog::new()
                .set_title("Open a Recording")
                .add_filter("Recordings", &["wav", "WAV", "flac", "FLAC"])
                .add_filter("All files", &["*"]);
            if let Some(dir) = here.parent().filter(|d| d.is_dir()) {
                d = d.set_directory(dir);
            }
            Picker::one(d, Pick::File, Self::open_picked)
        };
        self.pick(picker);
    }

    /// Open the recording or folder picked in Open's dialog, or ask first.
    fn open_picked(&mut self, p: PathBuf) {
        self.file_path = p.display().to_string();
        self.input = Input::File;
        match self.open_ask_for(&p) {
            Some(a) => self.open_ask = Some(a),
            None => self.open_recording(&p, true),
        }
    }

    /// Load the full-wind test's saved results from a file the user picks,
    /// for the next test to be set against.
    pub(super) fn load_earlier(&mut self) {
        let mut d = rfd::FileDialog::new()
            .set_title("Load the Full-Wind Positions Test's Results")
            .add_filter("Positions Test Results", &["json"]);
        let start = self.save_start();
        if start.is_dir() {
            d = d.set_directory(&start);
        }
        self.pick(Picker::one(d, Pick::File, |a, path| {
            if let Some(full) = a.read_full_wind(&path) {
                a.guided_earlier = Some(full);
                a.guided_earlier_file = true;
            }
        }));
    }

    /// Read the full-wind test's results saved at `path`; None if they
    /// can't be read, which says so.
    pub(super) fn read_full_wind(&mut self, path: &Path) -> Option<guided::FullWind> {
        let read = std::fs::read_to_string(path)
            .map_err(|e| e.to_string())
            .and_then(|t| guided::FullWind::from_json(&t))
            .and_then(|f| match f.wind_h {
                Some(_) => Err(
                    "this file holds a test run 24 hours after winding. Load the full-wind \
                     test's results instead."
                        .into(),
                ),
                None => Ok(f),
            });
        match read {
            Ok(f) => Some(f),
            Err(e) => {
                self.error(format!("Couldn't load {}: {e}", path.display()));
                None
            }
        }
    }

    /// Save `text` to a file picked in `dialog`, saying what happened.
    pub(super) fn save_text_as(
        &mut self,
        dialog: rfd::FileDialog,
        what: &'static str,
        text: String,
    ) {
        self.pick(Picker::one(dialog, Pick::Save, move |a, path| {
            a.info_msg(match std::fs::write(&path, text) {
                Ok(()) => format!("Saved {what} to {}", path.display()),
                Err(e) => format!("Couldn't save {what} to {}: {e}", path.display()),
            });
        }));
    }

    /// Save Report… in Report a Problem: ask where, then save it there.
    pub(super) fn report_save_asked(&mut self) {
        if let Some(d) = self.report.save_asked() {
            self.pick(Picker::one(d, Pick::Save, |a, zip| a.report.save_to(zip)));
        }
    }
}
