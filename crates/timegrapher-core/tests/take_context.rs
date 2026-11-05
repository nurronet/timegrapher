//! The take context's windows are `session`'s, laid end to end (#294).
//!
//! Each session entry's window in the context, less its recording's
//! offset, is the window `session` reads that entry in, to the bit; the
//! recordings sit end to end in the order the take reads them; and each
//! entry's masks are moved by its recording's offset. Synthetic takes
//! run everywhere; the corpus's takes of several recordings run when
//! `TIMEGRAPHER_CORPUS` names the corpus.

use std::path::{Path, PathBuf};
use timegrapher_core::session::Manifest;
use timegrapher_core::stream::StreamConfig;
use timegrapher_core::take::{self, EntrySettings};
use timegrapher_core::take_context::{
    entry_layout, in_files, EntryWindow, FilePart, Masks, TakeContext,
};

fn settings(settle_s: Option<f64>) -> EntrySettings {
    EntrySettings {
        stream: StreamConfig::default(),
        card_clock: false,
        settle_s,
    }
}

fn manifest(base: &Path) -> Manifest {
    toml::from_str(&std::fs::read_to_string(base.join("session.toml")).unwrap()).unwrap()
}

/// Every entry of the take in `base`, read as `session` reads it, against
/// the context's window for it less its offset.
fn same_as_session(base: &Path, m: &Manifest, entries: &[EntryWindow], given: Option<f64>) {
    assert_eq!(entries.len(), m.recordings.len());
    for (e, w) in m.recordings.iter().zip(entries) {
        let read = take::measure_entry(base, m, e, &settings(given), |_| Ok(None), |_| Ok(None));
        match (read, &w.window) {
            (Ok(r), Ok(c)) => {
                let back = |t: f64| t - w.offset_s;
                assert_eq!(
                    (r.start_s, r.stop_s, r.settle_s, r.end_s),
                    (
                        back(c.start_s),
                        back(c.stop_s),
                        back(c.from_s),
                        back(c.to_s)
                    ),
                    "{} entry {}",
                    base.display(),
                    w.entry
                );
            }
            (Err(take::EntryError::NoReading(a)), Err(b)) => assert_eq!(&a, b),
            (r, c) => panic!(
                "{} entry {}: session {:?}, context {c:?}",
                base.display(),
                w.entry,
                r.map(|r| (r.settle_s, r.end_s)).map_err(|e| e.to_string())
            ),
        }
    }
}

/// The recordings lie end to end, each where the one before it ends.
fn end_to_end(files: &[FilePart]) {
    let mut at = 0.0;
    for f in files {
        assert!(
            (f.offset_s - at).abs() < 1e-9,
            "{}: {} not {at}",
            f.path.display(),
            f.offset_s
        );
        at = f.offset_s + f.duration_s;
    }
}

fn synthetic_take(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tg-take-context-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    for (i, name) in ["a.wav", "b.wav", "c.wav"].iter().enumerate() {
        let sound = timegrapher_core::synth::generate(
            &timegrapher_core::synth::SynthConfig {
                duration_s: 60.0 + i as f64,
                rate_s_per_day: 5.0,
                seed: i as u64 + 1,
                ..Default::default()
            },
            |_| 270.0,
            |_| 0.0,
        );
        timegrapher_core::audio::write_wav(&dir.join(name), &sound).unwrap();
    }
    std::fs::write(
        dir.join("session.toml"),
        "bph = 28800\nmeasure_s = 30\n\
         [[recording]]\nfile = \"a.wav\"\nposition = \"DU\"\n\
         [[recording]]\nfile = \"./b.wav\"\nposition = \"DD\"\nsettle_s = 10\nmeasure_s = 20\n\
         [[recording]]\nfile = \"c.wav\"\nposition = \"CU\"\nstart_s = 5\nend_s = 50\n\
         masked = [[20.0, 25.0]]\n\
         [[recording]]\nfile = \"c.wav\"\nposition = \"CD\"\nstart_s = 50\nend_s = 50\n\
         [[recording]]\nfile = \".\"\nposition = \"CL\"\nsettle_s = 30\n",
    )
    .unwrap();
    dir
}

