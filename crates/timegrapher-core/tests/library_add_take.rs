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
        // Short enough for the app: the take by its name, not its whole
        // path, and a whole sentence.
        assert!(e.starts_with("\"source\" names "), "{e}");
        assert!(e.ends_with("add the take again."), "{e}");
        assert!(!e.contains(&*root.to_string_lossy()), "{e}");
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

/// File Under a Watch ([`library::Library::move_take`]) refuses an
/// Unsorted take that names its sound or clock log by a path leading out
/// of its folder, or from the root into it, as Add to Watch does, and
/// leaves it in Unsorted as readable as before (#882).
#[test]
fn filing_an_unsorted_take_with_files_outside_its_folder_is_refused() {
    let r = fs::canonicalize(root("move-outside")).unwrap();
    let (mut lib, id) = library_at(&r);
    let unsorted = lib.unsorted_dir().unwrap();
    fs::create_dir_all(&unsorted).unwrap();
    let take = unsorted.join("2026-11-03 09.00 Dial Up");
    fs::create_dir_all(&take).unwrap();
    wav(&unsorted.join("audio.wav"));
    wav(&take.join("audio.wav"));
    clock_log(&unsorted.join("clock.csv"));
    let from_root = take.join("audio.wav").to_string_lossy().replace('\\', "/");
    for (file, clock) in [
        ("../audio.wav", None),
        ("audio.wav", Some("../clock.csv")),
        (from_root.as_str(), None),
    ] {
        manifest(&take, file, clock);
        lib.scan();
        let before = fresh(&take).unwrap();
        let e = lib.move_take(&take, &id).unwrap_err();
        assert!(
            e.contains("won't lead to the file"),
            "{file} {clock:?}: {e}"
        );
        assert!(
            e.starts_with("\"2026-11-03 09.00 Dial Up\" names ") && e.ends_with("move it again."),
            "{e}"
        );
        assert!(!e.contains(&*r.to_string_lossy()), "{e}");
        assert!(lib.is_unsorted(&take), "{file} {clock:?}");
        assert_eq!(fresh(&take), Ok(before), "{file} {clock:?}");
    }
    let takes = r.join("Watches").join("Bench").join("takes");
    let left: Vec<_> = fs::read_dir(&takes).into_iter().flatten().collect();
    assert!(left.is_empty(), "{left:?}");
    // A take whose files are all inside its folder is filed, and reads
    // as before.
    manifest(&take, "audio.wav", None);
    lib.scan();
    let before = fresh(&take).unwrap();
    let filed = lib.move_take(&take, &id).unwrap();
    assert_eq!(fresh(&filed), Ok(before));
    let _ = fs::remove_dir_all(&r);
}
