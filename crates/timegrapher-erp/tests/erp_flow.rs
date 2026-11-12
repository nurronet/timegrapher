//! A station talking to a stand-in ERP (see `common`).

use std::net::TcpListener;
use std::time::{Duration, SystemTime};

use serde_json::{json, Value};
use timegrapher_erp::client::{Client, ErpError, LoginOutcome};
use timegrapher_erp::config::Config;
use timegrapher_erp::operator::OperatorSession;
use timegrapher_erp::payload::{Context, Enhanced, PositionResult, Recording};
use timegrapher_erp::signing::Signer;
use timegrapher_erp::submit::{self, Report};
use timegrapher_erp::testing::{start_erp, Erp, KEY, PIN, SECRET, TOKEN};

fn config(erp: &Erp) -> Config {
    Config {
        base_url: erp.url.clone(),
        api_key: KEY.into(),
        api_secret: SECRET.into(),
        station_id: "STATION-01".into(),
        verify_tls: true,
        enhanced: false,
    }
}

fn temp_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("tg-erp-flow-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn context(operator_id: &str) -> Context {
    Context {
        station_id: "STATION-01".into(),
        station_version: "1.0.0".into(),
        operator_id: operator_id.into(),
        identity_id: "ID-1".into(),
        work_order: String::new(),
        qa_standard: "Workshop".into(),
        temperature_condition: "Room / 23C".into(),
        measurement_type: "Initial Certification".into(),
        timestamp_iso: "2026-11-11T09:30:00Z".into(),
    }
}

fn position(name: &'static str, rate: f64) -> PositionResult {
    PositionResult {
        position: name,
        rate_s_per_day: Some(rate),
        beat_error_ms: Some(0.2),
        amplitude_deg: Some(285.5),
        beat_frequency_bph: Some(28800.0),
        lift_angle_deg: Some(52.0),
        duration_seconds: Some(41.123456789),
        notes: "test".into(),
        enhanced: None,
    }
}

struct Bench {
    erp: Erp,
    client: Client,
    signer: Signer,
    dir: std::path::PathBuf,
}

/// A station that has made and registered its key.
fn bench(name: &str) -> Bench {
    let erp = start_erp();
    let client = Client::new(&config(&erp)).unwrap();
    let dir = temp_dir(name);
    let (signer, created) =
        Signer::load_or_create(&dir, &Signer::key_id_for("STATION-01")).unwrap();
    assert!(created);
    client
        .register_key(signer.key_id(), &signer.public_key_b64())
        .unwrap();
    Bench {
        erp,
        client,
        signer,
        dir,
    }
}

fn sign_in(b: &Bench) -> OperatorSession {
    match b
        .client
        .operator_login("OP1", PIN, SystemTime::now())
        .unwrap()
    {
        LoginOutcome::SignedIn(s) => s,
        LoginOutcome::Refused(why) => panic!("{why}"),
    }
}

fn run(
    b: &Bench,
    operator: &mut Option<OperatorSession>,
    signer: bool,
    results: &[PositionResult],
) -> Report {
    let ctx = context(operator.as_ref().map_or("", |s| s.operator_id.as_str()));
    submit::send_session(
        &b.client,
        signer.then_some(&b.signer),
        &ctx,
        operator,
        results,
        SystemTime::now,
    )
}

fn statuses(report: &Report) -> Vec<String> {
    report
        .sent
        .iter()
        .map(|s| {
            s.outcome
                .as_ref()
                .unwrap()
                .verification_status
                .clone()
                .unwrap()
        })
        .collect()
}

#[test]
fn a_signed_in_operator_and_a_registered_key_give_attested_results_in_one_session() {
    let b = bench("attested");
    let mut operator = Some(sign_in(&b));
    let report = run(
        &b,
        &mut operator,
        true,
        &[position("Dial Up", 12.3456789), position("Crown Up", -3.5)],
    );
    assert_eq!(statuses(&report), ["Attested", "Attested"]);
    assert_eq!(report.session.as_deref(), Some("SESSION-1"));
    // the second position was attached to the first one's session, not given its own
    assert_eq!(b.erp.state.lock().unwrap().sessions, 1);
    let grade = report.completed.unwrap().unwrap();
    assert_eq!(
        grade.suggested_functional_grade.as_deref(),
        Some("B - Serviceable")
    );
    assert!(operator.is_some());
    let _ = std::fs::remove_dir_all(&b.dir);
}