#[test]
fn a_takes_entries_are_sessions_windows_laid_end_to_end() {
    let dir = synthetic_take("windows");
    let m = manifest(&dir);
    for given in [None, Some(15.0)] {
        let (ctx, log) = TakeContext::open(
            &dir,
            &m,
            settings(given),
            Masks::Cut,
            |_| Ok(None),
            |_| Ok(None),
            |_, _| {},
        )
        .unwrap();
        let lengths: Vec<f64> = ctx.files.iter().map(|f| f.duration_s).collect();
        assert_eq!(lengths, [60.0, 61.0, 62.0]);
        end_to_end(&ctx.files);
        assert!((log.duration_s - 183.0).abs() < 1e-9, "{}", log.duration_s);
        let offsets: Vec<f64> = ctx.entries.iter().map(|w| w.offset_s).collect();
        assert_eq!(offsets, [0.0, 60.0, 121.0, 121.0, 0.0]);
        // b.wav, spelt "./b.wav", is the second file; "." is all three.
        let parts: Vec<_> = ctx.entries.iter().map(|w| w.parts.clone()).collect();
        assert_eq!(parts, [0..1, 1..2, 2..3, 2..3, 0..3]);
        same_as_session(&dir, &m, &ctx.entries, given);
        // The settling-only entry reads nothing, as `session` notes it.
        assert_eq!(ctx.entries[3].window, Err(take::NoReading::Nothing));
        // c.wav's mask, at 20 to 25 s into it, is at 141 to 146 s in the
        // take, and no beat is kept inside it.
        assert_eq!(ctx.masked, [(141.0, 146.0)]);
        assert!(!log.beats.iter().any(|b| b.time > 141.0 && b.time < 146.0));
        // The headers alone give the same layout.
        let (files, entries) = entry_layout(&dir, &m, given).unwrap();
        assert_eq!((files, entries), (ctx.files.clone(), ctx.entries.clone()));
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_entry_outside_the_takes_recordings_is_refused() {
    let dir = synthetic_take("outside");
    let mut m = manifest(&dir);
    m.recordings[0].file = "elsewhere.wav".into();
    let e = entry_layout(&dir, &m, None).unwrap_err();
    assert!(e.contains("elsewhere.wav"), "{e}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// The corpus's takes of several recordings: laid end to end, every
/// window inside its own recording, and on the shorter ones each window
/// `session`'s to the bit.
#[test]
fn the_corpus_takes_of_several_recordings_lie_end_to_end() {
    let Some(corpus) = std::env::var_os("TIMEGRAPHER_CORPUS").map(PathBuf::from) else {
        eprintln!("SKIP: set TIMEGRAPHER_CORPUS to lay out the corpus's takes");
        return;
    };
    let mut seen = 0;
    for take_dir in std::fs::read_dir(corpus.join("takes")).unwrap() {
        let base = take_dir.unwrap().path();
        let Ok(text) = std::fs::read_to_string(base.join("session.toml")) else {
            continue;
        };
        let m: Manifest = toml::from_str(&text).unwrap();
        let Ok(sounds) = take::sound_files(std::slice::from_ref(&base)) else {
            continue;
        };
        if sounds.len() < 2 || m.recordings.is_empty() {
            continue;
        }
        let (files, entries) =
            entry_layout(&base, &m, None).unwrap_or_else(|e| panic!("{}: {e}", base.display()));
        end_to_end(&files);
        for w in &entries {
            let part = files
                .iter()
                .find(|f| (f.offset_s - w.offset_s).abs() < 1e-9)
                .unwrap();
            if let Ok(win) = &w.window {
                let end = if m.recordings[w.entry].file == "." {
                    files.last().map_or(0.0, |f| f.offset_s + f.duration_s)
                } else {
                    part.offset_s + part.duration_s
                };
                assert!(
                    win.start_s >= part.offset_s && win.stop_s <= end + 1e-9,
                    "{}",
                    base.display()
                );
            }
        }
        let name = base.file_name().unwrap().to_string_lossy().into_owned();
        if ["ym42_DD_30min", "eta2824_bare_DU_30min"].contains(&name.as_str()) {
            same_as_session(&base, &m, &entries, None);
        }
        seen += 1;
    }
    assert!(seen >= 5, "{seen} takes of several recordings");
}

#[test]
fn masks_kept_as_data_are_not_cut_from_the_beats() {
    let dir = synthetic_take("kept");
    let m = manifest(&dir);
    let open = |masks| {
        TakeContext::open(
            &dir,
            &m,
            settings(None),
            masks,
            |_| Ok(None),
            |_| Ok(None),
            |_, _| {},
        )
        .unwrap()
    };
    let (cut, cut_log) = open(Masks::Cut);
    let (kept, kept_log) = open(Masks::Kept);
    assert_eq!(kept.masked, cut.masked);
    assert_eq!(kept.entries, cut.entries);
    let inside = |log: &timegrapher_core::stream::BeatLog| {
        log.beats
            .iter()
            .filter(|b| b.time > 141.0 && b.time < 146.0)
            .count()
    };
    assert_eq!(inside(&cut_log), 0);
    assert!(inside(&kept_log) > 30, "{}", inside(&kept_log));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_stretch_of_the_take_is_split_into_its_files_own_seconds() {
    let dir = synthetic_take("split");
    let (files, _) = entry_layout(&dir, &manifest(&dir), None).unwrap();
    // Files at 0 to 60, 60 to 121 and 121 to 183 s.
    assert_eq!(in_files(&files, 10.0, 20.0), [(0, 10.0, 20.0)]);
    assert_eq!(
        in_files(&files, 50.0, 130.0),
        [(0, 50.0, 60.0), (1, 0.0, 61.0), (2, 0.0, 9.0)]
    );
    assert_eq!(in_files(&files, 183.0, 190.0), []);
    let _ = std::fs::remove_dir_all(&dir);
}

/// The positions the context lists are `take`'s listed positions of each
/// entry in its own recording, moved by that recording's offset.
#[test]
fn a_takes_positions_are_listed_in_take_seconds() {
    let dir = synthetic_take("positions");
    let m = manifest(&dir);
    let (ctx, _) = TakeContext::open(
        &dir,
        &m,
        settings(None),
        Masks::Cut,
        |_| Ok(None),
        |_| Ok(None),
        |_, _| {},
    )
    .unwrap();
    let mut want = Vec::new();
    for (e, w) in m.recordings.iter().zip(&ctx.entries) {
        let length = ctx.files[w.parts.clone()]
            .iter()
            .map(|f| f.duration_s)
            .sum();
        let at = w.offset_s;
        want.extend(
            take::listed_positions(&m, std::slice::from_ref(e), length)
                .into_iter()
                .map(|(s, f, t, p, c)| (s + at, f + at, t.map(|t| t + at), p, c)),
        );
    }
    assert_eq!(ctx.listed_positions(&m), want);
    assert_eq!(want.len(), 5);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Masks that arrive out of order, or meet, are held in order and merged,
/// as `take::open_entry` holds them.
#[test]
fn masks_out_of_order_or_meeting_are_held_in_order_and_merged() {
    let dir = synthetic_take("merged");
    let mut m = manifest(&dir);
    m.recordings[0].masked = vec![[30.0, 35.0], [10.0, 20.0], [20.0, 25.0]];
    m.recordings[1].masked = vec![[5.0, 6.0]];
    m.recordings[2].masked = Vec::new();
    let (ctx, _) = TakeContext::open(
        &dir,
        &m,
        settings(None),
        Masks::Kept,
        |_| Ok(None),
        |_| Ok(None),
        |_, _| {},
    )
    .unwrap();
    assert_eq!(ctx.masked, [(10.0, 25.0), (30.0, 35.0), (65.0, 66.0)]);
    let _ = std::fs::remove_dir_all(&dir);
}

/// A take's readings are cut at each change of position and each end of
/// settling; the next file of a take in the same position is no change.
#[test]
fn a_takes_reading_breaks_cut_at_changes_of_position_and_settling() {
    let dir = synthetic_take("cuts");
    let m: Manifest = toml::from_str(
        "bph = 28800\n\
         [[recording]]\nfile = \"a.wav\"\nposition = \"DU\"\n\
         [[recording]]\nfile = \"b.wav\"\nposition = \"DU\"\n\
         [[recording]]\nfile = \"c.wav\"\nposition = \"CU\"\n",
    )
    .unwrap();
    let (ctx, _) = TakeContext::open(
        &dir,
        &m,
        settings(None),
        Masks::Cut,
        |_| Ok(None),
        |_| Ok(None),
        |_, _| {},
    )
    .unwrap();
    let at: Vec<f64> = ctx.reading_breaks(&m).iter().map(|b| b.audio_s).collect();
    // a.wav settles for core's 20 s. b.wav (60 s on) is in a.wav's
    // position: no cut where it starts (where its own settling ends is
    // `take::reading_window`'s to say). c.wav changes to CU at 121 s and
    // settles to 141 s.
    assert!(!at.contains(&60.0), "{at:?}");
    for t in [20.0, 121.0, 141.0] {
        assert!(at.contains(&t), "{t} in {at:?}");
    }
    assert!(ctx.reading_breaks(&m).iter().all(|b| b.gap_s == 0.0));
    let _ = std::fs::remove_dir_all(&dir);
}
