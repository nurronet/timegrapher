//! Readings either side of a jump in the beats.

use super::*;
use crate::synth::{generate, SynthConfig};

/// A watch whose beats jump (a new rate and phase, no silence): the
/// readings after the jump are read once a whole average fits after
/// it, as at the start, not called no regular beat until the scatter's
/// look-back has cleared it (#1170).
#[test]
fn readings_return_a_whole_average_after_a_jump_in_the_beats() {
    let piece = |duration_s: f64, rate_s_per_day: f64, seed: u64| {
        generate(
            &SynthConfig {
                duration_s,
                rate_s_per_day,
                beat_error_ms: 0.2,
                seed,
                ..Default::default()
            },
            |_| 228.0,
            |_| 0.0,
        )
    };
    let mut samples = piece(30.0, 300.0, 1).samples;
    samples.extend(piece(30.3, 80.0, 2).samples.into_iter().skip(1234));
    let mut live = LiveAnalyzer::new(48000, LiveConfig::default());
    for chunk in samples.chunks(960) {
        live.push(chunk);
    }
    let jump = *live.breaks().last().expect("the jump is a break");
    assert!((30.0..34.0).contains(&jump), "{jump}");
    let end = jump + 12.0;
    let r = live.reading_at(end, 10.0);
    assert!(
        measure::no_watch_reason_in(&r.caveats).is_none(),
        "{:?}",
        r.caveats
    );
    assert!(r.whole);
    assert!((r.rate_s_per_day.unwrap() - 80.0).abs() < 2.0, "{r:?}");
    let resumed = live
        .trend(10.0)
        .into_iter()
        .find(|p| p.t > jump && p.rate_s_per_day.is_some())
        .unwrap()
        .t;
    assert!(
        resumed < jump + 12.0,
        "jump at {jump}, charts back at {resumed}"
    );
}

/// The bound the jump widens, pinned (Trevor, #1254): with no scatter
/// window fitted, a reading within SCATTER_LOOKBACK_S of a jump out of a
/// watch is not judged, and one past it is, as noise, as it is after a
/// heard break. A window fitted is judged whatever the cuts, and with no
/// such jump, nothing changes.
#[test]
fn no_window_after_a_jump_is_not_judged_until_the_look_back_clears_it() {
    let jump = 100.0;
    let at = |end: f64, fitted, heard, since| judged_s(fitted, end, end - 10.0, heard, since);
    assert_eq!(at(jump + 12.0, false, 0.0, jump), 0.0, "not assessed yet");
    assert_eq!(
        at(jump + 16.0, false, 0.0, jump),
        10.0,
        "judged: none fits, so noise"
    );
    assert_eq!(
        at(jump + 12.0, true, 0.0, jump),
        10.0,
        "a window fitted is judged"
    );
    let none = f64::NEG_INFINITY;
    assert_eq!(
        at(jump + 12.0, false, 0.0, none),
        10.0,
        "no jump out of a watch"
    );
    // A heard break later than the jump counts from itself, as before.
    assert_eq!(at(jump + 16.0, false, jump + 4.0, jump), 0.0);
    assert_eq!(at(jump + 20.0, false, jump + 4.0, jump), 10.0);
}
