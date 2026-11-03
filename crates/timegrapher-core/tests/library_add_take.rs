//! Adding a take folder to a watch ([`library::Library::add_take`]) keeps
//! it readable, or refuses before anything is moved (audit R2-2).

use std::fs;
use std::path::{Path, PathBuf};
use timegrapher_core::{audio, library, session::Manifest, synth, take};

fn root(name: &str) -> PathBuf {
    let r = std::env::temp_dir().join(format!("tg-add-take-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&r);
    fs::create_dir_all(&r).unwrap();
    r
}

fn wav(p: &Path) {
    let cfg = synth::SynthConfig {
        duration_s: 45.0,
        rate_s_per_day: 10.0,
        ..Default::default()
    };
    audio::write_wav(p, &synth::generate(&cfg, |_| 270.0, |_| 0.0)).unwrap();
}

fn manifest(dir: &Path, file: &str, clock: Option<&str>) {
    let clock = clock
        .map(|c| format!("clock = \"{c}\"\n"))
        .unwrap_or_default();
    fs::write(
        dir.join(take::MANIFEST),
        format!(
            "bph = 28800\nlift = 52\nsettle_s = 0\n\n[[recording]]\nfile = \"{file}\"\n\
             position = \"DU\"\n{clock}"
        ),
    )
    .unwrap();
}

fn clock_log(p: &Path) {
    let mut text = String::from("unix_s,frames\n");
    for k in 0..=45 {
        text += &format!(
            "{:.6},{}\n",
            1_700_000_000.0 + k as f64 * 1.00005,
            k * 48_000
        );
    }
    fs::write(p, text).unwrap();
}

/// The rate of `dir`'s first recording, read as `session` reads it.
fn fresh(dir: &Path) -> Result<f64, String> {
    let text = fs::read_to_string(dir.join(take::MANIFEST)).map_err(|e| e.to_string())?;
    let m: Manifest = toml::from_str(&text).map_err(|e| e.to_string())?;
    let settings = take::EntrySettings {
        stream: take::stream_settings(&m, None, take::Given::default()),
        card_clock: false,
        settle_s: None,
    };
    take::measure_entry(
        dir,
        &m,
        &m.recordings[0],
        &settings,
        |_| Ok(None),
        |_| Ok(None),
    )
    .map_err(|e| e.to_string())?
    .measurement
    .rate_s_per_day
    .ok_or_else(|| "no rate".into())
}

fn library_at(root: &Path) -> (library::Library, String) {
    let mut lib = library::Library::create(&root.join("Watches")).unwrap();
    lib.add(&library::Watch::new("Bench")).unwrap();
    let id = lib.find("Bench").unwrap().watch.id.clone();
    (lib, id)
}

/// A take naming `file` and `clock` leading out of its folder is refused
/// by Copy and by Move, and is left where it was, as readable as before.
fn refused(name: &str, file: &str, clock: Option<&str>, setup: impl Fn(&Path, &Path)) {
    let root = root(name);
    let source = root.join("source");
    fs::create_dir_all(&source).unwrap();
    setup(&root, &source);
    manifest(&source, file, clock);
    let before = fresh(&source).unwrap();
    let (mut lib, id) = library_at(&root);
    for keep in [true, false] {
        let e = lib.add_take(&source, &id, keep).unwrap_err();
        assert!(e.contains("won't lead to the file"), "{e}");
        assert_eq!(fresh(&source), Ok(before), "keep {keep}");
    }
    let takes = root.join("Watches").join("Bench").join("takes");
    let left: Vec<_> = fs::read_dir(&takes).into_iter().flatten().collect();
    assert!(left.is_empty(), "{left:?}");
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_take_whose_sound_is_outside_its_folder_is_refused() {
    refused("audio", "../audio.wav", None, |root, _| {
        wav(&root.join("audio.wav"))
    });
}

#[test]
fn a_take_whose_clock_log_is_outside_its_folder_is_refused() {
    refused(
        "clock",
        "audio.wav",
        Some("../clock.csv"),
        |root, source| {
            wav(&source.join("audio.wav"));
            clock_log(&root.join("clock.csv"));
        },
    );
}

#[test]
fn a_self_contained_take_is_copied_and_moved() {
    let root = root("self-contained");
    let source = root.join("source");
    fs::create_dir_all(source.join("sub")).unwrap();
    wav(&source.join("sub").join("audio.wav"));
    clock_log(&source.join("clock.csv"));
    // A path that steps out and back in stays inside.
    manifest(&source, "sub/../sub/audio.wav", Some("./clock.csv"));
    let want = fresh(&source).unwrap();
    let (mut lib, id) = library_at(&root);
    for keep in [true, false] {
        let (to, warning) = lib.add_take(&source, &id, keep).unwrap();
        assert!(warning.is_none(), "{warning:?}");
        assert!((fresh(&to).unwrap() - want).abs() < 0.01);
        assert_eq!(source.exists(), keep);
    }
    let _ = fs::remove_dir_all(&root);
}

/// A path from the root into the take's own folder names where the file
/// was, not where it goes, so it is refused as well, for sound and clock
/// log alike (the outside review of #876).
#[test]
fn a_take_naming_its_own_files_from_the_root_is_refused() {
    let r = root("absolute-inside");
    let source = fs::canonicalize(&r).unwrap().join("source");
    fs::create_dir_all(&source).unwrap();
    wav(&source.join("audio.wav"));
    clock_log(&source.join("clock.csv"));
    let wav_at = source
        .join("audio.wav")
        .to_string_lossy()
        .replace('\\', "/");
    let clock_at = source
        .join("clock.csv")
        .to_string_lossy()
        .replace('\\', "/");
    let (mut lib, id) = library_at(&r);
    for (file, clock) in [
        (wav_at.as_str(), None),
        ("audio.wav", Some(clock_at.as_str())),
    ] {
        manifest(&source, file, clock);
        let before = fresh(&source).unwrap();
        for keep in [true, false] {
            let e = lib.add_take(&source, &id, keep).unwrap_err();
            assert!(e.contains("won't lead to the file"), "{e}");
            assert_eq!(fresh(&source), Ok(before), "keep {keep}");
        }
    }
    let _ = fs::remove_dir_all(&r);
}

/// A path from the root to a file elsewhere still leads there after a
/// move, so such a take is added and reads as before.
#[test]
fn a_take_naming_a_file_elsewhere_from_the_root_is_added() {
    let r = root("absolute-elsewhere");
    let source = r.join("source");
    fs::create_dir_all(&source).unwrap();
    let elsewhere = fs::canonicalize(&r).unwrap().join("audio.wav");
    wav(&elsewhere);
    manifest(
        &source,
        &elsewhere.to_string_lossy().replace('\\', "/"),
        None,
    );
    let before = fresh(&source).unwrap();
    let (mut lib, id) = library_at(&r);
    let (to, warning) = lib.add_take(&source, &id, false).unwrap();
    assert!(warning.is_none(), "{warning:?}");
    assert_eq!(fresh(&to), Ok(before));
    assert!(!source.exists());
    let _ = fs::remove_dir_all(&r);
}

/// Filing an Unsorted take must check the same paths as Add to Watch,
/// before moving either its sound or its clock log (#882).
#[test]
fn filing_a_take_with_paths_that_would_break_leaves_it_readable_in_unsorted() {
    let r = root("file-under-refused");
    let (mut lib, id) = library_at(&r);
    let unsorted = lib.unsorted_dir().unwrap();
    let source = unsorted.join("source");
    fs::create_dir_all(&source).unwrap();
    wav(&source.join("audio.wav"));
    clock_log(&source.join("clock.csv"));
    fs::copy(source.join("audio.wav"), unsorted.join("audio.wav")).unwrap();
    fs::copy(source.join("clock.csv"), unsorted.join("clock.csv")).unwrap();
    let absolute = |name| {
        fs::canonicalize(source.join(name))
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/")
    };
    let sound = absolute("audio.wav");
    let clock = absolute("clock.csv");
    for (file, clock) in [
        ("../audio.wav", None),
        ("audio.wav", Some("../clock.csv")),
        (sound.as_str(), None),
        ("audio.wav", Some(clock.as_str())),
    ] {
        manifest(&source, file, clock);
        lib.scan();
        let before = fresh(&source).unwrap();
        let error = lib.move_take(&source, &id).unwrap_err();
        assert!(error.contains("won't lead to the file"), "{error}");
        assert_eq!(fresh(&source), Ok(before));
        assert!(lib.is_unsorted(&source));
        assert_eq!(lib.unsorted.takes.len(), 1);
        let destination = lib.find("Bench").unwrap().dir.join("takes");
        assert!(!destination.join("source").exists());
    }
    let _ = fs::remove_dir_all(r);
}

#[test]
fn filing_a_take_keeps_internal_relative_and_external_absolute_paths_readable() {
    let r = root("file-under-readable");
    let (mut lib, id) = library_at(&r);
    let source = lib.unsorted_dir().unwrap().join("source");
    fs::create_dir_all(&source).unwrap();
    wav(&source.join("audio.wav"));
    clock_log(&source.join("clock.csv"));
    manifest(&source, "audio.wav", Some("clock.csv"));
    let before = fresh(&source).unwrap();
    let destination = lib.move_take(&source, &id).unwrap();
    assert_eq!(fresh(&destination), Ok(before));
    assert!(!source.exists());
    // An unchanged destination is a no-op even with an absolute path
    // to one of its own files: no reference would be invalidated.
    let sound = fs::canonicalize(destination.join("audio.wav"))
        .unwrap()
        .to_string_lossy()
        .replace('\\', "/");
    manifest(&destination, &sound, Some("clock.csv"));
    assert_eq!(lib.move_take(&destination, &id), Ok(destination.clone()));
    fs::create_dir_all(&source).unwrap();
    manifest(&source, &sound, None);
    let before = fresh(&source).unwrap();
    let filed = lib.move_take(&source, &id).unwrap();
    assert_eq!(fresh(&filed), Ok(before));
    assert!(!source.exists());
    let _ = fs::remove_dir_all(r);
}
