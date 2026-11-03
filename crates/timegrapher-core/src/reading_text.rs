//! Shared words for a reading in the CLI and MCP JSON documents.
//! Surfaces choose the reading; core chooses its figures, precision and units.

use serde_json::Value;

/// Rate (already on the chosen clock), amplitude and unsigned beat error.
/// The unlock wins over the drop, as in every other core reading.
pub fn values(m: &Value, rate: Option<f64>, drop_ms: Option<f64>) -> [Option<f64>; 3] {
    [
        rate,
        m["amplitude_deg"].as_f64(),
        crate::session::beat_error_size(m["beat_error_unlock_ms"].as_f64(), drop_ms),
    ]
}

/// One reading's figures, each with its own standard error. Units follow
/// the math and design guides: s/d, degrees as °, and ms. Missing figures
/// are omitted; an entirely empty reading gives no line.
pub fn figures(m: &Value, [rate, amplitude, beat_error]: [Option<f64>; 3]) -> Option<String> {
    let pm = |key: &str, digits| {
        m["confidence"][key]["se"]
            .as_f64()
            .and_then(|se| crate::library::pm_text(se, digits))
            .map_or_else(String::new, |s| format!(" ± {s}"))
    };
    let mut parts = Vec::new();
    if let Some(r) = rate {
        parts.push(format!(
            "rate {}{} s/d",
            crate::library::signed_text(r, 1),
            pm("rate", 1)
        ));
    }
    if let Some(a) = amplitude {
        parts.push(format!("amplitude {a:.0}{}°", pm("amplitude", 0)));
    }
    if let Some(b) = beat_error {
        parts.push(format!("beat error {b:.2}{} ms", pm("beat_error", 2)));
    }
    (!parts.is_empty()).then(|| parts.join(", "))
}

/// A session/take measurement's figures, or the fact that no beat was found.
pub fn measurement(m: &Value) -> String {
    if m["watch_heard"] == false {
        "no regular beat found".into()
    } else {
        figures(
            m,
            values(m, m["rate_s_per_day"].as_f64(), m["beat_error_ms"].as_f64()),
        )
        .unwrap_or_default()
    }
}

/// A known position's full name, preserving an unknown code as supplied.
pub fn position_name(p: &str) -> String {
    crate::session::Position::parse(p).map_or_else(|| p.to_string(), |p| p.display_label())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reading_text_matches_the_cross_surface_cases() {
        let cases: Value =
            serde_json::from_str(include_str!("../tests/fixtures/reading_text.json")).unwrap();
        for c in cases.as_array().unwrap() {
            assert_eq!(measurement(&c["measurement"]), c["text"].as_str().unwrap());
            assert_eq!(
                position_name(c["position"].as_str().unwrap()),
                c["position_text"].as_str().unwrap()
            );
        }
    }
}
