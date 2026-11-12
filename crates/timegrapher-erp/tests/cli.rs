//! The `timegrapher-erp` program itself, run as a subprocess against the
//! stand-in ERP (see `common`): settings from the environment, the PIN piped
//! in, a real `session` summary on disk.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use serde_json::{json, Value};
use timegrapher_erp::testing::{start_erp, Erp, KEY, PIN, SECRET, TOKEN};

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tg-erp-cli-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A `timegrapher session` summary with two positions and one with no beat.
fn write_summary(dir: &Path) -> PathBuf {
    let reading = |position: &str, rate: f64, heard: bool| {
        json!({"label": format!("take {position}"), "position": position, "bph": 28800,
               "measurement": {"start_s": 20.0, "end_s": 80.0, "beats": 120,
                               "rate_s_per_day": rate, "beat_error_ms": 0.31,
                               "beat_error_unlock_ms": 0.28, "amplitude_deg": 281.123456789,
                               "watch_heard": heard}})
    };
    let summary = json!({"schema": "timegrapher.session/2", "software": {"version": "1.0.0"},
        "input": {"settings": {"lift_deg": 52.0}},
        "readings": [reading("CH", 12.3456789, true), reading("3H", -4.5, true), reading("CB", 0.0, false)]});
    let folder = dir.join("session_report");
    std::fs::create_dir_all(&folder).unwrap();
    let path = folder.join("summary.json");
    std::fs::write(&path, summary.to_string()).unwrap();
    path
}

fn program(erp: &Erp, home: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_timegrapher-erp"));
    command
        .env("TIMEGRAPHER_ERP_DIR", home)
        .env("TIMEGRAPHER_ERP_URL", &erp.url)
        .env("TIMEGRAPHER_ERP_KEY", KEY)
        .env("TIMEGRAPHER_ERP_SECRET", SECRET)
        .env("TIMEGRAPHER_ERP_STATION", "STATION-01");
    command
}

