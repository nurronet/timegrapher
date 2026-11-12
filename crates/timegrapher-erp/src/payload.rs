//! What is sent to the ERP for one position, and how it is read out of a
//! `timegrapher session` summary.
//!
//! Every figure comes from core: the summary's own numbers, the beat error
//! by core's rule ([`session::beat_error_size`]), the seconds measured
//! ([`measured_seconds`], which repeats what core's
//! `Measurement::measured_s` does, so this crate builds against core as
//! it is) and the position's name from core's [`Position`]. Nothing here
//! compares a reading with a
//! limit; the ERP does that against the QA standard.
//!
//! The ERP rebuilds the signed bytes from the numbers it parses, so each
//! figure is rounded to six decimals first and the same number is signed
//! and sent (see [`round6`]).

use serde_json::{json, Map, Value};
use timegrapher_core::session::{self, Position};

use crate::canonical;

/// The result schema the ERP reads.
pub const SCHEMA: &str = "ln_tg_timing_result_v1";
/// The schema of a result that carries enhanced data (see [`Enhanced`]).
pub const SCHEMA_ENHANCED: &str = "ln_tg_timing_result_v2";
/// The version of the enhanced block this writes (`enhanced_version`).
pub const ENHANCED_VERSION: u64 = 1;
/// What the ERP's `ping` lists under `capabilities` when it reads enhanced
/// results, and when it takes the recording of a position.
pub const CAPABILITY_ENHANCED: &str = "enhanced_results_v1";
pub const CAPABILITY_RECORDING: &str = "recording_upload_v1";

/// The summary document this reads.
pub const SUMMARY_SCHEMA_PREFIX: &str = "timegrapher.session/";
const SUMMARY_VERSION: &str = "2";

/// One position's figures, ready to send.
#[derive(Debug, Clone, PartialEq)]
pub struct PositionResult {
    /// The position as the ERP names it: "Dial Up", "Crown Left", ...
    pub position: &'static str,
    pub rate_s_per_day: Option<f64>,
    /// The size, ms: the ERP compares it with its limit as a plain number.
    pub beat_error_ms: Option<f64>,
    pub amplitude_deg: Option<f64>,
    pub beat_frequency_bph: Option<f64>,
    pub lift_angle_deg: Option<f64>,
    pub duration_seconds: Option<f64>,
    pub notes: String,
    /// The richer data for this position, when the station sends it.
    pub enhanced: Option<Enhanced>,
}

/// The richer data one position can carry beside its figures: the whole
/// measurement as core made it, and the sound it was measured from.
///
/// `block` is whatever the station records (the app sends core's
/// `Measurement`, the spans read and the Positions Test's indices); this
/// crate only signs its SHA-256 and sends it. The recording is a lossless
/// FLAC cut of the take, sent to the ERP apart from the result.
#[derive(Debug, Clone, PartialEq)]
pub struct Enhanced {
    pub block: Value,
    pub recording: Option<Recording>,
}

/// A position's sound, cut from the take.
#[derive(Debug, Clone, PartialEq)]
pub struct Recording {
    pub flac: Vec<u8>,
    /// The file name the ERP keeps it under, e.g. `Dial Up.flac`.
    pub name: String,
    /// Where the cut starts in the take, and how long it is, seconds.
    pub from_s: f64,
    pub duration_s: f64,
    pub sample_rate: u32,
    pub channels: u16,
    pub bits_per_sample: u16,
}

