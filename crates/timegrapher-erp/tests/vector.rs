//! The published test vector (the LN ERP's station integration guide,
//! section 6.5). Its canonical bytes and its signature were made with the
//! ERP's own `canonical_json` and Python's `cryptography` library, so this
//! checks the Rust side against the other language, not against itself.

use serde_json::json;
use timegrapher_erp::canonical;
use timegrapher_erp::signing::Signer;

const CANONICAL: &str = r#"{"baseline":{"amplitude_deg":0.0,"beat_error_ms":0.0,"has_baseline":false,"rate_s_per_day":0.0},"followup_mode":false,"identity_id":"EXAMPLE-MOVEMENT-00001","measurement_type":"Initial Certification","measurements":{"amplitude_deg":245.5,"beat_error_ms":0.3,"beat_frequency_bph":28800.0,"duration_seconds":8.0,"lift_angle_deg":52.0,"rate_s_per_day":2.5,"sample_rate_hz":44100.0},"notes":"Averaged over 8 samples across 8s capture window (position: Dial Up)","operator_id":"OP1","position":"Dial Up","qa_rate_limit_s_per_day":20,"qa_standard":"Workshop","schema":"ln_tg_timing_result_v1","station_id":"EXAMPLE-STATION-01","station_version":"0.9.0","temperature_condition":"Room / 23C","timestamp_iso":"2026-11-11T09:30:00Z","work_order":""}"#;

const SIGNATURE: &str =
    "T4NL9KBqaB1EGsK8lP+Ahe5ETmuZXXJpVXz8B7nNemeH4UGt3CQLiQWwAqwNVS3gcl9WqXLSzFnwzDYFLgstAA==";

fn payload() -> serde_json::Value {
    json!({
        "schema": "ln_tg_timing_result_v1",
        "station_version": "0.9.0",
        "station_id": "EXAMPLE-STATION-01",
        "operator_id": "OP1",
        "identity_id": "EXAMPLE-MOVEMENT-00001",
        "work_order": "",
        "position": "Dial Up",
        "qa_standard": "Workshop",
        "qa_rate_limit_s_per_day": 20,
        "temperature_condition": "Room / 23C",
        "measurement_type": "Initial Certification",
        "followup_mode": false,
        "timestamp_iso": "2026-11-11T09:30:00Z",
        "measurements": {
            "rate_s_per_day": 2.5, "beat_error_ms": 0.3, "amplitude_deg": 245.5,
            "beat_frequency_bph": 28800.0, "lift_angle_deg": 52.0,
            "sample_rate_hz": 44100.0, "duration_seconds": 8.0
        },
        "baseline": {"has_baseline": false, "rate_s_per_day": 0.0, "beat_error_ms": 0.0, "amplitude_deg": 0.0},
        "notes": "Averaged over 8 samples across 8s capture window (position: Dial Up)"
    })
}

#[test]
fn the_canonical_bytes_are_the_ones_the_erp_makes() {
    let bytes = canonical::to_bytes(&payload());
    assert_eq!(String::from_utf8(bytes).unwrap(), CANONICAL);
}

#[test]
fn the_signature_is_the_one_the_other_library_made() {
    let seed: [u8; 32] = std::array::from_fn(|i| i as u8);
    let signer = Signer::from_seed(&seed, "EXAMPLE-STATION-01-workstation").unwrap();
    let signature = signer.sign_b64(&canonical::to_bytes(&payload()));
    assert_eq!(signature, SIGNATURE);
}