fn run_with_input(mut command: Command, input: &str) -> Output {
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

fn text(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

#[test]
fn a_session_is_sent_signed_with_the_operator_attested_and_the_pin_never_kept() {
    let erp = start_erp();
    let home = temp_dir("submit");
    let summary = write_summary(&home);

    let mut command = program(&erp, &home);
    command.args(["submit"]).arg(home.as_os_str()).args([
        "--identity",
        "ID-1",
        "--operator",
        "OP1",
    ]);
    let out = run_with_input(command, &format!("{PIN}\n"));
    let shown = text(&out);
    assert!(out.status.success(), "{shown}");
    assert!(
        shown.contains("Left out: take CB: no regular beat"),
        "{shown}"
    );
    assert!(shown.contains("Dial Up: sent. Pass; Attested"), "{shown}");
    assert!(shown.contains("Crown Up: sent. Pass; Attested"), "{shown}");
    assert!(
        shown.contains("2 of 2 positions accepted; 2 attested."),
        "{shown}"
    );
    assert!(
        shown.contains("Suggested grade: B - Serviceable"),
        "{shown}"
    );
    assert!(shown.contains("Signed in: Op One"), "{shown}");

    // The key was made, registered, and noted as registered.
    assert_eq!(
        std::fs::read(home.join("workstation_signing_key.bin"))
            .unwrap()
            .len(),
        64
    );
    assert!(home.join("workstation_signing_key.registered").exists());

    // The PIN and the token appear only where they belong; one ERP session.
    let state = erp.state.lock().unwrap();
    assert_eq!(state.sessions, 1);
    // The type is the one the ERP recommends for this identity (it has a baseline).
    let sent: Value = serde_json::from_str(
        &state
            .log
            .iter()
            .find(|(n, _, _)| n == "submit_timing_result")
            .unwrap()
            .2,
    )
    .unwrap();
    assert_eq!(sent["measurement_type"], "Follow-up Certification");
    assert_eq!(sent["operator_id"], "OP1");
    assert_eq!(sent["identity_id"], "ID-1");
    for (name, _, body) in &state.log {
        match name.as_str() {
            "operator_login" => assert!(body.contains(PIN)),
            "submit_timing_result" => assert!(body.contains(TOKEN) && !body.contains(PIN)),
            _ => assert!(!body.contains(PIN), "{name}"),
        }
    }
    assert!(!shown.contains(PIN) && !shown.contains(TOKEN), "{shown}");
    for file in [&summary, &home.join("erp.toml")] {
        if file.exists() {
            let kept = std::fs::read_to_string(file).unwrap();
            assert!(!kept.contains(PIN) && !kept.contains(TOKEN));
        }
    }
    // The sign-in ended when the program did, after the session was completed.
    let names: Vec<&str> = state.log.iter().map(|(n, _, _)| n.as_str()).collect();
    assert_eq!(
        &names[names.len() - 2..],
        ["complete_measurement_session", "operator_logout"]
    );
    std::fs::remove_dir_all(&home).unwrap();
}

#[test]
fn the_second_run_does_not_register_the_key_again() {
    let erp = start_erp();
    let home = temp_dir("again");
    write_summary(&home);
    for _ in 0..2 {
        let mut command = program(&erp, &home);
        command.arg("submit").arg(home.as_os_str()).args([
            "--identity",
            "ID-1",
            "--operator",
            "OP1",
        ]);
        let out = run_with_input(command, &format!("{PIN}\n"));
        assert!(out.status.success(), "{}", text(&out));
    }
    let registrations = erp
        .state
        .lock()
        .unwrap()
        .log
        .iter()
        .filter(|(n, _, _)| n == "register_key")
        .count();
    assert_eq!(registrations, 1);
    std::fs::remove_dir_all(&home).unwrap();
}

#[test]
fn a_new_key_the_erp_already_holds_another_for_stops_the_run() {
    // The lost-key case: the ERP has a key under this ID, and this station's
    // is new. Nothing signed with it would verify, so nothing is sent.
    let erp = start_erp();
    erp.state
        .lock()
        .unwrap()
        .keys
        .insert("STATION-01-workstation".to_string(), vec![0u8; 32]);
    let home = temp_dir("lost-key");
    write_summary(&home);
    let mut command = program(&erp, &home);
    command
        .arg("submit")
        .arg(home.as_os_str())
        .args(["--identity", "ID-1", "--operator", "OP1"]);
    let out = run_with_input(command, &format!("{PIN}\n"));
    let shown = text(&out);
    assert!(!out.status.success(), "{shown}");
    assert!(shown.contains("already holds a different key"), "{shown}");
    assert!(!erp
        .state
        .lock()
        .unwrap()
        .log
        .iter()
        .any(|(n, _, _)| n == "submit_timing_result" || n == "operator_login"));
    std::fs::remove_dir_all(&home).unwrap();
}

#[test]
fn a_wrong_pin_stops_the_run_before_anything_is_sent() {
    let erp = start_erp();
    let home = temp_dir("wrong-pin");
    write_summary(&home);
    let mut command = program(&erp, &home);
    command
        .arg("submit")
        .arg(home.as_os_str())
        .args(["--identity", "ID-1", "--operator", "OP1"]);
    let out = run_with_input(command, "111111\n");
    let shown = text(&out);
    assert!(!out.status.success(), "{shown}");
    assert!(
        shown.contains("The Operator ID or PIN is wrong."),
        "{shown}"
    );
    assert_eq!(erp.state.lock().unwrap().sessions, 0);
    std::fs::remove_dir_all(&home).unwrap();
}

#[test]
fn a_pin_of_the_wrong_length_is_refused_without_being_sent() {
    let erp = start_erp();
    let home = temp_dir("short-pin");
    write_summary(&home);
    let mut command = program(&erp, &home);
    command
        .arg("submit")
        .arg(home.as_os_str())
        .args(["--identity", "ID-1", "--operator", "OP1"]);
    let out = run_with_input(command, "1234\n");
    let shown = text(&out);
    assert!(!out.status.success(), "{shown}");
    assert!(shown.contains("6 to 8 digits"), "{shown}");
    assert!(!erp
        .state
        .lock()
        .unwrap()
        .log
        .iter()
        .any(|(n, _, _)| n == "operator_login"));
    std::fs::remove_dir_all(&home).unwrap();
}

#[test]
fn an_identity_the_erp_does_not_know_stops_the_run_with_a_suggestion() {
    let erp = start_erp();
    let home = temp_dir("unknown");
    write_summary(&home);
    let mut command = program(&erp, &home);
    command
        .arg("submit")
        .arg(home.as_os_str())
        .args(["--identity", "nope"]);
    let out = run_with_input(command, "");
    let shown = text(&out);
    assert!(!out.status.success(), "{shown}");
    assert!(shown.contains("not known to the ERP"), "{shown}");
    assert!(shown.contains("did you mean: ID-1"), "{shown}");
    assert_eq!(erp.state.lock().unwrap().sessions, 0);
    std::fs::remove_dir_all(&home).unwrap();
}

#[test]
fn without_an_operator_it_says_the_results_will_be_invalid() {
    let erp = start_erp();
    let home = temp_dir("no-operator");
    write_summary(&home);
    let mut command = program(&erp, &home);
    command
        .arg("submit")
        .arg(home.as_os_str())
        .args(["--identity", "ID-1"]);
    let out = run_with_input(command, "");
    let shown = text(&out);
    assert!(
        shown.contains("will record these results as Invalid"),
        "{shown}"
    );
    assert!(shown.contains("Dial Up: sent. Pass; Invalid"), "{shown}");
    std::fs::remove_dir_all(&home).unwrap();
}

#[test]
fn a_dry_run_shows_the_signed_bytes_and_touches_nothing() {
    let erp = start_erp();
    let home = temp_dir("dry");
    write_summary(&home);
    let mut command = program(&erp, &home);
    command.arg("submit").arg(home.as_os_str()).args([
        "--identity",
        "ID-1",
        "--operator",
        "OP1",
        "--dry-run",
    ]);
    let out = run_with_input(command, "");
    let shown = text(&out);
    assert!(out.status.success(), "{shown}");
    assert!(shown.contains("\"rate_s_per_day\":12.345679"), "{shown}");
    assert!(shown.contains("\"position\":\"Dial Up\""), "{shown}");
    assert!(
        shown.contains("Dry run: nothing was signed or sent."),
        "{shown}"
    );
    assert!(erp.state.lock().unwrap().log.is_empty());
    assert!(!home.join("workstation_signing_key.bin").exists());
    std::fs::remove_dir_all(&home).unwrap();
}

#[test]
fn test_lookup_and_search_report_what_the_erp_says() {
    let erp = start_erp();
    let home = temp_dir("read");
    let ok = |args: &[&str]| {
        let mut command = program(&erp, &home);
        command.args(args);
        let out = run_with_input(command, "");
        (out.status.success(), text(&out))
    };
    let (good, shown) = ok(&["test"]);
    assert!(good && shown.contains("API version: 1.5.0"), "{shown}");
    let (good, shown) = ok(&["lookup", "ID-1"]);
    assert!(
        good && shown.contains("Part: ETA 2824-2")
            && shown.contains("Recommended: Follow-up Certification"),
        "{shown}"
    );
    let (good, shown) = ok(&["lookup", "nope"]);
    assert!(!good && shown.contains("not known"), "{shown}");
    std::fs::remove_dir_all(&home).unwrap();
}

#[test]
fn setup_keeps_the_settings_and_refuses_an_insecure_address() {
    let erp = start_erp();
    let home = temp_dir("setup");
    let mut command = Command::new(env!("CARGO_BIN_EXE_timegrapher-erp"));
    command
        .env("TIMEGRAPHER_ERP_DIR", &home)
        .env("TIMEGRAPHER_ERP_SECRET", SECRET)
        .args([
            "setup",
            "--url",
            "http://erp.example.com",
            "--key",
            KEY,
            "--station",
            "S1",
        ]);
    let out = run_with_input(command, "");
    assert!(!out.status.success());
    assert!(text(&out).contains("https://"), "{}", text(&out));
    assert!(!home.join("erp.toml").exists());

    let mut command = Command::new(env!("CARGO_BIN_EXE_timegrapher-erp"));
    command
        .env("TIMEGRAPHER_ERP_DIR", &home)
        .env("TIMEGRAPHER_ERP_SECRET", SECRET)
        .args(["setup", "--url", &erp.url, "--key", KEY, "--station", "S1"]);
    let out = run_with_input(command, "");
    assert!(out.status.success(), "{}", text(&out));
    let kept: Value = toml_to_json(&std::fs::read_to_string(home.join("erp.toml")).unwrap());
    assert_eq!(kept["station_id"], "S1");
    assert_eq!(kept["api_key"], KEY);
    std::fs::remove_dir_all(&home).unwrap();
}

/// Just enough TOML for the flat file `setup` writes.
fn toml_to_json(text: &str) -> Value {
    let mut map = serde_json::Map::new();
    for line in text.lines() {
        if let Some((key, value)) = line.split_once(" = ") {
            map.insert(
                key.trim().to_string(),
                json!(value.trim().trim_matches('"')),
            );
        }
    }
    Value::Object(map)
}