#[test]
fn without_an_operator_the_results_are_invalid_as_the_erp_records_them() {
    let b = bench("no-operator");
    let report = run(&b, &mut None, true, &[position("Dial Up", 1.0)]);
    assert_eq!(statuses(&report), ["Invalid"]);
    let _ = std::fs::remove_dir_all(&b.dir);
}

#[test]
fn a_station_that_cannot_sign_sends_unsigned_results() {
    let b = bench("unsigned");
    let mut operator = Some(sign_in(&b));
    let report = run(&b, &mut operator, false, &[position("Dial Up", 1.0)]);
    assert_eq!(statuses(&report), ["Unsigned"]);
    let _ = std::fs::remove_dir_all(&b.dir);
}

#[test]
fn a_sign_in_the_erp_has_ended_is_dropped_and_the_operator_told() {
    let b = bench("ended");
    let mut operator = Some(sign_in(&b));
    b.erp.state.lock().unwrap().ended_token = true;
    let report = run(&b, &mut operator, true, &[position("Dial Up", 1.0)]);
    assert_eq!(statuses(&report), ["Invalid"]);
    assert!(report.signin_rejected);
    assert!(operator.is_none());
    let _ = std::fs::remove_dir_all(&b.dir);
}

#[test]
fn a_wrong_pin_is_refused_in_the_erps_words() {
    let b = bench("wrong-pin");
    match b
        .client
        .operator_login("OP1", "111111", SystemTime::now())
        .unwrap()
    {
        LoginOutcome::Refused(why) => assert_eq!(why, "The Operator ID or PIN is wrong."),
        other => panic!("{other:?}"),
    }
    let _ = std::fs::remove_dir_all(&b.dir);
}

#[test]
fn every_figure_is_rounded_so_that_what_was_signed_is_what_the_erp_parses() {
    let b = bench("rounding");
    let mut operator = Some(sign_in(&b));
    // 12.3456789 and 41.123456789 carry more than six decimals
    let report = run(&b, &mut operator, true, &[position("Dial Up", 12.3456789)]);
    assert_eq!(statuses(&report), ["Attested"]);
    let _ = std::fs::remove_dir_all(&b.dir);
}

#[test]
fn the_stand_in_does_catch_a_figure_signed_unrounded_and_sent_with_six_decimals() {
    // The mistake this crate avoids: sign 12.3456789, send 12.345679.
    let b = bench("trap");
    let ctx = context("OP1");
    let mut result = position("Dial Up", 12.3456789);
    let mut signed = result.signed_fields(&ctx);
    signed["measurements"]["rate_s_per_day"] = json!(12.3456789);
    let signature = submit::signature_entry(&b.signer, &signed);
    result.rate_s_per_day = Some(12.345679);
    let sent = result.signed_fields(&ctx);
    let body = submit::wire_body(&sent, Some(signature), None, Some(TOKEN), None);
    let reply = b.client.submit_timing_result(&body).unwrap();
    assert_eq!(reply.verification_status.as_deref(), Some("Invalid"));
    let _ = std::fs::remove_dir_all(&b.dir);
}

#[test]
fn the_pin_and_the_token_go_only_where_they_belong() {
    let b = bench("secrets");
    let mut operator = Some(sign_in(&b));
    run(
        &b,
        &mut operator,
        true,
        &[position("Dial Up", 1.0), position("Dial Down", 2.0)],
    );
    b.client.operator_logout(operator.as_ref().unwrap());
    let state = b.erp.state.lock().unwrap();
    for (name, auth, body) in &state.log {
        assert_eq!(auth, &format!("token {KEY}:{SECRET}"), "{name}");
        match name.as_str() {
            "operator_login" => assert!(body.contains(PIN)),
            "submit_timing_result" => {
                assert!(!body.contains(PIN), "the PIN went with a result");
                assert!(body.contains(TOKEN), "the token did not go with a result");
            }
            "operator_logout" => assert!(body.contains(TOKEN)),
            _ => assert!(!body.contains(PIN) && !body.contains(TOKEN), "{name}"),
        }
    }
    // the signed bytes never held the token: it travels unsigned
    let (_, _, body) = state
        .log
        .iter()
        .find(|(n, _, _)| n == "submit_timing_result")
        .unwrap();
    let parsed: Value = serde_json::from_str(body).unwrap();
    assert!(parsed["signatures"][0]["signature_b64"].is_string());
    drop(state);
    let _ = std::fs::remove_dir_all(&b.dir);
}