/// SHA-256 of `bytes` as lower-case hex, as Python's `hexdigest()`.
pub fn sha256_hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let digest = ring::digest::digest(&ring::digest::SHA256, bytes);
    digest.as_ref().iter().fold(String::new(), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

/// What stays the same across a watch's positions.
#[derive(Debug, Clone, PartialEq)]
pub struct Context {
    pub station_id: String,
    pub station_version: String,
    /// The Operator ID signed in, or whoever is named when nobody is.
    pub operator_id: String,
    pub identity_id: String,
    pub work_order: String,
    pub qa_standard: String,
    pub temperature_condition: String,
    pub measurement_type: String,
    /// UTC, `2026-11-11T09:30:00Z`.
    pub timestamp_iso: String,
}

/// The seconds measured, from a reading's `start_s`, `end_s` and
/// `masked_s` as the session JSON gives them: the stretch read, less what
/// was masked out. `masked_s` is left out of the JSON when it is zero, so
/// the caller passes 0. This repeats core's `Measurement::measured_s`
/// (`end_s - start_s - masked_s`) because a program that holds only the
/// JSON has no `Measurement` to ask. Keep the two the same: the tests below
/// pin the arithmetic, with and without `masked_s`.
fn measured_seconds(start_s: f64, end_s: f64, masked_s: f64) -> f64 {
    end_s - start_s - masked_s
}

/// `x` as it reads when written with six decimals, so that the number signed
/// is the number sent. `-0.0` becomes `0.0`.
pub fn round6(x: f64) -> f64 {
    let r = format!("{x:.6}").parse::<f64>().unwrap_or(x);
    if r == 0.0 {
        0.0
    } else {
        r
    }
}

impl PositionResult {
    /// Every field the ERP signs for this position (everything but the four
    /// that travel unsigned: `signatures`, `raw_samples`, `session` and
    /// `operator_session`). Figures the position did not give are left out.
    ///
    /// No `baseline` is sent: the ERP looks the stored baseline up itself,
    /// and a `baseline.rate_s_per_day` sent, even 0.0, would be taken as one.
    pub fn signed_fields(&self, ctx: &Context) -> Value {
        self.signed_fields_with(ctx, false)
    }

    /// [`Self::signed_fields`], and with `enhanced` the extra fields of
    /// schema `ln_tg_timing_result_v2` when this position has enhanced data:
    /// `enhanced_version`, the digest of the block (the block itself travels
    /// unsigned, as `enhanced`, so a large one costs one short signed field)
    /// and the `recording` entry: its size, digest and where it was cut, so
    /// the file sent afterwards cannot be swapped.
    pub fn signed_fields_with(&self, ctx: &Context, enhanced: bool) -> Value {
        let mut measurements = Map::new();
        for (key, value) in [
            ("amplitude_deg", self.amplitude_deg),
            ("beat_error_ms", self.beat_error_ms),
            ("beat_frequency_bph", self.beat_frequency_bph),
            ("duration_seconds", self.duration_seconds),
            ("lift_angle_deg", self.lift_angle_deg),
            ("rate_s_per_day", self.rate_s_per_day),
        ] {
            if let Some(v) = value {
                measurements.insert(key.to_string(), json!(round6(v)));
            }
        }
        let mut fields = json!({
            "schema": SCHEMA,
            "station_id": ctx.station_id,
            "station_version": ctx.station_version,
            "operator_id": ctx.operator_id,
            "identity_id": ctx.identity_id,
            "work_order": ctx.work_order,
            "position": self.position,
            "qa_standard": ctx.qa_standard,
            "temperature_condition": ctx.temperature_condition,
            "measurement_type": ctx.measurement_type,
            "timestamp_iso": ctx.timestamp_iso,
            "measurements": Value::Object(measurements),
            "notes": self.notes,
        });
        if let Some(extra) = self.enhanced.as_ref().filter(|_| enhanced) {
            fields["schema"] = json!(SCHEMA_ENHANCED);
            fields["enhanced_version"] = json!(ENHANCED_VERSION);
            fields["enhanced_sha256"] = json!(sha256_hex(&canonical::to_bytes(&extra.block)));
            if let Some(r) = &extra.recording {
                fields["recording"] = json!({
                    "format": "flac",
                    "name": r.name,
                    "bytes": r.flac.len(),
                    "sha256": sha256_hex(&r.flac),
                    "from_s": round6(r.from_s),
                    "duration_s": round6(r.duration_s),
                    "sample_rate": r.sample_rate,
                    "channels": r.channels,
                    "bits_per_sample": r.bits_per_sample,
                });
            }
        }
        fields
    }
}

/// The positions of a summary that have figures, and those left out.
#[derive(Debug, Clone, PartialEq)]
pub struct SessionFigures {
    pub results: Vec<PositionResult>,
    /// One line per reading not sent, and why.
    pub left_out: Vec<String>,
}

/// Reads a `timegrapher session` `summary.json`.
pub fn read_summary(text: &str) -> Result<SessionFigures, String> {
    let doc: Value =
        serde_json::from_str(text).map_err(|e| format!("not a session summary: {e}"))?;
    let schema = doc["schema"].as_str().unwrap_or("");
    let version = schema.strip_prefix(SUMMARY_SCHEMA_PREFIX);
    if version != Some(SUMMARY_VERSION) {
        return Err(format!(
            "this reads {SUMMARY_SCHEMA_PREFIX}{SUMMARY_VERSION}, and the file is \"{schema}\""
        ));
    }
    let readings = doc["readings"]
        .as_array()
        .ok_or("the summary has no readings")?;
    let software = doc["software"]["version"].as_str().unwrap_or("unknown");
    let lift = doc["input"]["settings"]["lift_deg"].as_f64();

    let mut results = Vec::new();
    let mut left_out = Vec::new();
    for reading in readings {
        match one_reading(reading, software, lift) {
            Ok(result) => results.push(result),
            Err(why) => left_out.push(why),
        }
    }
    Ok(SessionFigures { results, left_out })
}

fn one_reading(
    reading: &Value,
    software: &str,
    lift: Option<f64>,
) -> Result<PositionResult, String> {
    let code = reading["position"].as_str().unwrap_or("");
    let label = reading["label"].as_str().unwrap_or(code);
    let position = Position::parse(code)
        .ok_or_else(|| format!("{label}: position \"{code}\" is not one the ERP knows"))?;
    let m = &reading["measurement"];
    if m["watch_heard"].as_bool() == Some(false) {
        return Err(format!(
            "{label}: no regular beat was found, so there is nothing to send"
        ));
    }
    let number = |key: &str| m[key].as_f64();
    let measured = measured_seconds(
        number("start_s").unwrap_or(0.0),
        number("end_s").unwrap_or(0.0),
        number("masked_s").unwrap_or(0.0),
    );
    let beats = m["beats"].as_u64().unwrap_or(0);
    Ok(PositionResult {
        position: position.name(),
        rate_s_per_day: number("rate_s_per_day"),
        beat_error_ms: session::beat_error_size(
            number("beat_error_unlock_ms"),
            number("beat_error_ms"),
        ),
        amplitude_deg: number("amplitude_deg"),
        beat_frequency_bph: reading["bph"].as_f64(),
        lift_angle_deg: lift,
        duration_seconds: Some(measured),
        notes: format!(
            "Measured by timegrapher {software}: {beats} beats over {measured:.0} s ({label})"
        ),
        enhanced: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> Context {
        Context {
            station_id: "S1".into(),
            station_version: "1.0.0".into(),
            operator_id: "OP1".into(),
            identity_id: "ID-1".into(),
            work_order: String::new(),
            qa_standard: "Workshop".into(),
            temperature_condition: "Room / 23C".into(),
            measurement_type: "Initial Certification".into(),
            timestamp_iso: "2026-11-11T09:30:00Z".into(),
        }
    }

    fn reading(position: &str, measurement: Value) -> Value {
        json!({"label": format!("take {position}"), "position": position, "bph": 28800,
               "measurement": measurement})
    }

    fn summary(readings: Vec<Value>) -> String {
        json!({"schema": "timegrapher.session/2", "software": {"version": "1.0.0"},
               "input": {"settings": {"lift_deg": 52.0}}, "readings": readings})
        .to_string()
    }

    fn full(rate: f64) -> Value {
        json!({"start_s": 20.0, "end_s": 80.0, "masked_s": 10.0, "beats": 100,
               "rate_s_per_day": rate, "beat_error_ms": -0.25, "beat_error_unlock_ms": -0.4,
               "amplitude_deg": 285.5, "watch_heard": true})
    }

    #[test]
    fn the_seconds_measured_are_the_stretch_less_what_was_masked() {
        assert_eq!(measured_seconds(20.0, 80.0, 10.0), 50.0);
        // masked_s is left out of the JSON when zero, so 0 is passed
        assert_eq!(measured_seconds(20.0, 80.0, 0.0), 60.0);
        // a reading that gave no masked_s is read the same as a zero one
        let mut m = full(1.0);
        m.as_object_mut().unwrap().remove("masked_s");
        let figures = read_summary(&summary(vec![reading("3H", m)])).unwrap();
        assert_eq!(figures.results[0].duration_seconds, Some(60.0));
    }

    #[test]
    fn six_decimals_make_the_number_that_is_signed_and_sent_the_same() {
        assert_eq!(round6(12.3456789), 12.345679);
        assert_eq!(round6(-0.0000001), 0.0);
        assert!(round6(-0.0000001).is_sign_positive());
        assert_eq!(round6(2.5), 2.5);
        // what the ERP parses from the text is what was signed
        let text = format!("{}", round6(0.1 + 0.2));
        assert_eq!(text.parse::<f64>().unwrap(), round6(0.1 + 0.2));
    }

    #[test]
    fn a_position_is_read_with_the_figures_core_decides() {
        let figures = read_summary(&summary(vec![reading("3H", full(2.5))])).unwrap();
        assert!(figures.left_out.is_empty());
        let r = &figures.results[0];
        assert_eq!(r.position, "Crown Up");
        assert_eq!(r.rate_s_per_day, Some(2.5));
        // core: the unlock figure where found, as a size
        assert_eq!(r.beat_error_ms, Some(0.4));
        assert_eq!(r.amplitude_deg, Some(285.5));
        assert_eq!(r.beat_frequency_bph, Some(28800.0));
        assert_eq!(r.lift_angle_deg, Some(52.0));
        // core: end less start less masked
        assert_eq!(r.duration_seconds, Some(50.0));
        assert!(r.notes.contains("100 beats over 50 s"), "{}", r.notes);
    }

    #[test]
    fn the_drop_beat_error_is_used_only_where_no_unlock_was_found() {
        let mut m = full(1.0);
        m["beat_error_unlock_ms"] = Value::Null;
        let figures = read_summary(&summary(vec![reading("CH", m)])).unwrap();
        assert_eq!(figures.results[0].beat_error_ms, Some(0.25));
    }

    #[test]
    fn all_six_positions_get_the_names_the_erp_uses() {
        let readings = ["CH", "CB", "9H", "6H", "3H", "12H"]
            .iter()
            .map(|p| reading(p, full(0.0)))
            .collect();
        let names: Vec<&str> = read_summary(&summary(readings))
            .unwrap()
            .results
            .iter()
            .map(|r| r.position)
            .collect();
        assert_eq!(
            names,
            [
                "Dial Up",
                "Dial Down",
                "Crown Down",
                "Crown Left",
                "Crown Up",
                "Crown Right"
            ]
        );
    }

    #[test]
    fn a_reading_with_no_regular_beat_is_left_out_with_the_reason() {
        let mut m = full(0.0);
        m["watch_heard"] = json!(false);
        let figures =
            read_summary(&summary(vec![reading("CH", m), reading("CB", full(1.0))])).unwrap();
        assert_eq!(figures.results.len(), 1);
        assert_eq!(figures.results[0].position, "Dial Down");
        assert!(
            figures.left_out[0].contains("no regular beat"),
            "{:?}",
            figures.left_out
        );
    }

    #[test]
    fn a_position_the_erp_does_not_know_is_left_out_not_guessed() {
        let figures = read_summary(&summary(vec![reading("sideways", full(1.0))])).unwrap();
        assert!(figures.results.is_empty());
        assert!(figures.left_out[0].contains("sideways"));
    }

    #[test]
    fn another_summary_version_or_a_different_file_is_refused_by_name() {
        let err = read_summary(r#"{"schema":"timegrapher.session/3","readings":[]}"#)
            .err()
            .unwrap();
        assert!(err.contains("timegrapher.session/3"), "{err}");
        assert!(read_summary("not json").is_err());
        assert!(read_summary(r#"{"schema":"timegrapher.session/2"}"#).is_err());
    }

    #[test]
    fn the_signed_fields_hold_what_the_erp_reads_and_no_baseline() {
        let figures = read_summary(&summary(vec![reading("CH", full(12.3456789))])).unwrap();
        let v = figures.results[0].signed_fields(&ctx());
        assert_eq!(v["measurements"]["rate_s_per_day"], json!(12.345679));
        assert_eq!(v["position"], "Dial Up");
        assert_eq!(v["schema"], SCHEMA);
        assert!(v.get("baseline").is_none());
        for wire_only in ["signatures", "raw_samples", "session", "operator_session"] {
            assert!(v.get(wire_only).is_none(), "{wire_only}");
        }
    }

    fn enhanced_result() -> PositionResult {
        let mut r = read_summary(&summary(vec![reading("CH", full(2.5))]))
            .unwrap()
            .results
            .remove(0);
        r.enhanced = Some(Enhanced {
            block: json!({"version": 1, "measurement": {"beats": 100}}),
            recording: Some(Recording {
                flac: b"fLaC-bytes".to_vec(),
                name: "Dial Up.flac".into(),
                from_s: 20.0,
                duration_s: 60.123456789,
                sample_rate: 48000,
                channels: 1,
                bits_per_sample: 16,
            }),
        });
        r
    }

    #[test]
    fn sha256_is_the_digest_the_erp_computes() {
        // the standard test vector
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn enhanced_data_changes_the_schema_and_signs_the_digest_not_the_block() {
        let r = enhanced_result();
        let v = r.signed_fields_with(&ctx(), true);
        assert_eq!(v["schema"], SCHEMA_ENHANCED);
        assert_eq!(v["enhanced_version"], 1);
        let block = &r.enhanced.as_ref().unwrap().block;
        assert_eq!(
            v["enhanced_sha256"],
            json!(sha256_hex(&canonical::to_bytes(block)))
        );
        // the block itself is not among the signed fields
        assert!(v.get("enhanced").is_none());
    }

    #[test]
    fn the_signed_recording_entry_names_the_file_by_size_and_digest() {
        let v = enhanced_result().signed_fields_with(&ctx(), true);
        let rec = &v["recording"];
        assert_eq!(rec["bytes"], 10);
        assert_eq!(rec["sha256"], json!(sha256_hex(b"fLaC-bytes")));
        assert_eq!(rec["format"], "flac");
        // six decimals, like every figure, so what is signed is what is parsed
        assert_eq!(rec["duration_s"], json!(60.123457));
        assert_eq!(rec["sample_rate"], 48000);
    }

    #[test]
    fn without_asking_for_enhanced_a_result_with_enhanced_data_signs_as_before() {
        let r = enhanced_result();
        let basic = r.signed_fields(&ctx());
        assert_eq!(basic["schema"], SCHEMA);
        for key in ["enhanced_version", "enhanced_sha256", "recording"] {
            assert!(basic.get(key).is_none(), "{key}");
        }
        assert_eq!(r.signed_fields_with(&ctx(), false), basic);
    }

    #[test]
    fn a_changed_block_or_recording_changes_what_is_signed() {
        let a = enhanced_result();
        let mut b = a.clone();
        b.enhanced.as_mut().unwrap().block["measurement"]["beats"] = json!(101);
        assert_ne!(
            a.signed_fields_with(&ctx(), true),
            b.signed_fields_with(&ctx(), true)
        );
        let mut c = a.clone();
        c.enhanced
            .as_mut()
            .unwrap()
            .recording
            .as_mut()
            .unwrap()
            .flac[0] ^= 1;
        assert_ne!(
            a.signed_fields_with(&ctx(), true),
            c.signed_fields_with(&ctx(), true)
        );
    }

    #[test]
    fn a_result_with_no_enhanced_data_signs_as_before_even_when_asked() {
        let r = read_summary(&summary(vec![reading("CH", full(2.5))]))
            .unwrap()
            .results
            .remove(0);
        assert_eq!(r.signed_fields_with(&ctx(), true), r.signed_fields(&ctx()));
    }

    #[test]
    fn a_figure_the_position_did_not_give_is_left_out_not_sent_as_zero() {
        let mut m = full(1.0);
        m["amplitude_deg"] = Value::Null;
        let figures = read_summary(&summary(vec![reading("CH", m)])).unwrap();
        let v = figures.results[0].signed_fields(&ctx());
        assert!(v["measurements"].get("amplitude_deg").is_none());
        assert!(v["measurements"].get("rate_s_per_day").is_some());
    }
}
