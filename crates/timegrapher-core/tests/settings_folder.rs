//! Where the user's settings and their store of clock corrections live,
//! and the store written there read back. One test, because it sets
//! `TIMEGRAPHER_CONFIG_DIR` (and `XDG_CONFIG_HOME`) for the whole process.

use std::fs;
use timegrapher_core::clockstore::{self, ClockStore, DeviceClock};

#[test]
fn the_settings_folder_holds_the_store_of_corrections() {
    let root = std::env::temp_dir().join(format!("tg-settings-folder-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let config = root.join("config");
    std::env::set_var("TIMEGRAPHER_CONFIG_DIR", &config);
    assert_eq!(clockstore::config_dir(), Some(config.clone()));
    let file = config.join("clocks.json");
    assert_eq!(clockstore::path(), Some(file.clone()));

    // No store yet is an empty one; a saved one reads back.
    assert!(ClockStore::load().unwrap().get("USB Mic").is_none());
    let mut store = ClockStore::default();
    store.set(DeviceClock {
        device: "USB Mic".into(),
        ppm: 12.5,
        ppm_sd: None,
        span_s: 600.0,
        measured_utc: "2026-10-10T03:00:00Z".into(),
        source: "measure".into(),
    });
    assert_eq!(store.save().unwrap(), file);
    assert!(file.is_file());
    let read = ClockStore::load().unwrap();
    assert_eq!(read.get("USB Mic").map(|d| d.ppm), Some(12.5));

    // Set but empty, it is not set: on Linux the folder is then the
    // desktop's settings folder.
    if cfg!(target_os = "linux") {
        let xdg = root.join("xdg");
        std::env::set_var("TIMEGRAPHER_CONFIG_DIR", "");
        std::env::set_var("XDG_CONFIG_HOME", &xdg);
        assert_eq!(clockstore::config_dir(), Some(xdg.join("timegrapher")));
    }
    let _ = fs::remove_dir_all(&root);
}