#[test]
fn a_second_registration_of_the_same_key_is_refused_by_the_erp() {
    let b = bench("twice");
    let err = b
        .client
        .register_key(b.signer.key_id(), &b.signer.public_key_b64())
        .err()
        .unwrap();
    match err {
        ErpError::Status { code: 417, body } => assert!(body.contains("Key ID already exists")),
        other => panic!("{other:?}"),
    }
    let _ = std::fs::remove_dir_all(&b.dir);
}

#[test]
fn the_lookup_tells_what_is_known_and_what_is_not() {
    let b = bench("lookup");
    let found = b.client.lookup_identity("ID-1").unwrap();
    assert!(found.found);
    assert_eq!(
        found.recommended_measurement_type.as_deref(),
        Some("Follow-up Certification")
    );
    let missing = b.client.lookup_identity("nope").unwrap();
    assert!(!missing.found);
    assert_eq!(missing.suggestions.len(), 1);
    let _ = std::fs::remove_dir_all(&b.dir);
}

#[test]
fn a_key_the_erp_does_not_know_is_reported_not_hidden() {
    let b = bench("bad-key");
    let mut cfg = config(&b.erp);
    cfg.api_secret = "wrong".into();
    let client = Client::new(&cfg).unwrap();
    match client.ping() {
        Err(ErpError::Status { code: 403, .. }) => {}
        other => panic!("{other:?}"),
    }
    let _ = std::fs::remove_dir_all(&b.dir);
}

#[test]
fn a_redirect_is_not_followed_so_the_key_and_secret_go_nowhere_else() {
    let elsewhere = start_erp();
    let b = bench("redirect");
    b.erp.state.lock().unwrap().redirect_ping_to = Some(format!(
        "{}/api/method/ln_watch_inventory.freeerp.ping",
        elsewhere.url
    ));
    match b.client.ping() {
        Err(ErpError::Status { code: 302, .. }) => {}
        other => panic!("{other:?}"),
    }
    assert!(
        elsewhere.state.lock().unwrap().log.is_empty(),
        "the other server was sent the key and secret"
    );
    let _ = std::fs::remove_dir_all(&b.dir);
}

#[test]
fn an_unreachable_erp_is_a_network_error_not_a_hang() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    drop(listener);
    let cfg = Config {
        base_url: url,
        api_key: KEY.into(),
        api_secret: SECRET.into(),
        station_id: "S".into(),
        verify_tls: true,
        enhanced: false,
    };
    let started = std::time::Instant::now();
    match Client::new(&cfg).unwrap().ping() {
        Err(ErpError::Network(_)) => {}
        other => panic!("{other:?}"),
    }
    assert!(started.elapsed() < Duration::from_secs(15));
}

// ----- enhanced data

fn flac_bytes(n: u8) -> Vec<u8> {
    let mut v = b"fLaC".to_vec();
    v.extend((0..=255u8).map(|i| i.wrapping_mul(n)));
    v
}

fn enhanced_position(name: &'static str, rate: f64, with_recording: bool) -> PositionResult {
    let mut p = position(name, rate);
    p.enhanced = Some(Enhanced {
        block: json!({"version": 1, "measurement": {"rate_s_per_day": rate, "beats": 4100,
                      "caveats": [], "clean_fraction": 0.987654321}, "spans": [[20.0, 80.5]]}),
        recording: with_recording.then(|| Recording {
            flac: flac_bytes(3),
            name: format!("{name}.flac"),
            from_s: 20.0,
            duration_s: 60.5,
            sample_rate: 48000,
            channels: 1,
            bits_per_sample: 16,
        }),
    });
    p
}

