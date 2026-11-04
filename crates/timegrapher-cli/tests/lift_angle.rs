//! `long` and `series` take the lift angle from `--lift`, else the
//! calibre (or a take's session file), else 52°, and say so when they
//! fall back: on stderr, and as `lift_unknown` in their JSON.
use serde_json::{json, Value};
use std::path::PathBuf;
use std::process::{Command, Output};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "tg-lift-angle-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("calibres.json"),
            json!({
                "schema": "timegrapher.user-calibres/1",
                "custom": [
                    {"name": "Bench 44", "bph": 28800, "lift_angle_deg": 44.0},
                    {"name": "Bench Unknown", "bph": 28800},
                    {"name": "Bench Wide", "bph": 28800, "lift_angle_deg": 120.0},
                ],
            })
            .to_string(),
        )
        .unwrap();
        let fixture = Self(dir);
        let out = fixture.run(&["synth", fixture.wav().to_str().unwrap(), "--duration", "40"]);
        assert!(out.status.success(), "{}", stderr(&out));
        fixture
    }
    fn wav(&self) -> PathBuf {
        self.0.join("watch.wav")
    }
    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_timegrapher"))
            .env("TIMEGRAPHER_CONFIG_DIR", &self.0)
            .args(args)
            .output()
            .unwrap()
    }
    /// The lift angle `command` used, from its JSON settings, and its stderr.
    fn lift(&self, command: &str, extra: &[&str]) -> (f64, String) {
        let (lift, err, _) = self.lift_read(command, self.wav(), extra);
        (lift, err)
    }
    /// As [`Fixture::lift`], of `input`, with whether the JSON carries
    /// the `lift_unknown` caveat.
    fn lift_read(&self, command: &str, input: PathBuf, extra: &[&str]) -> (f64, String, bool) {
        let wav = input;
        let out_dir = self.0.join(format!(
            "out-{command}-{}-{}",
            wav.file_name().unwrap().to_string_lossy(),
            extra.join("-")
        ));
        let mut args = vec![command, wav.to_str().unwrap(), "--json"];
        let out_dir = out_dir.to_str().unwrap().to_string();
        if command == "long" {
            args.extend(["--out", &out_dir]);
        }
        args.extend(extra);
        let out = self.run(&args);
        assert!(
            out.status.success(),
            "{command} {extra:?}: {}",
            stderr(&out)
        );
        let doc: Value = serde_json::from_slice(&out.stdout).unwrap();
        let lift = doc["input"]["settings"]["lift_deg"].as_f64().unwrap();
        let unknown = doc["caveats"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["code"] == "lift_unknown");
        (lift, stderr(&out), unknown)
    }
    /// A take folder holding the fixture's recording, its session file
    /// giving `lift` when there is one.
    fn take(&self, name: &str, lift: Option<f64>) -> PathBuf {
        let dir = self.0.join(name);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::copy(self.wav(), dir.join("watch.wav")).unwrap();
        let lift = lift.map_or(String::new(), |l| format!("lift = {l}\n"));
        std::fs::write(
            dir.join("session.toml"),
            format!("{lift}\n[[recording]]\nfile = \"watch.wav\"\nposition = \"DU\"\n"),
        )
        .unwrap();
        dir
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

fn has_note(err: &str) -> bool {
    err.lines()
        .any(|l| l.starts_with("note: lift 52°, the default"))
}

#[test]
fn a_missing_recording_gets_no_lift_note() {
    let f = Fixture::new();
    let missing = f.0.join("missing.wav");
    for command in ["long", "series"] {
        let out = f.run(&[command, missing.to_str().unwrap()]);
        assert!(!out.status.success(), "{command}");
        assert!(!has_note(&stderr(&out)), "{command}: {}", stderr(&out));
    }
}

#[test]
fn long_and_series_take_the_lift_from_the_flag_then_the_calibre_then_52() {
    let f = Fixture::new();
    for command in ["long", "series"] {
        let (lift, err) = f.lift(command, &["--calibre", "Bench 44"]);
        assert_eq!(lift, 44.0, "{command}");
        assert!(!has_note(&err), "{command}: {err}");

        let (lift, err) = f.lift(command, &["--calibre", "Bench 44", "--lift", "50"]);
        assert_eq!(lift, 50.0, "{command}");
        assert!(!has_note(&err), "{command}: {err}");

        let (lift, err) = f.lift(command, &["--calibre", "Bench Unknown"]);
        assert_eq!(lift, 52.0, "{command}");
        assert!(
            err.contains(
                "note: lift 52°, the default; the calibre's lift angle is not in the table"
            ),
            "{command}: {err}"
        );

        let (lift, err) = f.lift(command, &["--calibre", "Bench Wide"]);
        assert_eq!(lift, 52.0, "{command}");
        assert!(
            err.contains(
                "note: lift 52°, the default; the calibre's lift angle of 120° is set aside, outside 10 to 90°"
            ),
            "{command}: {err}"
        );

        let (lift, err) = f.lift(command, &["--calibre", "Bench Unknown", "--lift", "48"]);
        assert_eq!(lift, 48.0, "{command}");
        assert!(!has_note(&err), "{command}: {err}");

        let (lift, err) = f.lift(command, &[]);
        assert_eq!(lift, 52.0, "{command}");
        assert!(
            err.contains("note: lift 52°, the default; calibre not set"),
            "{command}: {err}"
        );
    }
}

/// The JSON says the lift is the default (`lift_unknown`, guard G8)
/// whenever it is: with no lift given, or a calibre with none; never with
/// `--lift`, a calibre's lift, or a take's session-file lift (#1019).
#[test]
fn long_and_series_json_say_when_the_lift_is_the_default() {
    let f = Fixture::new();
    let with = f.take("with-lift", Some(49.0));
    let without = f.take("without-lift", None);
    for command in ["long", "series"] {
        let cases: [(PathBuf, &[&str], f64, bool); 6] = [
            (f.wav(), &[], 52.0, true),
            (f.wav(), &["--calibre", "Bench Unknown"], 52.0, true),
            (f.wav(), &["--lift", "50"], 50.0, false),
            (f.wav(), &["--calibre", "Bench 44"], 44.0, false),
            (with.clone(), &[], 49.0, false),
            (without.clone(), &[], 52.0, true),
        ];
        for (input, extra, want_lift, want_unknown) in cases {
            let (lift, err, unknown) = f.lift_read(command, input.clone(), extra);
            let case = format!("{command} {} {extra:?}", input.display());
            assert_eq!(lift, want_lift, "{case}: {err}");
            assert_eq!(unknown, want_unknown, "{case}: {err}");
        }
    }
}

/// `analyze` says why its lift is the default in the words `long` and
/// `series` use, a calibre's set-aside lift among them.
#[test]
fn analyze_says_why_its_lift_is_the_default() {
    let f = Fixture::new();
    let wav = f.wav();
    for (calibre, why) in [
        (None, "calibre not set"),
        (
            Some("Bench Unknown"),
            "the calibre's lift angle is not in the table",
        ),
        (
            Some("Bench Wide"),
            "the calibre's lift angle of 120° is set aside, outside 10 to 90°",
        ),
    ] {
        let mut args = vec!["analyze", wav.to_str().unwrap()];
        args.extend(calibre.iter().flat_map(|c| ["--calibre", *c]));
        let out = f.run(&args);
        assert!(out.status.success(), "{calibre:?}: {}", stderr(&out));
        let text = String::from_utf8_lossy(&out.stdout);
        assert!(
            text.contains(&format!("lift angle 52°, the default; {why}")),
            "{calibre:?}: {text}"
        );
    }
    let out = f.run(&["analyze", wav.to_str().unwrap(), "--calibre", "Bench 44"]);
    assert!(!String::from_utf8_lossy(&out.stdout).contains("the default"));
}
