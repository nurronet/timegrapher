//! Each corpus take opened as the app opens it, headless, reads what the
//! command line reads for it (#514): each recording the take's session.toml
//! lists (the whole take, ".", or one file of it), opened in the app,
//! reads on the cards what `timegrapher session` reads for that entry, to
//! the last bit. Both read through core, so any difference is logic
//! written twice.
//!
//! The same holds for core's synthetic edge takes, written afresh.
//!
//! Needs the corpus (`TIMEGRAPHER_CORPUS`, its audio fetched) and a built
//! `timegrapher` (`TIMEGRAPHER_CLI`); the edge takes need only the latter.
//! With neither set it skips, saying so; with `TIMEGRAPHER_REQUIRE_CORPUS`
//! set, as in CI, a missing one fails.

use super::*;
use std::time::Instant;

/// The longest the app may take to open one take: the overnight take is
/// eight hours of audio.
const OPEN_LIMIT_S: u64 = 1200;

/// The corpus root and the command line, or none where the test skips.
fn corpus_and_cli() -> Option<(PathBuf, PathBuf)> {
    let required = std::env::var_os("TIMEGRAPHER_REQUIRE_CORPUS").is_some();
    let root = std::env::var_os("TIMEGRAPHER_CORPUS").map(PathBuf::from);
    let cli = std::env::var_os("TIMEGRAPHER_CLI").map(PathBuf::from);
    match (root, cli) {
        (Some(root), Some(cli)) => {
            assert!(
                root.join("corpus.json").is_file(),
                "no corpus.json in {root:?}"
            );
            assert!(cli.is_file(), "no command line at {cli:?}");
            Some((root, cli))
        }
        _ if required => panic!("TIMEGRAPHER_CORPUS and TIMEGRAPHER_CLI are both needed"),
        _ => {
            eprintln!("SKIP: set TIMEGRAPHER_CORPUS and TIMEGRAPHER_CLI to compare the app with the command line");
            None
        }
    }
}

/// The takes the corpus lists that have a session.toml, by id, but for its
/// holdout takes: their audio is fetched only for `regress --holdout`, and
/// their readings stay sealed (docs/regress.md, "Holdout takes").
fn takes(root: &Path) -> Vec<String> {
    let json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(root.join("corpus.json")).unwrap()).unwrap();
    json["takes"]
        .as_array()
        .expect("corpus.json lists its takes")
        .iter()
        .filter(|t| t["tier"].as_str() != Some("holdout"))
        .filter_map(|t| t["id"].as_str().map(str::to_string))
        .filter(|id| root.join("takes").join(id).join("session.toml").is_file())
        .collect()
}