#[test]
fn an_enhanced_position_is_signed_kept_and_its_recording_sent_and_checked() {
    let b = bench("enhanced");
    let mut operator = Some(sign_in(&b));
    let sent = enhanced_position("Dial Up", 2.5, true);
    let report = run(&b, &mut operator, true, std::slice::from_ref(&sent));
    // the station's signature covers the digest, and the ERP verified it
    assert_eq!(statuses(&report), ["Attested"]);
    let one = &report.sent[0];
    assert!(one.enhanced_kept);
    assert_eq!(
        one.recording.as_ref().unwrap().as_ref().unwrap(),
        &sent_len(&sent)
    );
    let state = b.erp.state.lock().unwrap();
    assert_eq!(state.results[0]["schema"], "ln_tg_timing_result_v2");
    assert_eq!(state.results[0]["enhanced_version"], 1);
    assert_eq!(state.enhanced.len(), 1);
    assert_eq!(state.enhanced[0].1, sent.enhanced.as_ref().unwrap().block);
    // the bytes that arrived are the bytes cut
    assert_eq!(state.recordings[0].1, flac_bytes(3));
    // the block is no part of the signed bytes; the recording entry is
    assert_eq!(state.results[0]["recording"]["bytes"], flac_bytes(3).len());
    let _ = std::fs::remove_dir_all(&b.dir);
}

fn sent_len(p: &PositionResult) -> usize {
    p.enhanced
        .as_ref()
        .unwrap()
        .recording
        .as_ref()
        .unwrap()
        .flac
        .len()
}

#[test]
fn an_erp_from_before_enhanced_data_is_sent_the_basic_result_and_told_so() {
    let b = bench("old-erp");
    b.erp.state.lock().unwrap().old_erp = true;
    let mut operator = Some(sign_in(&b));
    let report = run(
        &b,
        &mut operator,
        true,
        &[enhanced_position("Dial Up", 2.5, true)],
    );
    // still attested: what it could verify is what it was sent
    assert_eq!(statuses(&report), ["Attested"]);
    assert!(report
        .enhanced_declined
        .as_deref()
        .unwrap()
        .contains("does not take enhanced"));
    assert!(!report.sent[0].enhanced_kept);
    assert!(report.sent[0].recording.is_none());
    let state = b.erp.state.lock().unwrap();
    assert_eq!(state.results[0]["schema"], "ln_tg_timing_result_v1");
    assert!(state.results[0].get("enhanced").is_none());
    assert!(state.results[0].get("recording").is_none());
    assert!(state.recordings.is_empty());
    let _ = std::fs::remove_dir_all(&b.dir);
}

#[test]
fn an_erp_that_takes_enhanced_data_but_no_recordings_gets_the_block_alone() {
    let b = bench("no-recordings");
    b.erp.state.lock().unwrap().no_recordings = true;
    let mut operator = Some(sign_in(&b));
    let report = run(
        &b,
        &mut operator,
        true,
        &[enhanced_position("Dial Up", 2.5, true)],
    );
    assert_eq!(statuses(&report), ["Attested"]);
    assert!(report.sent[0].enhanced_kept);
    assert!(report.sent[0].recording.is_none());
    assert!(report
        .enhanced_declined
        .as_deref()
        .unwrap()
        .contains("recordings"));
    assert!(b.erp.state.lock().unwrap().recordings.is_empty());
    let _ = std::fs::remove_dir_all(&b.dir);
}

#[test]
fn a_position_with_no_recording_sends_the_block_and_no_upload() {
    let b = bench("block-only");
    let mut operator = Some(sign_in(&b));
    let report = run(
        &b,
        &mut operator,
        true,
        &[enhanced_position("Dial Up", 2.5, false)],
    );
    assert!(report.sent[0].enhanced_kept);
    assert!(report.sent[0].recording.is_none());
    let state = b.erp.state.lock().unwrap();
    assert!(state.results[0].get("recording").is_none());
    assert!(!state
        .log
        .iter()
        .any(|(name, _, _)| name == "submit_recording"));
    let _ = std::fs::remove_dir_all(&b.dir);
}

#[test]
fn results_without_enhanced_data_never_ask_the_erp_what_it_can_take() {
    let b = bench("plain");
    let mut operator = Some(sign_in(&b));
    let _ = run(&b, &mut operator, true, &[position("Dial Up", 1.0)]);
    let state = b.erp.state.lock().unwrap();
    assert!(!state.log.iter().any(|(name, _, _)| name == "ping"));
    assert_eq!(state.results[0]["schema"], "ln_tg_timing_result_v1");
    let _ = std::fs::remove_dir_all(&b.dir);
}

