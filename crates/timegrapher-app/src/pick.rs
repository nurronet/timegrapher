//! File dialogs that don't hold up listening.
//!
//! A frame holds the app while it draws, and the listening thread
//! ([`crate::pump`]) needs the app to take the sound in. A file dialog
//! waits on the user, so one shown from inside a frame held listening up
//! for as long as it stayed open, and the sound piled up in memory
//! meanwhile. A frame now only asks for a dialog
//! ([`crate::app::TimegrapherApp::pick`]); the window shows it once the
//! frame has let the app go, and hands back what was picked ([`answer`]).

use crate::app::TimegrapherApp;
use std::path::PathBuf;
use std::sync::Mutex;

/// What a dialog picks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pick {
    File,
    Files,
    Folder,
    Save,
}

/// What is done with what was picked, with the app held again.
type Then = Box<dyn FnOnce(&mut TimegrapherApp, Vec<PathBuf>) + Send>;

/// A file dialog to show once the frame lets the app go, and what to do
/// with its answer.
pub struct Picker {
    dialog: rfd::FileDialog,
    pick: Pick,
    then: Then,
}

impl Picker {
    /// Pick one file, folder or place to save. `then` runs only when one
    /// was picked.
    pub fn one(
        dialog: rfd::FileDialog,
        pick: Pick,
        then: impl FnOnce(&mut TimegrapherApp, PathBuf) + Send + 'static,
    ) -> Self {
        let then = move |app: &mut TimegrapherApp, mut picked: Vec<PathBuf>| {
            if let Some(p) = picked.pop() {
                then(app, p);
            }
        };
        Picker {
            dialog,
            pick,
            then: Box::new(then),
        }
    }

    /// Pick files. `then` runs only when some were picked.
    pub fn files(
        dialog: rfd::FileDialog,
        then: impl FnOnce(&mut TimegrapherApp, Vec<PathBuf>) + Send + 'static,
    ) -> Self {
        let then = move |app: &mut TimegrapherApp, picked: Vec<PathBuf>| {
            if !picked.is_empty() {
                then(app, picked);
            }
        };
        Picker {
            dialog,
            pick: Pick::Files,
            then: Box::new(then),
        }
    }
}

/// Show the dialog the app asked for, if it asked, with the app let go
/// while it is open so listening carries on; then hand what was picked
/// back to the app. `show` puts the dialog up: [`show`] in the window, a
/// stand-in in tests. Whether there was one.
pub fn answer(
    app: &Mutex<TimegrapherApp>,
    show: impl FnOnce(rfd::FileDialog, Pick) -> Vec<PathBuf>,
) -> bool {
    let Some(Picker { dialog, pick, then }) = crate::pump::lock(app).take_picker() else {
        return false;
    };
    let picked = show(dialog, pick);
    then(&mut crate::pump::lock(app), picked);
    true
}

/// Show `dialog` and wait on the user: what they picked, none if they
/// cancelled.
pub fn show(dialog: rfd::FileDialog, pick: Pick) -> Vec<PathBuf> {
    match pick {
        Pick::File => dialog.pick_file().into_iter().collect(),
        Pick::Files => dialog.pick_files().unwrap_or_default(),
        Pick::Folder => dialog.pick_folder().into_iter().collect(),
        Pick::Save => dialog.save_file().into_iter().collect(),
    }
}