/// The take opened as File > Open Recording opens it, waited for.
fn opened(dir: &Path) -> TimegrapherApp {
    let mut app = TimegrapherApp::with_devices(Vec::new(), None);
    // At 10 s, so edge_45s (25 s measured) guards against the whole take
    // needing a multiple of Average Over again.
    app.set_average(10.0);
    app.start_batch(dir);
    let t = Instant::now();
    while app.batch.is_some() && t.elapsed().as_secs() < OPEN_LIMIT_S {
        app.poll();
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(
        app.batch.is_none(),
        "{dir:?} still opening after {OPEN_LIMIT_S} s"
    );
    app
}

/// `timegrapher session --json` for the take: its readings, by label.
fn session_readings(cli: &Path, dir: &Path) -> Vec<serde_json::Value> {
    let out = std::env::temp_dir().join(format!(
        "tg-app-corpus-{}-{}",
        dir.file_name().unwrap().to_string_lossy(),
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&out);
    let run = std::process::Command::new(cli)
        .arg("session")
        .arg(dir)
        .arg("--json")
        .arg("--out")
        .arg(&out)
        .output()
        .expect("the command line runs");
    let _ = std::fs::remove_dir_all(&out);
    assert!(
        run.status.success(),
        "session {dir:?}: {}",
        String::from_utf8_lossy(&run.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&run.stdout).expect("session JSON");
    json["readings"].as_array().cloned().unwrap_or_default()
}

/// Where the app's whole-take reading and the command line's differ, in
/// words, figures and caveats alike; none where they are the same.
fn differences(id: &str, app: &Option<WholeTake>, cli: &serde_json::Value) -> Vec<String> {
    let m = &cli["measurement"];
    let Some(w) = app else {
        return vec![format!(
            "{id}: the app has no whole-take reading; session has {m}"
        )];
    };
    let mut out = Vec::new();
    let mut same = |what: &str, a: Option<f64>, key: &str| {
        let b = m[key].as_f64();
        if a != b {
            out.push(format!("{id}: {what} {a:?} in the app, {b:?} from session"));
        }
    };
    same("rate", w.m.rate_s_per_day, "rate_s_per_day");
    same("amplitude", w.m.amplitude_deg, "amplitude_deg");
    same("beat error", w.m.beat_error_ms, "beat_error_ms");
    same("start", Some(w.m.start_s), "start_s");
    same("end", Some(w.m.end_s), "end_s");
    same("beats", Some(w.m.beats as f64), "beats");
    // The caveats each names, in any order.
    let mut app_codes = timegrapher_core::edge_takes::caveat_codes(&w.m);
    let mut cli_codes: Vec<String> = m["caveats"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|c| c["code"].as_str().map(str::to_string))
        .collect();
    app_codes.sort();
    cli_codes.sort();
    if app_codes != cli_codes {
        out.push(format!(
            "{id}: caveats {app_codes:?} in the app, {cli_codes:?} from session"
        ));
    }
    out
}

/// Corpus entries the app still reads differently from session, and why.
/// Each must still differ, so the list only shrinks as the app is fixed. A
/// file the session file lists more than once is named with its entry's
/// place among them: `file entry 1`.
const KNOWN: &[(&str, &str)] = &[(
    "ym42_CR_long/take_part01-003.flac entry 1",
    "the first of two entries for one file, with its own measure_s: the app does not keep \
         its end",
)];

/// Each recording the takes under `root` list, read on the app's cards and
/// by `timegrapher session`, compared: what differs and is not on `known`,
/// or is on it and no longer differs. Takes, and then recordings, are read
/// side by side, one to a core, so the whole corpus fits CI's time.
fn compare(root: &Path, cli: &Path, known: &[(&str, &str)]) -> Vec<String> {
    let ids = takes(root);
    let readings = timegrapher_core::cores::on_every_core(&ids, |id| {
        session_readings(cli, &root.join("takes").join(id))
    });
    let entries: Vec<(String, serde_json::Value)> = ids
        .iter()
        .zip(readings)
        .flat_map(|(id, rs)| rs.into_iter().map(move |r| (id.clone(), r)))
        .collect();
    let label = |r: &serde_json::Value| r["label"].as_str().expect("a reading's label").to_string();
    // Each reading's name, with its place among the entries for its file
    // when there are several.
    let names: Vec<String> = entries
        .iter()
        .enumerate()
        .map(|(i, (id, r))| {
            let file =
                |(other, o): &(String, serde_json::Value)| other == id && label(o) == label(r);
            let name = format!("{id}/{}", label(r));
            if entries.iter().filter(|e| file(e)).count() > 1 {
                let place = entries[..i].iter().filter(|e| file(e)).count() + 1;
                format!("{name} entry {place}")
            } else {
                name
            }
        })
        .collect();
    let named: Vec<_> = entries.iter().zip(names).collect();
    let found = timegrapher_core::cores::on_every_core(&named, |((id, reading), name)| {
        let label = label(reading);
        let name = name.clone();
        let app = opened(&root.join("takes").join(id).join(label));
        let found = differences(&name, &app.whole_take(), reading);
        match known.iter().find(|(k, _)| *k == name) {
            Some((_, why)) if !found.is_empty() => {
                eprintln!("{name}: differs, as known ({why})");
                Vec::new()
            }
            Some(_) => vec![format!("{name}: now the same; take it off its known list")],
            None => {
                eprintln!(
                    "{name}: {}",
                    if found.is_empty() { "same" } else { "DIFFERS" }
                );
                found
            }
        }
    });
    eprintln!("{} recordings compared", entries.len());
    assert!(!entries.is_empty(), "no recording to compare in {root:?}");
    found.into_iter().flatten().collect()
}

/// Edge takes the app still reads differently from session, and why. Each
/// must still differ, so the list only shrinks as the app is fixed.
const EDGE_KNOWN: &[(&str, &str)] = &[
    (
        "edge_sleep_two_entries/. (0–90 s)",
        "one of two readings of one file: the app opens a file whole, not a stretch of it",
    ),
    (
        "edge_sleep_two_entries/. (90–180 s)",
        "one of two readings of one file: the app opens a file whole, not a stretch of it",
    ),
    (
        "edge_reads_nothing/. (0–90 s)",
        "a stretch of a file: the app opens a file whole, not a stretch of it",
    ),
];

/// Every recording each corpus take lists reads the same on the app's
/// cards as from `timegrapher session`.
#[test]
fn the_app_reads_each_corpus_take_as_the_command_line_does() {
    let Some((root, cli)) = corpus_and_cli() else {
        return;
    };
    let wrong = compare(&root, &cli, KNOWN);
    assert!(
        wrong.is_empty(),
        "the app and session differ:\n{}",
        wrong.join("\n")
    );
}

/// Each synthetic edge take (core's `edge_takes`: short, paused, slept,
/// silent, unlisted breaks, a drifting card) reads the same on the app's
/// cards as from `timegrapher session`. Needs only `TIMEGRAPHER_CLI`.
#[test]
fn the_app_reads_each_edge_take_as_the_command_line_does() {
    let required = std::env::var_os("TIMEGRAPHER_REQUIRE_CORPUS").is_some();
    let Some(cli) = std::env::var_os("TIMEGRAPHER_CLI").map(PathBuf::from) else {
        assert!(!required, "TIMEGRAPHER_CLI is needed");
        eprintln!("SKIP: set TIMEGRAPHER_CLI to compare the app with the command line");
        return;
    };
    let root = std::env::temp_dir().join(format!("tg-app-edge-takes-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    timegrapher_core::edge_takes::write_all(&root).unwrap();
    let wrong = compare(&root, &cli, EDGE_KNOWN);
    let _ = std::fs::remove_dir_all(&root);
    assert!(
        wrong.is_empty(),
        "the app and session differ:\n{}",
        wrong.join("\n")
    );
}

/// Split entries retain their clock even without readable recorder metadata.
fn split_entries_clock_probe(metadata: &str, folder: bool) {
    use timegrapher_core::{audio, synth};
    let required = std::env::var_os("TIMEGRAPHER_REQUIRE_CORPUS").is_some();
    let Some(cli) = std::env::var_os("TIMEGRAPHER_CLI").map(PathBuf::from) else {
        assert!(!required, "TIMEGRAPHER_CLI is needed");
        eprintln!("SKIP: set TIMEGRAPHER_CLI for the duplicate-entry probe");
        return;
    };
    let root = std::env::temp_dir().join(format!(
        "tg-app-duplicate-clock-{metadata}-{folder}-{}",
        std::process::id()
    ));
    let dir = root.join(format!("takes/duplicate_clock_{metadata}_{folder}"));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&dir).unwrap();
    let sound = synth::generate(
        &synth::SynthConfig {
            duration_s: 300.0,
            rate_s_per_day: -7.0,
            ..Default::default()
        },
        |_| 270.0,
        |_| 0.0,
    );
    let fs = sound.sample_rate;
    audio::write_wav(&dir.join("take.wav"), &sound).unwrap();
    let mut clock = String::from("unix_s,frames\n");
    let mut frames: Vec<u32> = (0..=30).map(|i| i * 10 * fs).collect();
    frames.push(100 * fs - fs / 10);
    frames.sort_unstable();
    for frame in frames {
        let wall = 1_700_000_000.0
            + frame as f64 / fs as f64 * 1.00002
            + if frame >= 100 * fs { 60.0 } else { 0.0 };
        clock.push_str(&format!("{wall:.6},{frame}\n"));
    }
    std::fs::write(dir.join("clock.csv"), clock).unwrap();
    let record = serde_json::json!({"segments": ["take.wav"], "breaks": [
        {"frame": 100 * fs, "audio_s": 100.0, "gap_s": 60.0}
    ]})
    .to_string();
    match metadata {
        "valid" => std::fs::write(dir.join("session.json"), &record).unwrap(),
        "truncated" => {
            std::fs::write(dir.join("session.json"), &record[..record.len() / 2]).unwrap()
        }
        "missing" => {}
        _ => unreachable!(),
    }
    std::fs::write(
        dir.join("session.toml"),
        "bph = 28800\nlift = 52\n\
         [[recording]]\nfile = \"take.wav\"\nposition = \"DU\"\nend_s = 160\nclock = \"clock.csv\"\n\
         [[recording]]\nfile = \"take.wav\"\nposition = \"DD\"\nstart_s = 160\nclock = \"clock.csv\"\n",
    )
    .unwrap();
    let readings = session_readings(&cli, &dir);
    assert_eq!(readings.len(), 2);
    for reading in &readings {
        let rate = reading["measurement"]["rate_s_per_day"].as_f64().unwrap();
        assert!((rate + 8.728).abs() < 0.2, "corrected rate: {rate}");
    }
    let path = if folder {
        dir.clone()
    } else {
        dir.join("take.wav")
    };
    let mut app = opened(&path);
    let clock = app.take_clock.as_ref().expect("the app retains the clock");
    assert!((clock.ppm - 20.0).abs() < 0.1);
    assert_eq!(clock.breaks.len(), 1);
    assert!((clock.breaks[0].gap_s - 60.0).abs() < 0.01);
    for (position, reading) in [Position::DialUp, Position::DialDown]
        .into_iter()
        .zip(&readings)
    {
        app.position = position;
        let wrong = differences(metadata, &app.whole_take(), reading);
        assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn split_entries_clock_valid_metadata() {
    split_entries_clock_probe("valid", false);
}
#[test]
fn split_entries_clock_truncated_metadata() {
    split_entries_clock_probe("truncated", false);
}
#[test]
fn split_entries_clock_missing_metadata() {
    split_entries_clock_probe("missing", false);
}

#[test]
fn split_entries_clock_folder_valid_metadata() {
    split_entries_clock_probe("valid", true);
}

#[test]
fn split_entries_clock_folder_truncated_metadata() {
    split_entries_clock_probe("truncated", true);
}

#[test]
fn split_entries_clock_folder_missing_metadata() {
    split_entries_clock_probe("missing", true);
}