#[test]
fn an_enhanced_block_changed_after_it_was_signed_is_not_kept_and_marked_invalid() {
    let b = bench("tampered");
    let ctx = context("OP1");
    let result = enhanced_position("Dial Up", 2.5, false);
    let signed = result.signed_fields_with(&ctx, true);
    let signature = submit::signature_entry(&b.signer, &signed);
    let mut block = result.enhanced.as_ref().unwrap().block.clone();
    block["measurement"]["rate_s_per_day"] = json!(-99.0);
    let body = submit::wire_body(&signed, Some(signature), None, Some(TOKEN), Some(&block));
    let reply = b.client.submit_timing_result(&body).unwrap();
    assert_eq!(reply.verification_status.as_deref(), Some("Invalid"));
    assert!(reply.verification_message.unwrap().contains("digest"));
    assert_eq!(reply.enhanced_kept, Some(false));
    assert!(b.erp.state.lock().unwrap().enhanced.is_empty());
    let _ = std::fs::remove_dir_all(&b.dir);
}

#[test]
fn a_recording_that_is_not_the_one_signed_is_refused_by_the_erp() {
    let b = bench("swapped");
    let mut operator = Some(sign_in(&b));
    let report = run(
        &b,
        &mut operator,
        true,
        &[enhanced_position("Dial Up", 2.5, true)],
    );
    let id = report.sent[0]
        .outcome
        .as_ref()
        .unwrap()
        .result_id
        .clone()
        .unwrap();
    let other = b
        .client
        .submit_recording(&id, "x.flac", &flac_bytes(5))
        .unwrap();
    assert!(!other.ok);
    assert!(other.reason.unwrap().contains("does not match"));
    let _ = std::fs::remove_dir_all(&b.dir);
}

#[test]
fn a_recording_over_the_limit_is_refused_before_it_is_sent() {
    let b = bench("too-big");
    let big = vec![0u8; timegrapher_erp::client::MAX_RECORDING_BYTES + 1];
    let err = b.client.submit_recording("R1", "x.flac", &big).unwrap_err();
    assert!(err.to_string().contains("MB"), "{err}");
    assert!(!b
        .erp
        .state
        .lock()
        .unwrap()
        .log
        .iter()
        .any(|(n, _, _)| n == "submit_recording"));
    let _ = std::fs::remove_dir_all(&b.dir);
}

/// `timegrapher session --json` of the public sample take in `guide-figures/data/quick-listen` (80 s of a
/// dial-up reading from the public corpus), as the program wrote it, with only the file path made relative.
const REAL_SESSION: &str = include_str!("fixtures/real_session_summary.json");

#[test]
fn a_real_session_summary_is_read_signed_and_attested_with_the_figures_the_erp_should_hold() {
    let figures = timegrapher_erp::payload::read_summary(REAL_SESSION).unwrap();
    assert!(figures.left_out.is_empty(), "{:?}", figures.left_out);
    assert_eq!(figures.results.len(), 1);
    let b = bench("real-summary");
    let mut operator = Some(sign_in(&b));
    let report = run(&b, &mut operator, true, &figures.results);
    assert_eq!(statuses(&report), ["Attested"]);
    let state = b.erp.state.lock().unwrap();
    let sent = &state.results[0];
    assert_eq!(sent["position"], "Dial Up");
    let m = &sent["measurements"];
    // each figure of the real reading, rounded to six decimals and sent as a float
    assert_eq!(m["rate_s_per_day"], json!(0.448577));
    assert_eq!(m["amplitude_deg"], json!(270.535044));
    // the beat error is the size from the unlock figure (0.392342), not the drop's 0.335379
    assert_eq!(m["beat_error_ms"], json!(0.392342));
    assert_eq!(m["beat_frequency_bph"], json!(28800.0));
    assert_eq!(m["lift_angle_deg"], json!(55.0));
    // end_s 80 less start_s 20, with no masked_s in the JSON
    assert_eq!(m["duration_seconds"], json!(60.0));
    drop(state);
    let _ = std::fs::remove_dir_all(&b.dir);
}
