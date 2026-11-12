//! `timegrapher report` carries the take's analysis (#400): its beats and
//! amplitude windows under `analysis/`, without the Tick Tock profiles, read
//! from the sound or from a take kept as its analysis alone.
use std::path::{Path, PathBuf};
use std::process::Command;
use timegrapher_core::analysis_take;
use timegrapher_core::session::Manifest;
use timegrapher_core::stream::StreamConfig;
use timegrapher_core::synth::{generate, SynthConfig};

fn scratch(name: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("tg-report-analysis-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A minute of a watch at 28,800 bph, 6 s/d fast.
fn watch_wav(path: &Path, seconds: f64) {
    let audio = generate(
        &SynthConfig {
            duration_s: seconds,
            rate_s_per_day: 6.0,
            beat_error_ms: 0.4,
            ..Default::default()
        },
        |_| 275.0,
        |_| 0.0,
    );
    timegrapher_core::audio::write_wav(path, &audio).unwrap();
}

/// `report` with `args`, its zip written in `dir/out` with a settings folder
/// of its own: the zip's files.
fn report(dir: &Path, args: &[&str]) -> Vec<(String, Vec<u8>)> {
    let out = dir.join("out");
    let _ = std::fs::remove_dir_all(&out);
    let run = Command::new(env!("CARGO_BIN_EXE_timegrapher"))
        .arg("report")
        .args(args)
        .arg("--out")
        .arg(&out)
        .env("TIMEGRAPHER_CONFIG_DIR", dir.join("config"))
        .output()
        .unwrap();
    assert!(
        run.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&run.stderr)
    );
    let zip = std::fs::read_dir(&out)
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| p.extension().is_some_and(|x| x == "zip"))
        .expect("a zip");
    timegrapher_core::archive::read_zip(&zip).unwrap()
}

fn names(files: &[(String, Vec<u8>)]) -> Vec<&str> {
    files.iter().map(|(p, _)| p.as_str()).collect()
}

/// The analysis in a report's files, read as a take's: unpacked into
/// `dir` and read with core's own reader.
fn analysis_of(files: &[(String, Vec<u8>)], dir: &Path) -> analysis_take::Stored {
    let _ = std::fs::remove_dir_all(dir);
    for (p, d) in files.iter().filter(|(p, _)| p.starts_with("analysis/")) {
        let to = dir.join(p);
        std::fs::create_dir_all(to.parent().unwrap()).unwrap();
        std::fs::write(to, d).unwrap();
    }
    analysis_take::read(dir).unwrap()
}

/// A recording's report holds its beats and windows beside the excerpt,
/// with no profiles; `--no-analysis` leaves them out and says so.
#[test]
fn a_report_carries_the_recordings_analysis() {
    let dir = scratch("sound");
    let wav = dir.join("watch.wav");
    watch_wav(&wav, 60.0);
    let wav_arg = wav.to_str().unwrap();
    let files = report(&dir, &[wav_arg, "--span", "20"]);
    let n = names(&files);
    for want in [
        "audio/excerpt.flac",
        "analysis/analysis.json",
        "analysis/beats.csv.gz",
        "analysis/windows.csv.gz",
        "analysis/profiles.bin.gz",
    ] {
        assert!(n.contains(&want), "{want} missing from {n:?}");
    }
    let s = analysis_of(&files, &dir.join("unpacked"));
    // The whole minute, not the 20 s excerpt: about 8 beats a second.
    assert!(s.log.beats.len() > 400, "{} beats", s.log.beats.len());
    assert!(!s.log.amplitude_windows.is_empty());
    assert!(s.profiles.is_empty());
    assert_eq!(s.log.bph, 28800);
    // Listed in the manifest with its checksum, as every file is.
    let manifest: serde_json::Value =
        serde_json::from_slice(&files.iter().find(|(p, _)| p == "manifest.json").unwrap().1)
            .unwrap();
    let listed: Vec<&str> = manifest["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["path"].as_str().unwrap())
        .collect();
    assert!(listed.contains(&"analysis/beats.csv.gz"), "{listed:?}");

    let without = report(&dir, &[wav_arg, "--span", "20", "--no-analysis"]);
    assert!(
        !names(&without).iter().any(|p| p.starts_with("analysis/")),
        "{:?}",
        names(&without)
    );
    let manifest: serde_json::Value = serde_json::from_slice(
        &without
            .iter()
            .find(|(p, _)| p == "manifest.json")
            .unwrap()
            .1,
    )
    .unwrap();
    assert!(
        manifest["left_out"].to_string().contains("--no-analysis"),
        "{}",
        manifest["left_out"]
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// A take kept as its analysis alone is reported with its analysis and no
/// sound: every beat it kept, without its profiles.
#[test]
fn an_analysis_only_take_is_reported_with_its_analysis() {
    let dir = scratch("kept");
    let wav = dir.join("watch.wav");
    watch_wav(&wav, 60.0);
    let toml = "bph = 28800\nlift = 52\n\n[[recording]]\nfile = \".\"\nposition = \"DU\"\n";
    let m: Manifest = toml::from_str(toml).unwrap();
    let mut cfg = StreamConfig::default();
    cfg.analysis.bph = Some(28800);
    cfg.analysis.amplitude.lift_deg = 52.0;
    let stored = analysis_take::measure(
        &[wav.as_path()],
        &cfg,
        &analysis_take::shape_stretches(&m, 60.0),
        serde_json::json!({"name": "timegrapher", "version": "test"}),
        |_| {},
    )
    .unwrap();
    assert!(!stored.profiles.is_empty());
    let take = dir.join("kept");
    let companions = vec![(
        timegrapher_core::take::MANIFEST.to_string(),
        format!("audio = false\n{toml}").into_bytes(),
    )];
    analysis_take::save(&take, &companions, &stored).unwrap();

    let files = report(&dir, &[take.to_str().unwrap()]);
    let n = names(&files);
    assert!(
        !n.iter().any(|p| p.starts_with("audio/")),
        "no sound to send: {n:?}"
    );
    assert!(n.contains(&"results/long-whole.json"), "{n:?}");
    let s = analysis_of(&files, &dir.join("unpacked"));
    assert_eq!(s.log.beats.len(), stored.log.beats.len());
    assert_eq!(
        s.log.amplitude_windows.len(),
        stored.log.amplitude_windows.len()
    );
    assert!(s.profiles.is_empty());
    assert_eq!(s.header.shape.len(), stored.header.shape.len());
    let manifest: serde_json::Value =
        serde_json::from_slice(&files.iter().find(|(p, _)| p == "manifest.json").unwrap().1)
            .unwrap();
    assert!(
        manifest["left_out"].to_string().contains("analysis alone"),
        "{}",
        manifest["left_out"]
    );
    let _ = std::fs::remove_dir_all(&dir);
}
