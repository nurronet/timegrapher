//! Baseline movement through the real command, with a deterministic local take.
use serde_json::{json, Value};
use std::{
    path::PathBuf,
    process::{Command, Output},
};
use timegrapher_core::synth::{generate, SynthConfig};

struct Fixture {
    root: PathBuf,
    baseline: Value,
}
impl Fixture {
    fn new(name: &str) -> Self {
        let root =
            std::env::temp_dir().join(format!("tg-regress-drift-{name}-{}", std::process::id()));
        let take = root.join("watch");
        std::fs::create_dir_all(&take).unwrap();
        let sound = generate(
            &SynthConfig {
                duration_s: 40.0,
                rate_s_per_day: 10.0,
                beat_error_ms: 0.6,
                ..Default::default()
            },
            |_| 270.0,
            |_| 0.0,
        );
        timegrapher_core::audio::write_wav(&take.join("watch.wav"), &sound).unwrap();
        std::fs::write(
            take.join("session.toml"),
            "bph=28800\nlift=52\nsettle_s=0\n[[recording]]\nfile='watch.wav'\nposition='DU'\n",
        )
        .unwrap();
        let mut f = Self {
            root,
            baseline: Value::Null,
        };
        let out = f.run(&["--write-baseline"]);
        assert!(out.status.success(), "{out:?}");
        f.baseline =
            serde_json::from_slice(&std::fs::read(f.root.join("baseline.json")).unwrap()).unwrap();
        assert!(f.entry()["beats"].as_u64().unwrap() > 10);
        f
    }
    fn entry(&self) -> &Value {
        &self.baseline["takes"]["watch/session.toml"][0]
    }
    fn set_old(&self, changes: &[(&str, Value)]) {
        let mut old = self.baseline.clone();
        for (field, value) in changes {
            old["takes"]["watch/session.toml"][0][field] = value.clone();
        }
        std::fs::write(
            self.root.join("baseline.json"),
            serde_json::to_vec(&old).unwrap(),
        )
        .unwrap();
    }
    fn run(&self, args: &[&str]) -> Output {
        Command::new(
            std::env::var_os("TIMEGRAPHER_CLI")
                .unwrap_or_else(|| env!("CARGO_BIN_EXE_timegrapher").into()),
        )
        .arg("regress")
        .arg(&self.root)
        .arg("--baseline")
        .arg(self.root.join("baseline.json"))
        .args(["--quiet", "--jobs", "1"])
        .args(args)
        .output()
        .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

#[test]
fn gained_beats_fail_and_name_the_reading_and_both_counts() {
    let f = Fixture::new("gains");
    let beats = f.entry()["beats"].as_u64().unwrap();
    f.set_old(&[("beats", json!(beats - 4))]);
    let out = f.run(&[]);
    let text = String::from_utf8(out.stdout).unwrap();
    assert_eq!(out.status.code(), Some(3), "{text}");
    assert!(
        text.contains(&format!(
            "watch #1: beats {beats} (was {}): GAINED BEATS",
            beats - 4
        )),
        "{text}"
    );
    // The matching loss still fails at the same margin.
    f.set_old(&[("beats", json!(beats + 4))]);
    let out = f.run(&[]);
    let text = String::from_utf8(out.stdout).unwrap();
    assert_eq!(out.status.code(), Some(3), "{text}");
    assert!(
        text.contains(&format!(
            "watch #1: beats {beats} (was {}): LOST BEATS",
            beats + 4
        )),
        "{text}"
    );
}

#[test]
fn within_margin_gains_and_small_field_changes_are_reported_without_failing() {
    let f = Fixture::new("drift");
    let beats = f.entry()["beats"].as_u64().unwrap();
    let rate = f.entry()["rate_s_per_day"].as_f64().unwrap();
    f.set_old(&[
        ("beats", json!(beats - 3)),
        ("rate_s_per_day", json!(rate - 0.125)),
    ]);
    let out = f.run(&[]);
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(out.status.success(), "{text}");
    assert!(
        text.contains("beats: 1 entries, largest 3.000000e0"),
        "{text}"
    );
    assert!(
        text.contains("rate_s_per_day: 1 entries, largest 1.250000e-1"),
        "{text}"
    );
    assert!(!text.contains("amplitude_deg: 1 entries"), "{text}");
    // --all only expands the detail rows; the summary is independent of it.
    let all = f.run(&["--all"]);
    assert!(all.status.success());
    assert!(String::from_utf8(all.stdout)
        .unwrap()
        .contains("beats: 1 entries, largest 3.000000e0"));
}
